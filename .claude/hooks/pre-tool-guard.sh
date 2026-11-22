#!/usr/bin/env bash
# PreToolUse hook: AGENTS.md の P0 規則のうち、パスとコマンドの形で判定できるものをツール実行前に止める。
#   - api/migrations/ の既存ファイル (git 管理下) の編集 → 拒否 (_sqlx_migrations のチェックサムと照合されるため)
#   - 新しいマイグレーションのファイル名 → 形式・順序・連番の時刻を検査 (`date` で取った実時刻を使う)
#   - .env 系 (*.example を除く) の Read / Edit / Write、Bash での丸ごと表示 → 拒否
#   - main への直接 git push → 拒否 (サーバー側のルールセットでも拒否されるが、手前で止める)
# 判定できない入力 (jq が無い、JSON が壊れている) では何もしない (誤検知で作業を止めない)。
set -u

command -v jq >/dev/null 2>&1 || exit 0
input=$(cat)
tool=$(jq -r '.tool_name // empty' <<<"$input" 2>/dev/null) || exit 0
[ -n "$tool" ] || exit 0

root=${CLAUDE_PROJECT_DIR:-$(jq -r '.cwd // empty' <<<"$input")}
[ -n "$root" ] || root=$(pwd)

deny() {
  jq -n --arg r "$1" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $r}}'
  exit 0
}

# リポジトリ相対のパスにする (リポジトリ外なら空)
relpath() {
  local p=$1
  case "$p" in
    /*) ;;
    *) p="$root/$p" ;;
  esac
  case "$p" in
    "$root"/*) printf '%s' "${p#"$root"/}" ;;
    *) printf '' ;;
  esac
}

# .env 系の秘密ファイルか (.env.example などのひな形は除く)
is_secret_env() {
  local base
  base=$(basename "$1")
  case "$base" in
    .env.example | .env.sample | .env.template) return 1 ;;
    .env | .env.*) return 0 ;;
  esac
  return 1
}

check_migration_name() {
  local rel=$1 base stamp hms latest now_stamp
  base=$(basename "$rel")
  if ! [[ $base =~ ^([0-9]{14})_[a-z0-9_]+\.sql$ ]]; then
    deny "マイグレーションのファイル名は <yyyymmddHHMMSS>_<英小文字とアンダースコア>.sql にしてください (受け取った名前: $base)。時刻は \`date \"+%Y%m%d%H%M%S\"\` で取得した実時刻を使います。"
  fi
  stamp=${BASH_REMATCH[1]}
  hms=${stamp:8:6}
  # 000001 / 000002 … のような連番は禁止 (時刻は実時刻にする)
  if [[ $hms =~ ^0000[0-9][0-9]$ ]] && [ "$hms" != "000000" ]; then
    deny "マイグレーションの時刻部分 ($hms) が連番になっています。\`date \"+%Y%m%d%H%M%S\"\` を実行して得た実時刻をファイル名に使ってください。"
  fi
  # 既存の最新より新しいこと (sqlx は版の昇順で適用する。古い版を後から足すと本番で適用されない)
  latest=$(find "$root/api/migrations" -maxdepth 1 -name '[0-9]*_*.sql' -exec basename {} \; 2>/dev/null | cut -c1-14 | sort | tail -1)
  if [ -n "$latest" ] && { [ "$stamp" \< "$latest" ] || [ "$stamp" = "$latest" ]; }; then
    deny "マイグレーションの版 ($stamp) が既存の最新 ($latest) 以前です。\`date \"+%Y%m%d%H%M%S\"\` で取得した現在時刻を使ってください。"
  fi
  # 未来すぎる版 (1 日より先) も、日付の取り違えとして止める
  now_stamp=$(date -v+1d "+%Y%m%d%H%M%S" 2>/dev/null || date -d '+1 day' "+%Y%m%d%H%M%S" 2>/dev/null || true)
  if [ -n "$now_stamp" ] && [ "$stamp" \> "$now_stamp" ]; then
    deny "マイグレーションの版 ($stamp) が現在時刻より 1 日以上先です。\`date \"+%Y%m%d%H%M%S\"\` で取得した現在時刻を使ってください。"
  fi
}

check_file_tool() {
  local path rel
  path=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")
  [ -n "$path" ] || return 0
  rel=$(relpath "$path")

  if is_secret_env "$path"; then
    deny ".env 系のファイル ($(basename "$path")) はエージェントから読み書きしない決まりです (AGENTS.md の P0: 秘密情報)。必要な値は .env.example を見るか、ユーザーに設定を依頼してください。"
  fi

  [ "$tool" = "Read" ] && return 0
  [ -n "$rel" ] || return 0

  case "$rel" in
    api/migrations/*.sql)
      if git -C "$root" ls-files --error-unmatch -- "$rel" >/dev/null 2>&1; then
        deny "適用済みのマイグレーション ($rel) は変更禁止です (_sqlx_migrations のチェックサムと照合され、本番の api が起動しなくなる)。スキーマの変更は新しいファイルで行ってください。データの持ち方が変わるなら api/rollback/ に戻し方も用意します。"
      fi
      if [ "$tool" = "Write" ] && [ ! -e "$root/$rel" ]; then
        check_migration_name "$rel"
      fi
      ;;
  esac
}

check_bash() {
  local cmd branch
  cmd=$(jq -r '.tool_input.command // empty' <<<"$input")
  [ -n "$cmd" ] || return 0

  # .env 系を丸ごと表示するコマンド (grep で特定のキーを取り出す使い方は止めない)
  if grep -Eq '(^|[;&|(`[:space:]])(cat|less|more|head|tail|bat|nl|strings|xxd|od|hexdump|open)([[:space:]]+-[^[:space:]]+([[:space:]]+[0-9]+)?)*[[:space:]]+([^;&|[:space:]]*/)?\.env(\.[A-Za-z0-9_-]+)?([[:space:];&|)]|$)' <<<"$cmd" &&
    ! grep -Eq '\.env\.(example|sample|template)' <<<"$cmd"; then
    deny ".env 系のファイルを丸ごと表示するコマンドは実行しない決まりです (AGENTS.md の P0: 秘密情報)。特定のキーが要るなら grep で取り出して変数に入れ、値を出力しないでください。"
  fi

  # main への直接 push
  if grep -Eq '(^|[;&|[:space:]])git([[:space:]]+-C[[:space:]]+[^[:space:]]+)?[[:space:]]+push([[:space:]]|$)' <<<"$cmd"; then
    if grep -Eq 'push([[:space:]]+[^;&|]*)?[[:space:]](\+?(HEAD:)?(refs/heads/)?main)([[:space:];&|]|$)' <<<"$cmd"; then
      deny "main への直接 push は禁止です (ブランチ → PR → ci → squash マージ)。作業ブランチを push してください。"
    fi
    branch=$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || true)
    if [ "$branch" = "main" ] && grep -Eq 'git([[:space:]]+-C[[:space:]]+[^[:space:]]+)?[[:space:]]+push([[:space:]]+(-u|--set-upstream|origin))*[[:space:]]*([;&|]|$)' <<<"$cmd"; then
      deny "現在のブランチが main なので、この git push は main への直接 push になります。作業ブランチを作ってから push してください。"
    fi
  fi
}

case "$tool" in
  Read | Edit | Write | MultiEdit | NotebookEdit) check_file_tool ;;
  Bash) check_bash ;;
esac
exit 0
