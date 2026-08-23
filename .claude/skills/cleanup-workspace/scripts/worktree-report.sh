#!/usr/bin/env bash
# worktree / ローカルブランチ / ビルド成果物の状況をまとめて表示する (読み取り専用。何も消さない)。
#
# 使い方: worktree-report.sh [--no-fetch] [--recent-hours N]
#   - 先に git fetch --prune を実行して、リモートで消えたブランチを [gone] として検出する (--no-fetch で省略)
#   - PR の状態は gh で取得する (squash マージなので git だけではマージ済みか判定できない)
#   - 直近 N 時間 (既定 3) 以内に更新があった worktree、他プロセスが cwd にしている worktree、
#     メインの checkout に無い / 内容が違う .env 系ファイルを持つ worktree は「要確認」にする
#   - メインの checkout (git worktree list の先頭) のセッションから実行する
#   - 出力は実行時点のスナップショット。削除直前の再チェックは remove-worktree.sh が行う
set -uo pipefail

no_fetch=0 recent_hours=3
while [ $# -gt 0 ]; do
  case "$1" in
    --no-fetch) no_fetch=1 ;;
    --recent-hours) shift; recent_hours=${1:-3} ;;
    -h|--help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "不明な引数: $1" >&2; exit 2 ;;
  esac
  shift
done

main_wt=$(git worktree list --porcelain | awk 'NR==1 && /^worktree / {sub(/^worktree /, ""); print}')
[ -n "$main_wt" ] || { echo "git リポジトリ内で実行してください" >&2; exit 1; }
cd "$main_wt"

if [ "$no_fetch" = 0 ]; then
  git fetch --prune --quiet origin || echo "警告: git fetch --prune に失敗 (オフライン?)。[gone] の判定が古い可能性があります" >&2
fi

default_branch=$(git symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')
default_branch=${default_branch:-main}
base_ref="origin/${default_branch}"

have_gh=1
if ! command -v gh >/dev/null 2>&1 || ! gh auth status >/dev/null 2>&1; then
  have_gh=0
  echo "警告: gh が使えないため PR の状態は '?' になります" >&2
fi

# 他プロセスの cwd (worktree を使っているセッションの検出用)。"<pid> <cmd> <cwd>" の行
cwd_table=$(lsof -a -d cwd -Fpcn 2>/dev/null | awk '/^p/{pid=substr($0,2)} /^c/{c=substr($0,2)} /^n/{print pid, c, substr($0,2)}')

size_of() { if [ -d "$1" ]; then du -sh "$1" 2>/dev/null | cut -f1; else echo "-"; fi; }

# <branch> → "#12 MERGED" / "なし" / "?"。同じブランチに複数 PR があれば OPEN > MERGED > CLOSED の順で代表させる
pr_state() {
  [ "$have_gh" = 1 ] || { echo "?"; return; }
  local out
  out=$(gh pr list --head "$1" --state all --limit 10 --json number,state \
    --jq 'map(select(.state=="OPEN"))[0] // map(select(.state=="MERGED"))[0] // .[0] // empty | "#\(.number) \(.state)"' 2>/dev/null) || { echo "?"; return; }
  echo "${out:-なし}"
}

track_of() {
  local t
  t=$(git for-each-ref --format='%(upstream:short) %(upstream:track)' "refs/heads/$1" 2>/dev/null | sed 's/[[:space:]]*$//')
  [ -n "$t" ] && echo "$t" || echo "(未 push)"
}

# <path> → 直近 recent_hours 時間に更新されたファイルがあれば "あり"
recent_of() {
  local hit
  hit=$(find "$1" \( -name target -o -name node_modules -o -name .next -o -name .git \) -prune -o \
        -type f -mmin "-$((recent_hours * 60))" -print -quit 2>/dev/null)
  [ -n "$hit" ] && echo "あり" || echo "-"
}

# <path> → その worktree を cwd にしているプロセス ("pid cmd" をカンマ区切り)
users_of() {
  [ -n "$cwd_table" ] || { echo "-"; return; }
  local u
  u=$(printf '%s\n' "$cwd_table" | awk -v p="$1" 'index($3, p) == 1 && (length($3) == length(p) || substr($3, length(p)+1, 1) == "/") {printf "%s%s %s", (n++ ? ", " : ""), $1, $2}')
  [ -n "$u" ] && echo "$u" || echo "-"
}

# <path> → git 管理外の .env 系のうち、メインの checkout に無い / 内容が違うもの
env_risk_of() {
  local f out=""
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if [ ! -f "${main_wt}/${f}" ]; then out+="${f} (main に無い) "
    elif ! cmp -s "$1/$f" "${main_wt}/${f}"; then out+="${f} (main と内容が違う) "
    fi
  done < <(git -C "$1" status --ignored --porcelain 2>/dev/null | awk '$1=="!!"{print $2}' | grep -E '(^|/)\.env(\.|$)' | grep -v '\.example$')
  [ -n "$out" ] && echo "${out% }" || echo "-"
}

verdict_of() { # <dirty> <pr> <ahead> <recent> <users> <envrisk>
  local dirty=$1 pr=$2 ahead=$3 recent=$4 users=$5 envrisk=$6 base=""
  if [ "$dirty" != "0" ]; then echo "要確認: 未コミットの変更あり"; return; fi
  case "$pr" in
    *MERGED*) base="削除候補 (PR マージ済み)" ;;
    *CLOSED*) base="削除候補 (PR クローズ、未マージ)" ;;
    *OPEN*)   echo "作業中 (残す)"; return ;;
    なし) if [ "$ahead" = "0" ]; then base="PR なしで ${base_ref} と同一 (作った直後? 不要なら削除可)"; else echo "要確認: PR なしで ${ahead} コミット先行"; return; fi ;;
    *) echo "要確認 (PR の状態を取得できず)"; return ;;
  esac
  local warn=""
  [ "$users" != "-" ] && warn+=" / 他プロセスが使用中"
  [ "$recent" != "-" ] && warn+=" / ${recent_hours}h 以内に更新"
  [ "$envrisk" != "-" ] && warn+=" / .env を退避してから"
  case "$base" in
    削除候補*) if [ -n "$warn" ]; then echo "要確認: ${base}${warn}"; else echo "$base"; fi ;;
    *) echo "要確認: ${base}${warn}" ;;
  esac
}

data_vol=/
[ -d /System/Volumes/Data ] && data_vol=/System/Volumes/Data
echo "## ディスク"
df -h "$data_vol" | awk -v v="$data_vol" 'NR==2 {print "- " v " : 空き " $4 " / 全体 " $2 " (" $5 " 使用)"}'
echo "- メイン checkout: ${main_wt} (ブランチ $(git rev-parse --abbrev-ref HEAD), ${base_ref} との差: $(git rev-list --count "${base_ref}..HEAD" 2>/dev/null || echo '?') 先行 / $(git rev-list --count "HEAD..${base_ref}" 2>/dev/null || echo '?') 遅れ)"
echo "- メイン checkout の成果物: target/ $(size_of target), web/node_modules $(size_of web/node_modules), web/.next $(size_of web/.next)"
echo "- その他のキャッシュ: ~/Library/pnpm $(size_of "$HOME/Library/pnpm"), ~/Library/Caches/pnpm $(size_of "$HOME/Library/Caches/pnpm"), ~/.cargo/registry $(size_of "$HOME/.cargo/registry")"
if command -v docker >/dev/null 2>&1; then
  d=$(docker system df --format '{{.Type}}: {{.Size}} (回収可能 {{.Reclaimable}})' 2>/dev/null | tr '\n' ';' | sed 's/;/; /g; s/; $//')
  [ -n "$d" ] && echo "- Docker: ${d}  ※ 未使用でもタグ付きイメージは \`docker image prune -a\` でないと消えない"
fi
echo

echo "## worktree (.claude/worktrees/ など)"
echo
echo "| worktree | ブランチ | PR | 未コミット | ${base_ref} に無いコミット | upstream | 合計 | target | node_modules | .next | ${recent_hours}h 以内の更新 | 使用中プロセス | main に無い .env | 判定 |"
echo "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"

wt_branches=$'\n'
emit_row() {
  local path=$1 branch=$2 name dirty pr ahead track recent users envrisk verdict
  [ "$path" = "$main_wt" ] && return
  name=${path#"${main_wt}/"}
  [ "$branch" != "(detached)" ] && wt_branches+="${branch}"$'\n'
  if [ ! -d "$path" ]; then
    printf '| %s | %s | - | - | - | - | - | - | - | - | - | - | - | %s |\n' "$name" "$branch" "ディレクトリなし → git worktree prune"
    return
  fi
  dirty=$(git -C "$path" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
  recent=$(recent_of "$path"); users=$(users_of "$path"); envrisk=$(env_risk_of "$path")
  if [ "$branch" = "(detached)" ]; then
    pr="-"; ahead="-"; track="-"; verdict="要確認 (detached HEAD)"
  else
    pr=$(pr_state "$branch")
    ahead=$(git rev-list --count "${base_ref}..${branch}" 2>/dev/null || echo "?")
    track=$(track_of "$branch")
    verdict=$(verdict_of "$dirty" "$pr" "$ahead" "$recent" "$users" "$envrisk")
  fi
  printf '| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n' \
    "$name" "$branch" "$pr" "$dirty" "$ahead" "$track" "$(size_of "$path")" \
    "$(size_of "$path/target")" "$(size_of "$path/web/node_modules")" "$(size_of "$path/web/.next")" \
    "$recent" "$users" "$envrisk" "$verdict"
}

path=""; branch=""
while IFS= read -r line; do
  case "$line" in
    "worktree "*) path=${line#worktree }; branch="(detached)" ;;
    "branch "*)   branch=${line#branch refs/heads/} ;;
    "")           [ -n "$path" ] && emit_row "$path" "$branch"; path="" ;;
  esac
done < <(git worktree list --porcelain; echo)
echo
echo "※ squash マージ後も「${base_ref} に無いコミット」は 0 にならない。マージ済みかは PR 列で判断する。"
echo "※ 「main に無い .env」があると worktree 削除で失われる。残すなら先にメインの checkout へコピーする。"
echo

echo "## worktree を持たないローカルブランチ"
echo
echo "| ブランチ | PR | ${base_ref} に無いコミット | upstream | 判定 |"
echo "|---|---|---|---|---|"
while IFS= read -r b; do
  [ -z "$b" ] && continue
  [ "$b" = "$default_branch" ] && continue
  case "$wt_branches" in *$'\n'"$b"$'\n'*) continue ;; esac
  pr=$(pr_state "$b")
  ahead=$(git rev-list --count "${base_ref}..${b}" 2>/dev/null || echo "?")
  track=$(track_of "$b")
  case "$pr" in
    *MERGED*) verdict="削除候補 (PR マージ済み → git branch -D)" ;;
    *CLOSED*) verdict="削除候補 (PR クローズ → git branch -D)" ;;
    *OPEN*)   verdict="作業中 (残す)" ;;
    なし) if [ "$ahead" = "0" ]; then verdict="削除候補 (${base_ref} と同一 → git branch -D)"; else verdict="要確認: PR なしで ${ahead} コミット先行"; fi ;;
    *) verdict="要確認 (PR の状態を取得できず)" ;;
  esac
  printf '| %s | %s | %s | %s | %s |\n' "$b" "$pr" "$ahead" "$track" "$verdict"
done < <(git for-each-ref --format='%(refname:short)' refs/heads/)
echo

echo "## 次の一手"
echo "- 「削除候補」の worktree: scripts/remove-worktree.sh <worktree 名>  (未コミット・未マージ・使用中・.env の危険があれば拒否する)"
echo "- 「要確認」: 内容を確認してユーザーに判断を仰ぐ (git -C <path> status --short / git log ${base_ref}..<branch> --oneline / 使用中プロセスなら先にそのセッションを閉じてもらう)"
echo "- 残す worktree のビルド成果物だけ消す: (cd <path> && cargo clean) / rm -rf <path>/web/.next"
