#!/usr/bin/env bash
# 更新履歴 (web/src/content/changelog.mdx) から、前のリリース以降に追記されたエントリだけを取り出す
# (release.yml が GitHub Release の本文に使う)。
#   .github/scripts/release-notes.sh [前のタグ] [changelog のパス]
# 更新履歴は新しいエントリを先頭に足していく (## で始まる見出し単位) ので、
# 前のタグ時点の見出しに無い ## セクションが「今回の分」になる。前のタグを省いたら全エントリを出す。
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

prev="${1:-}"
file="${2:-web/src/content/changelog.mdx}"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
: > "$tmp/prev.mdx"
if [ -n "$prev" ]; then
  git show "${prev}:${file}" > "$tmp/prev.mdx" 2>/dev/null || : > "$tmp/prev.mdx"
fi

# 前のタグ時点の見出しを覚えておき、そこに無い ## セクションだけを出す。
# 先頭の MDX コメントや前書きは最初の ## より前にあるので (keep が未設定のうちは) 出力されない。
# 更新履歴の中のリンクはサイト内の絶対パス (/docs/...) なので、GitHub 上でも辿れるよう URL に直す
# (前のタグが無いときは prev.mdx が空になる。NR==FNR だと 1 ファイル目が 0 行のときに
#  2 ファイル目の 1 行目まで拾ってしまうので、FILENAME で判定する)
awk -v prevfile="$tmp/prev.mdx" '
     FILENAME == prevfile { if ($0 ~ /^## /) seen[$0]=1; next }
     /^## / { keep = ($0 in seen) ? 0 : 1 }
     keep' "$tmp/prev.mdx" "$file" \
  | sed -E 's#\]\(/#](https://discalendar.app/#g' \
  > "$tmp/notes.md"

# 末尾の空行を落とす
awk '{ lines[NR] = $0 } END { last = 0; for (i = 1; i <= NR; i++) if (lines[i] ~ /[^[:space:]]/) last = i
       for (i = 1; i <= last; i++) print lines[i] }' "$tmp/notes.md"
