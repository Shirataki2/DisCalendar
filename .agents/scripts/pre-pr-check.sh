#!/usr/bin/env bash
# PR を作る前のチェック (Claude Code / Codex 共通)。CI では落ちるのに忘れやすいものを手元でまとめて見る。
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
for crate in api bot; do
  rs_files=$(grep -E "^$crate/(src|tests)/.*\.rs$" <<<"$changed" || true)
  [ -n "$rs_files" ] || continue
  # 変更行 (+/-) にクエリマクロか SQL が含まれるか
  # shellcheck disable=SC2086
  touched_sql=$(git diff -U0 "$merge_base" -- $rs_files | grep -E '^[+-][^+-]' | grep -E "$sqlx_pattern" || true)
  for f in $rs_files; do
    if git ls-files --others --exclude-standard -- "$f" | grep -q . && grep -Eq "$sqlx_pattern" "$f"; then
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

# DATABASE_URL を値を表示せずに用意する (クレートの .env → api/.env の順)
database_url_for() {
  local crate=$1 f
  if [ -n "${DATABASE_URL:-}" ]; then
    printf '%s' "$DATABASE_URL"
    return
  fi
  for f in "$crate/.env" api/.env; do
    if [ -f "$f" ] && grep -q '^DATABASE_URL=' "$f"; then
      grep '^DATABASE_URL=' "$f" | head -1 | cut -d= -f2-
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
  if has_changes '^(api/|bot/|shared/|Cargo\.toml|Cargo\.lock)'; then
    run "rust: cargo fmt --check" cargo fmt --all --check
    run "rust: cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
    run "rust: cargo test" cargo test --workspace
    for crate in api bot; do
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
    if command -v shellcheck >/dev/null 2>&1; then
      # shellcheck disable=SC2046
      run "shellcheck" shellcheck $(grep -E '\.sh$' <<<"$changed" | while read -r f; do [ -f "$f" ] && echo "$f"; done)
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
