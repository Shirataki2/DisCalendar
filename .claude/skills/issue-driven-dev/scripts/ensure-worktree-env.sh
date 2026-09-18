#!/usr/bin/env bash
# main checkout にだけある .env 系を、指定した worktree の不足分へコピーする。
set -euo pipefail

target=${1:-$(git rev-parse --show-toplevel)}
main_wt=$(git worktree list --porcelain | awk 'NR==1 && /^worktree / {sub(/^worktree /, ""); print}')
[ -n "$main_wt" ] || { echo "git リポジトリ内で実行してください" >&2; exit 1; }

echo "設定ファイルの確認:"
copied=0
while IFS= read -r source; do
  rel=${source#"${main_wt}/"}
  dest="${target}/${rel}"
  if [ -e "$dest" ] || [ -L "$dest" ]; then continue; fi
  mkdir -p "$(dirname "$dest")"
  cp "$source" "$dest"
  echo "  - ${rel} (main checkout からコピー)"
  copied=$((copied + 1))
done < <(find "$main_wt" -maxdepth 2 \( -path "$main_wt/.claude" -o -path "$main_wt/tmp" -o -path "$main_wt/target" -o -path '*/node_modules' \) -prune -o \
         \( -name '.env' -o -name '.env.*' \) ! -name '.env.example' -type f -print)
[ "$copied" -gt 0 ] || echo "  (不足なし、または main checkout にコピー元なし)"
