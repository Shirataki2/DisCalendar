#!/usr/bin/env bash
# Claude Code のクラウドセッションで、Claude / Codex 共通の環境セットアップを呼ぶ。
# ローカルの Claude Code セッションでは何もしない。
set -u

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

if [ -n "${CLAUDE_PROJECT_DIR:-}" ]; then
  repo_root=$CLAUDE_PROJECT_DIR
elif repo_root=$(git rev-parse --show-toplevel 2>/dev/null); then
  :
else
  repo_root=$(cd "$(dirname "$0")/../.." && pwd)
fi
exec bash "$repo_root/.agents/scripts/setup-cloud-environment.sh"
