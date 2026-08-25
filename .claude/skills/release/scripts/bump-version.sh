#!/usr/bin/env bash
# リリース準備: web / api / bot のバージョンを揃えて上げる (release スキルの手順 2)。
#   .claude/skills/release/scripts/bump-version.sh 3.1.0
#   .claude/skills/release/scripts/bump-version.sh minor   # 今の版から major / minor / patch を 1 つ上げる
# 書き換えるのは web/package.json / api/Cargo.toml / bot/Cargo.toml / Cargo.lock の 4 か所。
# 変更をコミットするのは呼び出し側 (この手のスクリプトはコミットしない)
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

arg="${1:-}"
if [ -z "$arg" ]; then
  echo "usage: $0 <3.1.0 | major | minor | patch>" >&2
  exit 2
fi

current=$(.github/scripts/check-versions.sh)

case "$arg" in
  major | minor | patch)
    # プレリリース (3.1.0-rc.1) が付いていても数値の 3 つ組だけを見る
    IFS=. read -r cur_major cur_minor cur_patch <<< "${current%%-*}"
    case "$arg" in
      major) next="$((cur_major + 1)).0.0" ;;
      minor) next="${cur_major}.$((cur_minor + 1)).0" ;;
      patch) next="${cur_major}.${cur_minor}.$((cur_patch + 1))" ;;
    esac
    ;;
  *)
    next="$arg"
    ;;
esac

if ! [[ "$next" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "::error::バージョンの形式が <major>.<minor>.<patch>[-<prerelease>] ではない (got: $next)" >&2
  exit 1
fi

# semver の大小比較 ($1 > $2 なら 0)。3.1.0-rc.1 < 3.1.0 (プレリリースは同じ数字の正式版より前) を守る
version_gt() {
  local a_core="${1%%-*}" b_core="${2%%-*}" a_pre="" b_pre=""
  [ "$1" != "$a_core" ] && a_pre="${1#*-}"
  [ "$2" != "$b_core" ] && b_pre="${2#*-}"
  if [ "$a_core" != "$b_core" ]; then
    [ "$(printf '%s\n%s\n' "$a_core" "$b_core" | sort -V | tail -1)" = "$a_core" ]
    return
  fi
  # 数字が同じとき: プレリリース無し > プレリリース有り
  [ -z "$a_pre" ] && [ -n "$b_pre" ] && return 0
  [ -n "$a_pre" ] && [ -z "$b_pre" ] && return 1
  [ "$a_pre" = "$b_pre" ] && return 1
  [ "$(printf '%s\n%s\n' "$a_pre" "$b_pre" | sort -V | tail -1)" = "$a_pre" ]
}

# 番号が戻ると、公開済みのタグと表示バージョンの順序が壊れる (直接指定のタイプミス対策)。
# 本当に戻す必要があるときは 4 か所を手で直す
if ! version_gt "$next" "$current"; then
  if [ "$next" = "$current" ]; then
    echo "::error::今と同じバージョン ($current) が指定されている" >&2
  else
    echo "::error::指定のバージョン ($next) が今の版 ($current) より小さい" >&2
  fi
  exit 1
fi

# web/package.json: 先頭に現れる "version" (dependencies の中には無い)
tmp=$(mktemp) && trap 'rm -f "$tmp"' EXIT
sed -E '0,/"version"[[:space:]]*:/s/("version"[[:space:]]*:[[:space:]]*")[^"]+(")/\1'"$next"'\2/' \
  web/package.json > "$tmp" && cp "$tmp" web/package.json

# Cargo.toml: [package] セクションの version
for manifest in api/Cargo.toml bot/Cargo.toml; do
  awk -v v="$next" '/^\[package\]/{p=1} p && !done && /^version[[:space:]]*=/{sub(/=.*/, "= \"" v "\""); done=1} {print}' \
    "$manifest" > "$tmp" && cp "$tmp" "$manifest"
done

# Cargo.lock: ワークスペースメンバーの version (cargo があれば cargo に任せる)
if command -v cargo > /dev/null 2>&1 && cargo metadata --offline --format-version 1 > /dev/null 2>&1; then
  :
else
  for name in discalendar-api discalendar-bot; do
    awk -v n="name = \"$name\"" -v v="$next" \
      '{ if (prev == n && $0 ~ /^version[[:space:]]*=/) { sub(/=.*/, "= \"" v "\"") } prev = $0; print }' \
      Cargo.lock > "$tmp" && cp "$tmp" Cargo.lock
  done
fi

.github/scripts/check-versions.sh "$next" > /dev/null
echo "${current} -> ${next}"
echo
echo "次の手順 (.claude/skills/release/SKILL.md):"
echo "  1. git switch -c claude/release-v${next} && git add -A && git commit -m \"リリース準備: v${next}\""
echo "  2. PR を作ってマージする (CI の version ジョブで 4 か所の整合を確認する)"
echo "  3. main を最新にして git tag -a \"v${next}\" -m \"v${next}\" && git push origin \"v${next}\""
