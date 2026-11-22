#!/usr/bin/env bash
# Stop hook: 作業ブランチで止まる前に、忘れやすいもの (.sqlx/ の更新・rollback・バージョン・上限値のずれ) を検出して返す。
# 中身は .agents/scripts/pre-pr-check.sh --detect-only --no-changelog (コマンドは流さないので数秒で終わる)。
#   - 更新履歴の有無は内部変更でも毎回警告になってしまうので、ここでは見ない (PR 前チェックで見る)
#   - 同じ警告で何度も止めないよう、前回止めた内容と同じなら通す (セッションごとに覚える)
#   - main 上・変更なし・Stop hook からの続き (stop_hook_active) では何もしない
set -u

command -v jq >/dev/null 2>&1 || exit 0
input=$(cat)
[ "$(jq -r '.stop_hook_active // false' <<<"$input" 2>/dev/null)" = "true" ] && exit 0

root=${CLAUDE_PROJECT_DIR:-$(jq -r '.cwd // empty' <<<"$input")}
[ -n "$root" ] && cd "$root" 2>/dev/null || exit 0
[ "$(git rev-parse --abbrev-ref HEAD 2>/dev/null)" != "main" ] || exit 0
git rev-parse --verify -q origin/main >/dev/null || exit 0

out=$(bash .agents/scripts/pre-pr-check.sh --detect-only --no-changelog 2>/dev/null)
findings=$(grep -E '^(警告|エラー): ' <<<"$out" || true)
[ -n "$findings" ] || exit 0

session=$(jq -r '.session_id // "default"' <<<"$input")
state="${TMPDIR:-/tmp}/discalendar-stop-precheck-${session//[^A-Za-z0-9_-]/_}"
digest=$(printf '%s' "$findings" | cksum)
if [ -f "$state" ] && [ "$(cat "$state")" = "$digest" ]; then
  exit 0
fi
printf '%s' "$digest" >"$state"

jq -n --arg r "PR 前チェック (.agents/scripts/pre-pr-check.sh --detect-only) で次が見つかりました。直すか、直さない理由をユーザーに伝えてください:
$findings" '{decision: "block", reason: $r}'
exit 0
