#!/usr/bin/env bash
# pre-tool-guard.sh の判定を確かめる (CI の version ジョブと手元で実行する)。
#   bash .claude/hooks/tests/pre-tool-guard-test.sh
set -euo pipefail

root=$(git rev-parse --show-toplevel)
hook="$root/.claude/hooks/pre-tool-guard.sh"
export CLAUDE_PROJECT_DIR=$root
# マイグレーションの版は JST の実時刻で付けているので、CI (UTC) でも同じ基準で比べる
export TZ=Asia/Tokyo
fail=0

decision() {
  local out
  out=$(printf '%s' "$1" | bash "$hook")
  if [ -z "$out" ]; then
    echo pass
  else
    jq -r '.hookSpecificOutput.permissionDecision' <<<"$out"
  fi
}

expect() {
  local want=$1 name=$2 json=$3 got
  got=$(decision "$json")
  if [ "$got" = "$want" ]; then
    echo "ok   $name"
  else
    echo "失敗 $name (期待: $want, 実際: $got)" >&2
    fail=1
  fi
}

file_json() { jq -nc --arg t "$1" --arg p "$2" '{tool_name: $t, tool_input: {file_path: $p}}'; }
bash_json() { jq -nc --arg c "$1" '{tool_name: "Bash", tool_input: {command: $c}}'; }

existing=$(git -C "$root" ls-files 'api/migrations/*.sql' | head -1)
today=$(date "+%Y%m%d")
now=$(date "+%Y%m%d%H%M%S")

# マイグレーション
expect deny "既存マイグレーションの Edit" "$(file_json Edit "$root/$existing")"
expect deny "既存マイグレーションの Write (上書き)" "$(file_json Write "$existing")"
expect pass "現在時刻の新しいマイグレーション" "$(file_json Write "$root/api/migrations/${now}_add_example.sql")"
expect deny "連番の時刻 (000001)" "$(file_json Write "$root/api/migrations/${today}000001_add_example.sql")"
expect deny "既存より古い版" "$(file_json Write "$root/api/migrations/20200101123456_add_example.sql")"
expect deny "1 日以上先の版" "$(file_json Write "$root/api/migrations/29991231123456_add_example.sql")"
expect deny "形式の違う名前" "$(file_json Write "$root/api/migrations/add_example.sql")"
expect pass "rollback は対象外" "$(file_json Write "$root/api/rollback/${now}_drop_example.sql")"

# .env 系
expect deny ".env.local の Read" "$(file_json Read "$root/web/.env.local")"
expect deny "api/.env の Edit" "$(file_json Edit "api/.env")"
expect pass ".env.example の Read" "$(file_json Read "$root/web/.env.example")"
expect pass "env.ts の Read" "$(file_json Read "$root/web/src/lib/env.ts")"
expect deny "cat api/.env" "$(bash_json 'cat api/.env')"
expect deny "head -n 5 web/.env.local" "$(bash_json 'cd web && head -n 5 .env.local')"
expect pass "grep でキーを取り出す" "$(bash_json "DATABASE_URL=\$(grep '^DATABASE_URL=' api/.env | cut -d= -f2-) cargo test")"
expect pass "cat .env.example" "$(bash_json 'cat web/.env.example')"

# git push
expect deny "git push origin main" "$(bash_json 'git push origin main')"
expect deny "git push origin HEAD:main" "$(bash_json 'git push origin HEAD:main')"
expect deny "git push -f origin main" "$(bash_json 'git add -A && git push -f origin main')"
expect pass "作業ブランチの push" "$(bash_json 'git push -u origin claude/issue-1-example')"
expect pass "main を含む別名のブランチ" "$(bash_json 'git push origin fix-main-layout')"

# 判定できない入力は通す
expect pass "壊れた JSON" 'not json'
expect pass "対象外のツール" '{"tool_name":"Grep","tool_input":{"pattern":"x"}}'

exit "$fail"
