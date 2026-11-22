#!/usr/bin/env bash
# PostToolUse hook: Edit / Write したファイルだけを整形する (CI の Biome / rustfmt で落ちないように)。
#   - web/ の ts / tsx / js / jsx / mjs / cjs / json / jsonc / css → biome check --write (整形 + import 整理 + 安全な自動修正)。
#     直しきれない lint の指摘が残ったら、エージェントに返す
#   - *.rs → rustfmt (edition はワークスペースの Cargo.toml と揃える)
# 整形ツールが無い (node_modules 未インストール・クラウドの準備前) ときは何もしない。
set -u

command -v jq >/dev/null 2>&1 || exit 0
input=$(cat)
path=$(jq -r '.tool_input.file_path // empty' <<<"$input" 2>/dev/null) || exit 0
[ -n "$path" ] || exit 0

cwd=$(jq -r '.cwd // empty' <<<"$input")
[ -n "$cwd" ] || cwd=${CLAUDE_PROJECT_DIR:-$(pwd)}
case "$path" in
  /*) ;;
  *) path="$cwd/$path" ;;
esac
[ -f "$path" ] || exit 0
# ./web/... や ../ を含むパス、シンボリックリンク経由のパスも、git の show-toplevel と同じ実パスに揃える
path="$(cd "$(dirname "$path")" 2>/dev/null && pwd -P)/$(basename "$path")" || exit 0
# ファイルを含む checkout (worktree ならその worktree) のルートを基準にする
root=$(git -C "$(dirname "$path")" rev-parse --show-toplevel 2>/dev/null) || exit 0

feedback() {
  jq -n --arg c "$1" '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: $c}}'
  exit 0
}

case "$path" in
  "$root"/web/*.ts | "$root"/web/*.tsx | "$root"/web/*.js | "$root"/web/*.jsx | "$root"/web/*.mjs | "$root"/web/*.cjs | \
    "$root"/web/*.json | "$root"/web/*.jsonc | "$root"/web/*.css)
    biome="$root/web/node_modules/.bin/biome"
    [ -x "$biome" ] || exit 0
    # biome.json の files.includes / .gitignore で対象外のファイルは黙って飛ばす
    if ! out=$(cd "$root/web" && "$biome" check --write --no-errors-on-unmatched --colors=off "$path" 2>&1); then
      feedback "biome check --write で直しきれない指摘が残っています ($(basename "$path"))。CI の pnpm lint で落ちるので直してください:
$(printf '%s\n' "$out" | head -60)"
    fi
    ;;
  *.rs)
    command -v rustfmt >/dev/null 2>&1 || exit 0
    # ファイルを直接渡すと mod 宣言を辿って子モジュール (未編集のファイル) まで書き換えるので、
    # 標準入力から整形して編集したファイルだけに書き戻す (設定ファイルはファイルのディレクトリから探させる)
    tmp=$(mktemp) || exit 0
    if ! out=$(cd "$(dirname "$path")" && rustfmt --edition 2024 --emit stdout <"$path" 2>&1 >"$tmp"); then
      rm -f "$tmp"
      feedback "rustfmt が失敗しました ($(basename "$path"))。構文エラーの可能性があります:
$(printf '%s\n' "$out" | head -40)"
    fi
    if [ -s "$tmp" ] && ! cmp -s "$tmp" "$path"; then
      cat "$tmp" >"$path"
    fi
    rm -f "$tmp"
    ;;
esac
exit 0
