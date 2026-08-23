#!/usr/bin/env bash
# Issue 用の worktree を作り、git 管理外の設定ファイル (.env 系) をメイン checkout からコピーする。
#
# 使い方: setup-worktree.sh <Issue番号> <slug> [--install] [--branch <既存ブランチ>]
#   - .claude/worktrees/issue-<N>-<slug> に、origin/main 起点のブランチ claude/issue-<N>-<slug> を作る
#   - --branch を指定すると、新規作成せずその既存ブランチ (ローカル or origin) を checkout する
#   - --install を付けると web/ で pnpm install --frozen-lockfile も実行する
#   - 既に同じ worktree があればそのまま使う (冪等)
# 終わったら表示されるパスを EnterWorktree の path に渡してセッションを移す。
set -euo pipefail

usage() { sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; }

issue="" slug="" install=0 branch=""
while [ $# -gt 0 ]; do
  case "$1" in
    --install) install=1 ;;
    --branch) shift; branch=${1:-} ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "不明なオプション: $1" >&2; usage >&2; exit 2 ;;
    *) if [ -z "$issue" ]; then issue=$1; elif [ -z "$slug" ]; then slug=$1; else echo "引数が多すぎます" >&2; exit 2; fi ;;
  esac
  shift
done

if ! [[ "$issue" =~ ^[0-9]+$ ]]; then echo "Issue 番号 (数字) を指定してください" >&2; usage >&2; exit 2; fi
if ! [[ "$slug" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then echo "slug は英小文字・数字・ハイフンで指定してください (例: bot-tasks)" >&2; exit 2; fi

# メインの checkout (git worktree list の先頭) を基準にする
main_wt=$(git worktree list --porcelain | awk 'NR==1 && /^worktree / {sub(/^worktree /, ""); print}')
[ -n "$main_wt" ] || { echo "git リポジトリ内で実行してください" >&2; exit 1; }
cd "$main_wt"

name="issue-${issue}-${slug}"
path=".claude/worktrees/${name}"
abs_path="${main_wt}/${path}"
[ -n "$branch" ] || branch="claude/${name}"

git fetch --quiet origin || echo "警告: git fetch に失敗しました (オフライン?)。ローカルの origin/main を起点にします" >&2

if [ -d "$abs_path" ] && git worktree list --porcelain | grep -qx "worktree ${abs_path}"; then
  echo "既存の worktree を使います: ${abs_path}"
elif git show-ref --verify --quiet "refs/heads/${branch}"; then
  echo "ローカルブランチ ${branch} を worktree に checkout します"
  git worktree add "$path" "$branch"
elif git show-ref --verify --quiet "refs/remotes/origin/${branch}"; then
  echo "origin/${branch} を追跡するブランチを worktree に作ります"
  git worktree add --track -b "$branch" "$path" "origin/${branch}"
else
  echo "origin/main 起点で ${branch} を作ります"
  git worktree add --no-track -b "$branch" "$path" origin/main
fi

# .env 系 (gitignore 対象) をコピーする。既にあるものは上書きしない
echo "設定ファイルのコピー:"
copied=0
while IFS= read -r f; do
  rel=${f#./}
  dest="${abs_path}/${rel}"
  if [ -e "$dest" ]; then continue; fi
  mkdir -p "$(dirname "$dest")"
  cp "$f" "$dest"
  echo "  - ${rel}"
  copied=$((copied + 1))
done < <(find . -maxdepth 2 \( -path ./.claude -o -path ./tmp -o -path ./target -o -path '*/node_modules' \) -prune -o \
           \( -name '.env' -o -name '.env.*' \) ! -name '.env.example' -type f -print)
[ "$copied" -gt 0 ] || echo "  (コピーするものはありませんでした)"

if [ "$install" = 1 ] && [ -f "${abs_path}/web/package.json" ]; then
  echo "web/ で pnpm install を実行します"
  (cd "${abs_path}/web" && pnpm install --frozen-lockfile)
fi

cat <<MSG

worktree: ${abs_path}
ブランチ: $(git -C "$abs_path" rev-parse --abbrev-ref HEAD) (HEAD $(git -C "$abs_path" rev-parse --short HEAD))
次: EnterWorktree の path に上のパスを渡してセッションを移す。
    旧実装の参照は絶対パス ${main_wt}/tmp/DisCalendarV2/ を使う (worktree にはコピーされない)。
MSG
