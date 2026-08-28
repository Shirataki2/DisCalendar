#!/usr/bin/env bash
# 更新履歴 (web/src/content/changelog.mdx) からリリースノートに使うエントリを取り出す
# (release.yml が GitHub Release の本文に使う)。
#   .github/scripts/release-notes.sh [タグ] [changelog のパス]
# 更新履歴はバージョン見出し「## vX.Y.Z (YYYY年M月D日)」ごとにエントリをまとめている
# (見出しはリリース準備時に bump-version.sh が挿入する) ので、正式版のタグならそのバージョンの節を出す。
# タグを省いたとき / プレリリース (v3.1.0-rc.1) のときは、まだバージョン見出しが付いていない
# 先頭の未リリース分 (最初の ## より上のエントリ) を出す。
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

tag="${1:-}"
file="${2:-web/src/content/changelog.mdx}"

if [ -n "$tag" ] && [[ ! "$tag" =~ ^v[0-9] ]]; then
  echo "::error::タグは v3.1.0 の形式で渡す (got: $tag)" >&2
  exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if [ -n "$tag" ] && [[ "$tag" != *-* ]]; then
  # 正式版: そのバージョンの節の本文。見出し行そのものは Release のタイトルと重複するので出さない。
  # 見出しが無ければ何も出さない (利用者に見える変更が無いリリース。文言は release.yml 側で補う)
  awk -v tag="$tag" '
    /^## / { keep = ($0 ~ "^## " tag "( |$)") }
    keep && !/^## /' "$file" > "$tmp/section.md"
else
  # 未リリース分: 最初のバージョン見出し (## ...) より上にある、最初のエントリ (### ...) 以降。
  # それより前の MDX コメントや前書きは出さない
  awk '/^## / { exit } /^### / { picked = 1 } picked' "$file" > "$tmp/section.md"
fi

# 更新履歴の中のリンクはサイト内の絶対パス (/docs/...) なので、GitHub 上でも辿れるよう URL に直す
sed -E 's#\]\(/#](https://discalendar.app/#g' "$tmp/section.md" > "$tmp/notes.md"

# 先頭・末尾の空行を落とす (節の切り出しは見出しの前後の空行ごと拾うため)
awk '{ lines[NR] = $0 } END { first = 0; last = 0
       for (i = 1; i <= NR; i++) if (lines[i] ~ /[^[:space:]]/) { if (!first) first = i; last = i }
       for (i = first; first && i <= last; i++) print lines[i] }' "$tmp/notes.md"
