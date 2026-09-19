#!/usr/bin/env bash
# release-notes.sh / bump-version.sh 共通。ファイル名の ASCII 順で取り込む。
load_changelog_fragments() {
  local file LC_ALL=C
  changelog_fragments=()
  for file in "$1"/*.md; do
    [ -f "$file" ] || continue
    [ "${file##*/}" != README.md ] || continue
    if ! awk '
      NF && !seen { seen = 1; if ($0 !~ /^### [^[:space:]]/) bad = 1 }
      /^#( |$)|^##( |$)/ { bad = 1 }
      END { exit (!seen || bad) }' "$file"; then
      echo "::error::${file}: 最初の見出しは ### タイトルとし、# / ## 見出しは入れないでください" >&2
      return 1
    fi
    changelog_fragments+=("$file")
  done
}

print_changelog_fragments() {
  local file
  # macOS の Bash 3 でも空配列を set -u で展開しない。
  [ "${#changelog_fragments[@]}" -gt 0 ] || return 0
  for file in "${changelog_fragments[@]}"; do
    cat "$file"
    printf '\n\n'
  done
}
