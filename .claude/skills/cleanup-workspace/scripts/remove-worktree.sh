#!/usr/bin/env bash
# worktree を安全に削除する。次のどれかに当てはまれば拒否する (exit 3):
#   未コミットの変更 / PR が OPEN / PR なしで origin/main より先行 / PR なしで直近 3 時間以内に更新 /
#   マージ済み PR の head より後にローカルコミットがある / 他プロセスが cwd にしている /
#   メインの checkout に無い・内容が違う .env 系ファイルがある / detached HEAD
#   (ディレクトリが既に無い prunable な worktree でも、ブランチ側の確認は同じように行う)
#
# 使い方: remove-worktree.sh <worktree のパス | .claude/worktrees/ 配下の名前> [--force] [--keep-branch]
#   --force       安全チェックを無視して消す (ユーザーの明示的な了解を得てから使う。他プロセス使用中だけは --force でも拒否)
#   --keep-branch worktree だけ消してローカルブランチは残す
# 消すのは worktree ディレクトリ (target/ や node_modules も一緒に消える) とローカルブランチだけ。
# リモートブランチには触れない (残っていれば最後に案内する)。
# メインの checkout (git worktree list の先頭) のセッションから実行する。対象 worktree の中からは実行できない。
set -euo pipefail

usage() { sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; }

target="" force=0 keep_branch=0 recent_hours=3
for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    --keep-branch) keep_branch=1 ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "不明なオプション: $arg" >&2; usage >&2; exit 2 ;;
    *) target=$arg ;;
  esac
done
[ -n "$target" ] || { usage >&2; exit 2; }

orig_pwd=$(pwd -P)
main_wt=$(git worktree list --porcelain | awk 'NR==1 && /^worktree / {sub(/^worktree /, ""); print}')
[ -n "$main_wt" ] || { echo "git リポジトリ内で実行してください" >&2; exit 1; }
cd "$main_wt"

# 名前だけなら .claude/worktrees/<name> (ディレクトリが既に消えている prunable なものも名前で指せるようにする)
case "$target" in
  */*) ;;
  *) [ -e "$target" ] || target=".claude/worktrees/${target}" ;;
esac
if [ -d "$target" ]; then
  abs=$(cd "$target" && pwd -P)
else
  case "$target" in /*) abs=$target ;; *) abs="${main_wt}/${target}" ;; esac
fi

# git worktree list から該当レコードを探す (ディレクトリが消えている prunable なものも拾う)
branch="" head_sha="" found=0 path="" wb="" wh=""
while IFS= read -r line; do
  case "$line" in
    "worktree "*) path=${line#worktree }; wb="(detached)"; wh="" ;;
    "HEAD "*)     wh=${line#HEAD } ;;
    "branch "*)   wb=${line#branch refs/heads/} ;;
    "") if [ -n "$path" ] && [ "$path" = "$abs" ]; then found=1; branch=$wb; head_sha=$wh; fi; path="" ;;
  esac
done < <(git worktree list --porcelain; echo)

[ "$found" = 1 ] || { echo "git worktree list に ${abs} がありません" >&2; exit 1; }
[ "$abs" = "$main_wt" ] && { echo "メインの checkout は削除できません" >&2; exit 1; }
case "$orig_pwd/" in "$abs"/*) echo "このセッションは対象 worktree の中にいます。ExitWorktree (keep) でメインの checkout に戻ってから実行してください" >&2; exit 1 ;; esac

default_branch=$(git symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')
base_ref="origin/${default_branch:-main}"

# --- ブランチ側の確認 (worktree のディレクトリが無くても行う) ---
pr="?" pr_head=""
[ "$branch" = "(detached)" ] && pr="-"
if [ "$branch" != "(detached)" ] && command -v gh >/dev/null 2>&1; then
  # 同じブランチ名の PR が複数あれば OPEN > MERGED > CLOSED の順で代表させ、その head SHA も取る
  pr_json=$(gh pr list --head "$branch" --state all --limit 10 --json number,state,headRefOid \
    --jq 'map(select(.state=="OPEN"))[0] // map(select(.state=="MERGED"))[0] // .[0] // empty | "#\(.number) \(.state) \(.headRefOid)"' 2>/dev/null || echo "?")
  if [ -z "$pr_json" ]; then pr="なし"
  elif [ "$pr_json" = "?" ]; then pr="?"
  else pr=$(echo "$pr_json" | cut -d' ' -f1-2); pr_head=$(echo "$pr_json" | cut -d' ' -f3); fi
fi
ahead="-"
[ "$branch" != "(detached)" ] && ahead=$(git rev-list --count "${base_ref}..${branch}" 2>/dev/null || echo "?")
# マージ / クローズ済み PR の head より後にローカルでコミットを足していないか
after_pr=""
if [ -n "$pr_head" ] && [ "$branch" != "(detached)" ]; then
  case "$pr" in
    *MERGED*|*CLOSED*)
      if ! git cat-file -e "${pr_head}^{commit}" 2>/dev/null || ! git merge-base --is-ancestor "$pr_head" "$branch" 2>/dev/null; then
        after_pr="PR ${pr} の head (${pr_head:0:7}) がローカルブランチに含まれていません (rebase / amend 後?)"
      else
        n=$(git rev-list --count "${pr_head}..${branch}" 2>/dev/null || echo 0)
        [ "$n" != "0" ] && after_pr="PR ${pr} のマージ後にローカルで ${n} コミット追加されています (git log ${pr_head:0:7}..${branch})"
      fi ;;
  esac
fi

if [ ! -d "$abs" ]; then
  echo "対象: ${abs} (ディレクトリは既にありません → git worktree prune)"
  echo "ブランチ: ${branch} / HEAD: ${head_sha:0:7} / PR: ${pr} / ${base_ref} に無いコミット: ${ahead}"
  # detached HEAD は worktree の登録 (.git/worktrees/<name>/HEAD) が最後の参照かもしれない。
  # どのブランチ / リモートにも含まれないコミットなら prune で unreachable になるので止める
  if [ "$branch" = "(detached)" ] && [ -n "$head_sha" ] && [ "$force" = 0 ] \
     && [ -z "$(git for-each-ref --contains "$head_sha" refs/heads refs/remotes 2>/dev/null)" ]; then
    echo "削除を中止しました: detached HEAD のコミット ${head_sha:0:7} はどのブランチにも含まれておらず、prune すると失われます。" >&2
    echo "残すなら先に退避してから再実行してください: git branch rescue/$(basename "$abs") ${head_sha}" >&2
    echo "不要だと確認できたら --force で prune します" >&2
    exit 3
  fi
  if [ "$branch" = "(detached)" ]; then
    # detached ならブランチは無いので、登録を消すだけ (上の未参照チェックを通過している)
    git worktree prune
    echo "worktree の登録を消しました: ${abs}"
    exit 0
  fi
  if [ "$force" = 0 ]; then
    reasons=()
    case "$pr" in
      *MERGED*|*CLOSED*) ;;
      *OPEN*) reasons+=("PR ${pr} がまだ開いています") ;;
      なし) [ "$ahead" != "0" ] && reasons+=("PR が無いのに ${base_ref} に無いコミットが ${ahead} 件あります (このブランチが最後の参照かもしれません)") ;;
      *) reasons+=("PR の状態を確認できませんでした (gh 未認証?)") ;;
    esac
    [ -n "$after_pr" ] && reasons+=("$after_pr")
    if [ ${#reasons[@]} -gt 0 ]; then
      echo "worktree の登録は消しますが、ブランチ ${branch} は残します:" >&2
      for r in "${reasons[@]}"; do echo "  - $r" >&2; done
      echo "ブランチも消すなら、内容を確認しユーザーの了解を得たうえで --force を付けて再実行してください" >&2
      git worktree prune
      exit 3
    fi
  fi
  git worktree prune
else
  # 他プロセスの cwd になっていないか (別の Claude Code セッション、シェル、dev サーバーなど)
  users=""
  if command -v lsof >/dev/null 2>&1; then
    users=$(lsof -a -d cwd -Fpcn 2>/dev/null | awk -v p="$abs" '/^p/{pid=substr($0,2)} /^c/{c=substr($0,2)} /^n/{n=substr($0,2); if (index(n,p)==1 && (length(n)==length(p) || substr(n,length(p)+1,1)=="/")) printf "%s%s %s", (k++ ? ", " : ""), pid, c}') || users=""
  else
    echo "警告: lsof が無いため、他プロセスがこの worktree を使用中かは確認できません" >&2
  fi
  if [ -n "$users" ]; then
    echo "削除を中止しました: 他のプロセスがこの worktree を cwd にしています (${users})。" >&2
    echo "そのセッション / シェル / サーバーを閉じてもらってから再実行してください (--force でも消しません)" >&2
    exit 3
  fi

  dirty=$(git -C "$abs" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
  recent=$(find "$abs" \( -name target -o -name node_modules -o -name .next -o -name .git \) -prune -o -type f -mmin "-$((recent_hours * 60))" -print -quit 2>/dev/null)
  env_risk=""
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if [ ! -f "${main_wt}/${f}" ]; then env_risk+="  - ${f}: メインの checkout に無い (残すなら cp \"${abs}/${f}\" \"${main_wt}/${f}\")"$'\n'
    elif ! cmp -s "$abs/$f" "${main_wt}/${f}"; then env_risk+="  - ${f}: メインの checkout と内容が違う (diff を確認)"$'\n'
    fi
  done < <(git -C "$abs" status --ignored --porcelain 2>/dev/null | awk '$1=="!!"{print $2}' | grep -E '(^|/)\.env(\.|$)' | grep -v '\.example$')

  echo "対象: ${abs}"
  echo "ブランチ: ${branch} / PR: ${pr} / 未コミット: ${dirty} / ${base_ref} に無いコミット: ${ahead}"

  if [ "$force" = 0 ]; then
    reasons=()
    [ "$dirty" != "0" ] && reasons+=("未コミットの変更が ${dirty} 件あります")
    case "$pr" in
      *MERGED*|*CLOSED*) ;;
      *OPEN*) reasons+=("PR ${pr} がまだ開いています") ;;
      なし)
        [ "$ahead" != "0" ] && reasons+=("PR が無いのに ${base_ref} に無いコミットが ${ahead} 件あります (push / PR 作成前?)")
        [ -n "$recent" ] && reasons+=("PR が無く、直近 ${recent_hours} 時間以内に更新されています (作業中の可能性)") ;;
      *) reasons+=("PR の状態を確認できませんでした (gh 未認証?)") ;;
    esac
    [ -n "$after_pr" ] && reasons+=("$after_pr")
    [ "$branch" = "(detached)" ] && reasons+=("detached HEAD です")
    [ -n "$env_risk" ] && reasons+=("git 管理外の .env 系ファイルが worktree ごと消えます:"$'\n'"${env_risk%$'\n'}")
    if [ ${#reasons[@]} -gt 0 ]; then
      echo "削除を中止しました:" >&2
      for r in "${reasons[@]}"; do echo "  - $r" >&2; done
      echo "内容を確認し (必要なら .env を退避し)、ユーザーの了解を得たうえで --force を付けて再実行してください" >&2
      exit 3
    fi
    git worktree remove "$abs"
  else
    [ -n "$env_risk" ] && { echo "注意: 次の .env 系ファイルも消えます:"; printf '%s' "$env_risk"; }
    git worktree remove --force "$abs"
  fi
  echo "worktree を削除しました: ${abs}"
fi

if [ "$keep_branch" = 0 ] && [ -n "$branch" ] && [ "$branch" != "(detached)" ] && git show-ref --verify --quiet "refs/heads/${branch}"; then
  git branch -D "$branch"
  echo "ローカルブランチを削除しました: ${branch}"
fi
git worktree prune

if [ -n "$branch" ] && [ "$branch" != "(detached)" ] && git ls-remote --exit-code --heads origin "$branch" >/dev/null 2>&1; then
  echo "注意: origin/${branch} はまだリモートに残っています。消す場合はユーザーに確認のうえ: git push origin --delete ${branch}"
fi
