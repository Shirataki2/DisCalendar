#!/usr/bin/env bash
# コマンド文字列は hook に渡す入力そのものなので、単一引用符の中の $(...) は展開しない
# shellcheck disable=SC2016
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
bash_json() { jq -nc --arg c "$1" --arg d "${2:-$root}" '{tool_name: "Bash", cwd: $d, tool_input: {command: $c}}'; }

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
# CLAUDE_PROJECT_DIR が別の checkout を指していても、ファイルを含む checkout (worktree) で判定する
CLAUDE_PROJECT_DIR=/ expect deny "別の checkout から見た既存マイグレーション" "$(file_json Edit "$root/$existing")"
expect pass "rollback は対象外" "$(file_json Write "$root/api/rollback/${now}_drop_example.sql")"
# . / .. を含むパスも同じファイルとして判定する
expect deny "./ つきの相対パス" "$(file_json Edit "./$existing")"
expect deny "途中に /./ を含む絶対パス" "$(file_json Edit "$root/./$existing")"
expect deny "途中に /../ を含む絶対パス" "$(file_json Edit "$root/api/../$existing")"
expect deny "./ つきの新規ファイル (古い版)" "$(file_json Write "./api/migrations/20200101123456_add_example.sql")"

# .env 系
expect deny ".env.local の Read" "$(file_json Read "$root/web/.env.local")"
expect deny "api/.env の Edit" "$(file_json Edit "api/.env")"
expect pass ".env.example の Read" "$(file_json Read "$root/web/.env.example")"
expect pass "env.ts の Read" "$(file_json Read "$root/web/src/lib/env.ts")"
expect deny "cat api/.env" "$(bash_json 'cat api/.env')"
expect deny "head -n 5 web/.env.local" "$(bash_json 'cd web && head -n 5 .env.local')"
expect pass "grep でキーを取り出す" "$(bash_json "DATABASE_URL=\$(grep '^DATABASE_URL=' api/.env | cut -d= -f2-) cargo test")"
expect pass "cat .env.example" "$(bash_json 'cat web/.env.example')"
expect deny "引用符つきの cat \"api/.env\"" "$(bash_json 'cat "api/.env"')"
expect deny "引用符つきの cat '.env'" "$(bash_json "cat '.env'")"
expect deny "ひな形と並べた cat" "$(bash_json 'cat api/.env web/.env.example')"
expect deny "ひな形の後の cat" "$(bash_json 'cat web/.env.example && cat api/.env')"
expect deny "コマンド置換の中の cat" "$(bash_json 'X=$(cat api/.env) echo ok')"
expect deny "グロブ .env*" "$(bash_json 'cat .env*')"
expect pass "ひな形同士の cat" "$(bash_json 'cat web/.env.example api/.env.example')"

# git push
expect deny "git push origin main" "$(bash_json 'git push origin main')"
expect deny "git push origin HEAD:main" "$(bash_json 'git push origin HEAD:main')"
expect deny "git push -f origin main" "$(bash_json 'git add -A && git push -f origin main')"
expect deny "src:main の refspec" "$(bash_json 'git push origin feature:main')"
expect deny "main:main の refspec" "$(bash_json 'git push origin main:main')"
expect deny "refs/heads/main の宛先" "$(bash_json 'git push origin +claude/foo:refs/heads/main')"
expect deny "git -C で main を push" "$(bash_json 'git -C /tmp/x push origin main')"
expect pass "作業ブランチの push" "$(bash_json 'git push -u origin claude/issue-1-example')"
expect pass "main を含む別名のブランチ" "$(bash_json 'git push origin fix-main-layout')"

# main 上での引数なし / HEAD / コマンド置換の push (main にいる一時リポジトリで確かめる)
tmp_repo=$(mktemp -d)
git -C "$tmp_repo" init -q -b main
expect deny "main 上の git push" "$(bash_json 'git push' "$tmp_repo")"
expect deny "main 上の git push origin HEAD" "$(bash_json 'git push origin HEAD' "$tmp_repo")"
expect deny "main 上のコマンド置換" "$(bash_json 'git push -u origin "$(git rev-parse --abbrev-ref HEAD)"' "$tmp_repo")"
expect deny "cd してから push" "$(bash_json "cd $tmp_repo && git push")"
git -C "$tmp_repo" switch -q -c claude/issue-1-example
expect pass "作業ブランチ上のコマンド置換" "$(bash_json 'git push -u origin "$(git rev-parse --abbrev-ref HEAD)"' "$tmp_repo")"
# ローカルに main がある状態で、全ブランチを送る push
git -C "$tmp_repo" -c user.name=test -c user.email=test@example.invalid commit -q --allow-empty -m init
git -C "$tmp_repo" branch -q main
expect deny "git push --all" "$(bash_json 'git push --all origin' "$tmp_repo")"
expect deny "git push --mirror" "$(bash_json 'git push --mirror origin' "$tmp_repo")"
expect pass "git push --tags" "$(bash_json 'git push --tags origin' "$tmp_repo")"
rm -rf "$tmp_repo"

# 判定できない入力は通す
expect pass "壊れた JSON" 'not json'
expect pass "対象外のツール" '{"tool_name":"Grep","tool_input":{"pattern":"x"}}'

exit "$fail"
