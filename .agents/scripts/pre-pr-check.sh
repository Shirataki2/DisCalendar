#!/usr/bin/env bash
# PR を作る前のチェック (Claude Code / Codex / Antigravity / Gemini CLI 共通)。CI では落ちるのに忘れやすいものを手元でまとめて見る。
#
#   bash .agents/scripts/pre-pr-check.sh                 # 検出 + 変更したパスに応じた検証 (lint / 型 / テスト)
#   bash .agents/scripts/pre-pr-check.sh --detect-only   # 検出だけ (コマンドを流さないので数秒で終わる)
#   bash .agents/scripts/pre-pr-check.sh --no-changelog  # 更新履歴の有無を見ない (Stop hook 用)
#   bash .agents/scripts/pre-pr-check.sh --base origin/main
#
# 比較対象は base (既定 origin/main) との merge-base から作業ツリーまで (未コミット・未追跡のファイルも含む)。
# 検出するもの:
#   - sqlx のクエリを変えたのに .sqlx/ が更新されていない (DB に繋がれば cargo sqlx prepare --check で確かめる)
#   - 利用者に見える変更なのに changelog.d/<Issue番号>-<短い名前>.md が無い / 形式が違う
#   - api/migrations/ に追加したのに api/rollback/ に同じ版の戻し方が無い
#   - 機能 PR でバージョン (web/package.json / api・bot の Cargo.toml) を上げている
#   - web と api / bot の上限値のずれ (.github/scripts/check-limits.sh)
# 終了コード: 0 = 問題なし (警告だけのときも 0)、1 = 検証コマンドの失敗または形式の誤り
set -uo pipefail

base=origin/main
detect_only=false
check_changelog=true
while [ $# -gt 0 ]; do
  case "$1" in
    --detect-only) detect_only=true ;;
    --no-changelog) check_changelog=false ;;
    --base)
      base=$2
      shift
      ;;
    -h | --help)
      sed -n '2,17p' "$0"
      exit 0
      ;;
    *)
      echo "不明な引数: $1" >&2
      exit 2
      ;;
  esac
  shift
done

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2

warnings=()
errors=()
warn() { warnings+=("$1"); }
error() { errors+=("$1"); }

if ! merge_base=$(git merge-base "$base" HEAD 2>/dev/null); then
  echo "base ($base) との merge-base が取れません。git fetch origin を実行してください。" >&2
  exit 2
fi

# 変更されたファイル (追加・変更・削除・未追跡)
changed=$( {
  git diff --name-only "$merge_base"
  git ls-files --others --exclude-standard
} | sort -u)
added=$( {
  git diff --name-only --diff-filter=A "$merge_base"
  git ls-files --others --exclude-standard
} | sort -u)

has_changes() { grep -Eq "$1" <<<"$changed"; }

if [ -z "$changed" ]; then
  echo "base ($base) からの変更はありません。"
  exit 0
fi

# --- sqlx のオフラインデータ ---
sqlx_pattern='query(_as|_scalar|_file|_file_as)?(_unchecked)?!|(^|[^A-Za-z_])(SELECT|INSERT INTO|UPDATE|DELETE FROM|RETURNING|ON CONFLICT|WHERE)([^A-Za-z_]|$)'

# ファイル $1 の変更行 (削除だけの箇所は前後の行) が query! 系マクロの呼び出し (開き括弧から対応する閉じ括弧まで) に入るか
macro_touched() {
  local f=$1 lines
  lines=$(git diff -U0 "$merge_base" -- "$f" | awk '/^@@/ {
    split($3, a, ","); s = substr(a[1], 2) + 0; c = (a[2] == "") ? 1 : a[2] + 0
    if (c == 0) { print s; print s + 1 } else for (i = 0; i < c; i++) print s + i
  }' | tr '\n' ' ')
  [ -n "$lines" ] || return 1
  awk -v lines="$lines" '
    BEGIN { n = split(lines, L, " "); for (i = 1; i <= n; i++) T[L[i]] = 1 }
    {
      rest = $0
      if (!in_m && match($0, /query(_as|_scalar|_file|_file_as)?(_unchecked)?!/)) {
        in_m = 1; depth = 0; opened = 0; hit = 0; rest = substr($0, RSTART)
      }
      if (!in_m) next
      if (NR in T) hit = 1
      t = rest; o = gsub(/\(/, "", t); t = rest; c = gsub(/\)/, "", t)
      depth += o - c; if (o > 0) opened = 1
      if (opened && depth <= 0) { if (hit) { found = 1; exit } in_m = 0 }
    }
    END { exit found ? 0 : 1 }
  ' "$f"
}
for crate in api bot support-bot; do
  rs_files=$(grep -E "^$crate/(src|tests)/.*\.rs$" <<<"$changed" || true)
  [ -n "$rs_files" ] || continue
  # 変更行 (+/-) にクエリマクロか SQL が含まれるか
  # shellcheck disable=SC2086
  touched_sql=$(git diff -U0 "$merge_base" -- $rs_files | grep -E '^[+-][^+-]' | grep -E "$sqlx_pattern" || true)
  for f in $rs_files; do
    [ -f "$f" ] || continue
    if git ls-files --others --exclude-standard -- "$f" | grep -q .; then
      grep -Eq "$sqlx_pattern" "$f" && touched_sql=yes
      continue
    fi
    # 複数行の SQL の途中だけを変えた場合も拾うため、変更行 (新しい側の行番号) がクエリマクロの範囲に入るかを見る
    if macro_touched "$f"; then
      touched_sql=yes
    fi
  done
  [ -n "$touched_sql" ] || continue
  if ! has_changes "^$crate/\.sqlx/"; then
    warn "$crate: sqlx のクエリを変えたように見えますが $crate/.sqlx/ が更新されていません。$crate/ で \`cargo sqlx prepare -- --all-targets\` を実行してください (CI は SQLX_OFFLINE=true)。"
  fi
done

# --- 更新履歴 ---
if $check_changelog; then
  user_facing=$(grep -E '^(web/src/|web/public/|api/src/|bot/src/|shared/)' <<<"$changed" |
    grep -Ev '(\.test\.|\.spec\.|/__tests__/|/tests?/|^web/src/assets/)' || true)
  new_entries=$(grep -E '^changelog\.d/' <<<"$added" | grep -v '^changelog\.d/README\.md$' || true)
  if [ -n "$user_facing" ] && [ -z "$new_entries" ]; then
    warn "利用者に見えるコード (web/src・api/src・bot/src など) を変えていますが changelog.d/ に更新履歴がありません。利用者に見える機能追加・変更・不具合修正なら changelog.d/<Issue番号>-<短い名前>.md を追加してください (内部変更だけなら不要)。"
  fi
  for entry in $new_entries; do
    [ -f "$entry" ] || continue
    name=$(basename "$entry")
    if ! [[ $name =~ ^[0-9]+-[a-z0-9-]+\.md$ ]]; then
      error "$entry: ファイル名は <Issue番号>-<短い名前>.md にしてください (changelog.d/README.md)。"
    fi
    if ! head -1 "$entry" | grep -Eq '^### '; then
      error "$entry: 最初の行は \`### タイトル\` にしてください (changelog.d/README.md)。"
    fi
  done
fi

# --- マイグレーションと戻し方 ---
for m in $(grep -E '^api/migrations/[0-9]{14}_.*\.sql$' <<<"$added" || true); do
  version=$(basename "$m" | cut -c1-14)
  if ! ls api/rollback/"${version}"_*.sql >/dev/null 2>&1; then
    warn "$m に対応する戻し方 api/rollback/${version}_*.sql がありません。古い api はマイグレーションが当たった DB では起動できないので、カラム追加のような変更でも用意してください (docs/operations.md「マイグレーションが入った版から戻す」)。"
  fi
done
for m in $(git diff --name-only --diff-filter=MDR "$merge_base" -- 'api/migrations/*.sql'); do
  error "$m: 適用済みのマイグレーションを変更・削除しています。_sqlx_migrations のチェックサムと合わなくなるので、新しいファイルで変更してください。"
done

# --- バージョン (機能 PR では上げない) ---
branch=$(git rev-parse --abbrev-ref HEAD)
if git diff "$merge_base" -- web/package.json api/Cargo.toml bot/Cargo.toml | grep -Eq '^\+[[:space:]]*"?version"?[[:space:]]*[:=]'; then
  case "$branch" in
    *release*) ;;
    *) warn "バージョンを変更しています。バージョンはリリース準備でだけ上げます (.claude/skills/release/)。リリース準備でなければ戻してください。" ;;
  esac
fi

# --- 上限値のずれ ---
if has_changes '^(web/src/|api/src/|bot/src/|\.github/scripts/check-limits\.sh)'; then
  if ! limits_out=$(bash .github/scripts/check-limits.sh 2>&1); then
    error "web と api / bot の上限値がずれています:
$limits_out"
  fi
fi

# --- 検証コマンド ---
run() {
  local label=$1
  shift
  echo "▶ $label"
  if ! "$@"; then
    error "$label が失敗しました。"
  fi
}

# DATABASE_URL を値を表示せずに用意する (クレートの .env → api/.env の順)。
# support-bot は本体と別のデータベースを使うので、support-bot/.env だけを見る
# (環境変数の DATABASE_URL や api/.env は本体の DB を指していて、prepare --check が食い違うため)
database_url_for() {
  local crate=$1 f candidates
  if [ "$crate" = support-bot ]; then
    candidates=(support-bot/.env)
  elif [ -n "${DATABASE_URL:-}" ]; then
    printf '%s' "$DATABASE_URL"
    return
  else
    candidates=("$crate/.env" api/.env)
  fi
  local v
  for f in "${candidates[@]}"; do
    if [ -f "$f" ] && grep -Eq '^(export[[:space:]]+)?DATABASE_URL=' "$f"; then
      # dotenv と同じく export 接頭辞・前後の引用符・行末の空白や CR を外す
      v=$(grep -E '^(export[[:space:]]+)?DATABASE_URL=' "$f" | head -1 | cut -d= -f2- | sed -e 's/[[:space:]]*$//' | tr -d '\r')
      case "$v" in
        \"*\") v=${v#\"} v=${v%\"} ;;
        \'*\') v=${v#\'} v=${v%\'} ;;
        *)
          v=${v%%[[:space:]]#*}
          v=${v%"${v##*[![:space:]]}"}
          ;;
      esac
      printf '%s' "$v"
      return
    fi
  done
}

if ! $detect_only; then
  if has_changes '^web/'; then
    if [ -d web/node_modules ]; then
      run "web: pnpm lint" pnpm -C web lint
      run "web: tsc --noEmit" bash -c 'cd web && pnpm exec next typegen >/dev/null 2>&1; pnpm exec tsc --noEmit'
      run "web: pnpm test" pnpm -C web test
    else
      warn "web/node_modules が無いので web の検証を飛ばしました (pnpm -C web install --frozen-lockfile)。"
    fi
  fi
  if has_changes '^(api/|bot/|support-bot/|shared/|Cargo\.toml|Cargo\.lock)'; then
    run "rust: cargo fmt --check" cargo fmt --all --check
    # bot/.env が無く api/.env だけある構成でも、bot の DB テストが接続先を得られるように渡す (値は表示しない)。
    # クエリのコンパイル時チェックは .sqlx/ で行う (SQLX_OFFLINE=true、CI と同じ)。DATABASE_URL を 1 つ渡すと
    # 別のデータベースを使う support-bot のクエリまでその DB で検査されて落ちるため。.sqlx/ のずれは下の prepare --check で見る。
    # #[sqlx::test] はこの DATABASE_URL に一時データベースを作るだけなので、どのクレートのテストにも使える
    workspace_url=$(database_url_for bot)
    if [ -n "$workspace_url" ]; then
      run "rust: cargo clippy" env SQLX_OFFLINE=true DATABASE_URL="$workspace_url" cargo clippy --workspace --all-targets -- -D warnings
      run "rust: cargo test" env SQLX_OFFLINE=true DATABASE_URL="$workspace_url" cargo test --workspace
    else
      run "rust: cargo clippy" env SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings
      run "rust: cargo test" env SQLX_OFFLINE=true cargo test --workspace
    fi
    for crate in api bot support-bot; do
      has_changes "^$crate/(src|tests|\.sqlx)/" || continue
      url=$(database_url_for "$crate")
      if [ -n "$url" ] && cargo sqlx --version >/dev/null 2>&1; then
        echo "▶ $crate: cargo sqlx prepare --check"
        if ! (cd "$crate" && DATABASE_URL=$url cargo sqlx prepare --check -- --all-targets >/dev/null 2>&1); then
          error "$crate: .sqlx/ がクエリと合っていません。$crate/ で \`cargo sqlx prepare -- --all-targets\` を実行してください。"
        fi
      fi
    done
  fi
  if has_changes '^(\.claude/hooks/|\.agents/scripts/|\.github/scripts/|infra/backup/)'; then
    sh_files=$(grep -E '\.sh$' <<<"$changed" | while read -r f; do [ -f "$f" ] && echo "$f"; done)
    if [ -n "$sh_files" ] && command -v shellcheck >/dev/null 2>&1; then
      # shellcheck disable=SC2086
      run "shellcheck" shellcheck $sh_files
    fi
    [ -f .claude/hooks/tests/pre-tool-guard-test.sh ] && run "hook のテスト" bash .claude/hooks/tests/pre-tool-guard-test.sh
  fi
fi

# --- 結果 ---
echo
if [ ${#warnings[@]} -eq 0 ] && [ ${#errors[@]} -eq 0 ]; then
  echo "PR 前チェック: 問題なし"
  exit 0
fi
for w in ${warnings[@]+"${warnings[@]}"}; do
  echo "警告: $w"
done
for e in ${errors[@]+"${errors[@]}"}; do
  echo "エラー: $e"
done
[ ${#errors[@]} -eq 0 ]
