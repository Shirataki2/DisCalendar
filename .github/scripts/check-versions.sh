#!/usr/bin/env bash
# web / api / bot のバージョンが揃っているか確かめる (CI の version ジョブと release ワークフローが使う)。
#   .github/scripts/check-versions.sh            # 4 か所が一致するか見て、そのバージョンを出力する
#   .github/scripts/check-versions.sh 3.1.0      # さらに、それが指定のバージョンと一致するかも見る
# バージョンは 3 か所 (+ Cargo.lock) にあり、リリースのたびに揃えて上げる (.claude/skills/release/)
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

expected="${1:-}"

# web/package.json の "version": "x.y.z" (先頭の 1 件。dependencies の中には現れない位置にある)
web=$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' web/package.json | head -1)
# Cargo.toml の [package] 直後の version = "x.y.z"
cargo_version() { awk '/^\[package\]/{p=1;next} p && /^version[[:space:]]*=/{gsub(/[",]/,"",$3); print $3; exit}' "$1"; }
api=$(cargo_version api/Cargo.toml)
bot=$(cargo_version bot/Cargo.toml)
# Cargo.lock の [[package]] name = "..." の次の行の version
lock_version() { awk -v n="name = \"$1\"" '$0==n{getline; gsub(/[",]/,"",$3); print $3; exit}' Cargo.lock; }
lock_api=$(lock_version discalendar-api)
lock_bot=$(lock_version discalendar-bot)

fail=0
# 一覧は stderr に出す (stdout はバージョンだけにして呼び出し側が受け取れるようにする)
report() {
  printf '  %-24s %s\n' "$1" "${2:-(取得できない)}" >&2
  [ -n "${2:-}" ] || fail=1
}

if [ "$web" != "$api" ] || [ "$web" != "$bot" ] || [ "$web" != "$lock_api" ] || [ "$web" != "$lock_bot" ] || [ -z "$web" ]; then
  echo "::error::バージョンが揃っていない (.claude/skills/release/scripts/bump-version.sh で揃えて上げる)"
  fail=1
fi

report "web/package.json" "$web"
report "api/Cargo.toml" "$api"
report "bot/Cargo.toml" "$bot"
report "Cargo.lock (api)" "$lock_api"
report "Cargo.lock (bot)" "$lock_bot"

if [ -n "$expected" ] && [ "$web" != "$expected" ]; then
  echo "::error::バージョンが ${expected} ではない (web/package.json は ${web:-不明})"
  fail=1
fi

[ "$fail" -eq 0 ] || exit 1
echo "$web"
