#!/usr/bin/env bash
# Codex デスクトップアプリの Local environment から呼ぶ worktree 用セットアップ。
# ignored な .env は .worktreeinclude がコピーするため、ここでは依存だけを準備する。
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if command -v corepack >/dev/null 2>&1; then
  corepack enable >/dev/null 2>&1 || true
fi

if [ -f web/package.json ]; then
  (cd web && pnpm install --frozen-lockfile)
fi

# Rust の依存は worktree 間で同じ cargo cache を使える。ネットワークが使えない場合でも
# web の準備までを無駄にしないよう、fetch の失敗は警告に留める。
if command -v cargo >/dev/null 2>&1; then
  cargo fetch || echo "警告: cargo fetch に失敗しました。必要なときに再実行してください" >&2
fi

