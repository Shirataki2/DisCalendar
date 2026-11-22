#!/usr/bin/env bash
# PreToolUse hook: AGENTS.md の P0 規則のうち、パスとコマンドの形で判定できるものをツール実行前に止める。
#   - api/migrations/ の既存ファイル (git 管理下) の編集 → 拒否 (_sqlx_migrations のチェックサムと照合されるため)
#   - 新しいマイグレーションのファイル名 → 形式・順序・連番の時刻を検査 (`date` で取った実時刻を使う)
#   - .env 系 (*.example などのひな形を除く) の Read / Edit / Write、Bash での丸ごと表示 → 拒否
#   - main への直接 git push → 拒否 (サーバー側のルールセットでも拒否されるが、手前で止める)
# パスの判定は、そのファイルを含む checkout (worktree を含む) のルートを基準にする。
# 判定できない入力 (jq が無い、JSON が壊れている) では何もしない (誤検知で作業を止めない)。
set -u

command -v jq >/dev/null 2>&1 || exit 0
input=$(cat)
tool=$(jq -r '.tool_name // empty' <<<"$input" 2>/dev/null) || exit 0
[ -n "$tool" ] || exit 0

cwd=$(jq -r '.cwd // empty' <<<"$input")
[ -n "$cwd" ] || cwd=${CLAUDE_PROJECT_DIR:-$(pwd)}

deny() {
  jq -n --arg r "$1" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $r}}'
  exit 0
}

# パスを含む checkout のルート (worktree ならその worktree)。まだ無いファイルは、存在する親ディレクトリから求める
checkout_root() {
  local dir=$1
  while [ -n "$dir" ] && [ ! -d "$dir" ]; do
    dir=$(dirname "$dir")
  done
  git -C "$dir" rev-parse --show-toplevel 2>/dev/null
}

# 絶対パスの . / .. を字句的に畳み、存在する部分はシンボリックリンクを解決した実パスにする
# (git の show-toplevel は実パスを返すので、前方一致で比べられるように揃える。/tmp → /private/tmp など)
normalize_path() {
  local IFS=/ parts p out=() norm dir rest=""
  read -r -a parts <<<"$1"
  for p in "${parts[@]}"; do
    case "$p" in
      "" | .) ;;
      ..) [ ${#out[@]} -gt 0 ] && unset 'out[${#out[@]}-1]' ;;
      *) out+=("$p") ;;
    esac
  done
  norm=$(printf '/%s' ${out[@]+"${out[@]}"})
  [ -n "$norm" ] || norm=/
  dir=$norm
  while [ "$dir" != / ] && [ ! -d "$dir" ]; do
    rest="/$(basename -- "$dir")$rest"
    dir=$(dirname -- "$dir")
  done
  dir=$(cd "$dir" 2>/dev/null && pwd -P) || dir=${dir%/}
  printf '%s%s' "${dir%/}" "$rest"
}

# .env 系の秘密ファイルか (.env.example などのひな形は除く。.env* のようなグロブも秘密として扱う)
is_secret_env() {
  local base
  base=$(basename -- "$1")
  case "$base" in
    .env.example | .env.sample | .env.template) return 1 ;;
    .env | .env.* | .env\**) return 0 ;;
  esac
  return 1
}

check_migration_name() {
  local root=$1 rel=$2 base stamp hms latest now_stamp
  base=$(basename "$rel")
  if ! [[ $base =~ ^([0-9]{14})_[a-z0-9_]+\.sql$ ]]; then
    deny "マイグレーションのファイル名は <yyyymmddHHMMSS>_<英小文字とアンダースコア>.sql にしてください (受け取った名前: $base)。時刻は \`date \"+%Y%m%d%H%M%S\"\` で取得した実時刻を使います。"
  fi
  stamp=${BASH_REMATCH[1]}
  hms=${stamp:8:6}
  # 000001 / 000002 … のような連番は禁止 (時刻は実時刻にする)
  if [[ $hms =~ ^0000[0-9][0-9]$ ]] && [ "$hms" != "000000" ]; then
    deny "マイグレーションの時刻部分 ($hms) が連番になっています。\`date \"+%Y%m%d%H%M%S\"\` を実行して得た実時刻をファイル名に使ってください。"
  fi
  # 既存の最新より新しいこと (sqlx は版の昇順で適用する。古い版を後から足すと本番で適用されない)
  latest=$(find "$root/api/migrations" -maxdepth 1 -name '[0-9]*_*.sql' -exec basename {} \; 2>/dev/null | cut -c1-14 | sort | tail -1)
  if [ -n "$latest" ] && { [ "$stamp" \< "$latest" ] || [ "$stamp" = "$latest" ]; }; then
    deny "マイグレーションの版 ($stamp) が既存の最新 ($latest) 以前です。\`date \"+%Y%m%d%H%M%S\"\` で取得した現在時刻を使ってください。"
  fi
  # 未来すぎる版 (1 日より先) も、日付の取り違えとして止める
  now_stamp=$(date -v+1d "+%Y%m%d%H%M%S" 2>/dev/null || date -d '+1 day' "+%Y%m%d%H%M%S" 2>/dev/null || true)
  if [ -n "$now_stamp" ] && [ "$stamp" \> "$now_stamp" ]; then
    deny "マイグレーションの版 ($stamp) が現在時刻より 1 日以上先です。\`date \"+%Y%m%d%H%M%S\"\` で取得した現在時刻を使ってください。"
  fi
}

check_file_tool() {
  local path root rel
  path=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")
  [ -n "$path" ] || return 0
  case "$path" in
    /*) ;;
    *) path="$cwd/$path" ;;
  esac
  # ./api/migrations/... や api/../api/migrations/... でも同じファイルとして判定する
  path=$(normalize_path "$path")

  if is_secret_env "$path"; then
    deny ".env 系のファイル ($(basename "$path")) はエージェントから読み書きしない決まりです (AGENTS.md の P0: 秘密情報)。必要な値は .env.example を見るか、ユーザーに設定を依頼してください。"
  fi

  [ "$tool" = "Read" ] && return 0
  root=$(checkout_root "$(dirname "$path")") || return 0
  [ -n "$root" ] || return 0
  rel=${path#"$root"/}

  case "$rel" in
    api/migrations/*.sql)
      if git -C "$root" ls-files --error-unmatch -- "$rel" >/dev/null 2>&1; then
        deny "適用済みのマイグレーション ($rel) は変更禁止です (_sqlx_migrations のチェックサムと照合され、本番の api が起動しなくなる)。スキーマの変更は新しいファイルで行ってください。データの持ち方が変わるなら api/rollback/ に戻し方も用意します。"
      fi
      if [ "$tool" = "Write" ] && [ ! -e "$path" ]; then
        check_migration_name "$root" "$rel"
      fi
      ;;
  esac
}

# ディレクトリを含む checkout のルート (git でなければディレクトリそのもの)
repo_root_of() {
  git -C "$1" rev-parse --show-toplevel 2>/dev/null || printf '%s' "$1"
}

# 引用符を外す
unquote() {
  local t=$1
  t=${t//\"/}
  t=${t//\'/}
  printf '%s' "$t"
}

# git push の 1 区切り (git 以降の語) を調べる。$1 = 実行ディレクトリ、残り = 語
check_push() {
  local dir=$1
  shift
  local words=("$@")
  local i=0 n=${#words[@]} w positional=() branch repo_given=false tags_given=false
  # git のグローバルオプション ([-C <path>] [-c k=v] ...) を読み飛ばし、最初のサブコマンドが push のときだけ調べる
  # (git commit -m "push origin main" のような引数の中の push は見ない)
  while [ $i -lt "$n" ]; do
    w=$(unquote "${words[$i]}")
    case "$w" in
      -C)
        i=$((i + 1))
        w=$(unquote "${words[$i]:-}")
        case "$w" in
          /*) dir=$w ;;
          *) dir="$dir/$w" ;;
        esac
        ;;
      -c | --git-dir | --work-tree | --namespace | --config-env | --exec-path | --super-prefix) i=$((i + 1)) ;;
      -*) ;;
      *) break ;;
    esac
    i=$((i + 1))
  done
  [ $i -lt "$n" ] || return 0
  # 同じコマンドの中で先にブランチを切り替えていたら、後続の push は切替先のブランチで判定する
  if [ "$w" = "switch" ] || [ "$w" = "checkout" ]; then
    local target=""
    for ((i = i + 1; i < n; i++)); do
      w=$(unquote "${words[$i]}")
      case "$w" in
        --) break ;;
        # 新しいブランチを作って切り替える: 直後が切替先、その後ろは開始点 (origin/main など) なので見ない
        -c | -C | -b | -B | --create | --force-create | --orphan)
          target=$(unquote "${words[$((i + 1))]:-}")
          break
          ;;
        -*) ;;
        *)
          # 既存ブランチへの切替。checkout はファイル名も取るので、ブランチとして存在する名前だけを切替先にする
          case "$w" in
            main | origin/main) target=main ;;
            *)
              if git -C "$dir" show-ref -q --verify "refs/heads/$w" 2>/dev/null ||
                git -C "$dir" show-ref -q --verify "refs/remotes/origin/$w" 2>/dev/null; then
                target=$w
              fi
              ;;
          esac
          break
          ;;
      esac
    done
    if [ -n "$target" ]; then
      switched_branch=$target
      switched_root=$(repo_root_of "$dir")
    fi
    return 0
  fi
  [ "$w" = "push" ] || return 0
  local has_main=false
  git -C "$dir" show-ref --verify -q refs/heads/main 2>/dev/null && has_main=true
  for ((i = i + 1; i < n; i++)); do
    w=$(unquote "${words[$i]}")
    case "$w" in
      --all | --mirror | --branches)
        # 全ブランチ / 全 ref を送る。ローカルに main があれば main も送られる
        if $has_main; then
          deny "git push $w はローカルの main も送るので、main への直接 push になります。作業ブランチだけを指定して push してください。"
        fi
        ;;
      # タグだけを送る (refspec が無ければブランチは送らない)
      --tags) tags_given=true ;;
      # --repo はリモートを指定するので、位置引数はすべて refspec になる
      --repo) repo_given=true i=$((i + 1)) ;;
      --repo=*) repo_given=true ;;
      -o | --push-option | --receive-pack | --exec) i=$((i + 1)) ;;
      -*) ;;
      *) positional+=("$w") ;;
    esac
  done
  # 1 つ目はリモート (--repo があれば無し)、残りが refspec。宛先 (コロンの右、無ければそのもの) が main なら拒否
  local ref dst dynamic=false refs=() skip=1
  $repo_given && skip=0
  [ ${#positional[@]} -gt "$skip" ] && refs=("${positional[@]:$skip}")
  for ref in ${refs[@]+"${refs[@]}"}; do
    ref=${ref#+}
    # ":" だけ (両側にあるブランチをすべて送る matching push) は、ローカルに main があれば main も送る
    if [ "$ref" = ":" ] && $has_main; then
      deny "git push <remote> : は同名のブランチをすべて送るので、main も送られます。作業ブランチだけを指定して push してください。"
    fi
    dst=${ref##*:}
    # refs/heads/*:refs/heads/* のようなワイルドカードは、宛先のパターンが main に一致し、ローカルに main があれば拒否
    case "$dst" in
      *\**)
        # shellcheck disable=SC2053 # 宛先はグロブとして比べる
        if $has_main && { [[ refs/heads/main == $dst ]] || [[ main == $dst ]]; }; then
          deny "ワイルドカードの refspec ($ref) は main も送るので、main への直接 push になります。作業ブランチだけを指定して push してください。"
        fi
        ;;
    esac
    dst=${dst#refs/heads/}
    if [ "$dst" = "main" ]; then
      deny "main への直接 push は禁止です (ブランチ → PR → ci → squash マージ)。作業ブランチを push してください。"
    fi
    case "$ref" in
      HEAD | \$* | *:HEAD) dynamic=true ;;
    esac
  done
  # refspec が無い・HEAD・コマンド置換のときは、現在のブランチが main なら main への push になる
  # (--tags だけで refspec が無いときはタグしか送らないので対象外)
  if { [ ${#refs[@]} -eq 0 ] && ! $tags_given; } || $dynamic; then
    branch=$(git -C "$dir" symbolic-ref --short -q HEAD 2>/dev/null || true)
    # 切替は同じ checkout への push にだけ反映する (git -C A switch ... && git -C B push は B の現在のブランチで見る)
    if [ -n "$switched_branch" ] && [ "$(repo_root_of "$dir")" = "$switched_root" ]; then
      branch=$switched_branch
    fi
    if [ "$branch" = "main" ]; then
      deny "現在のブランチが main なので、この git push は main への直接 push になります。作業ブランチを作ってから push してください。"
    fi
  fi
}

# 標準入力のコマンド文字列を、引用符の状態を見ながら区切りごとの行に分ける。
#   - 単一引用符の中: 何も区切らない
#   - 二重引用符の中: コマンド置換 ($(...) / `...`) の括弧と ` だけで区切る (; & | は文字列の一部)
#   - 引用符の外: ; & | ( ) ` で区切る
split_commands() {
  awk -v sq="'" '
    BEGIN { q = "" }
    {
      out = ""; n = length($0)
      for (i = 1; i <= n; i++) {
        c = substr($0, i, 1)
        if (q == sq) { if (c == sq) q = ""; out = out c; continue }
        if (q == "\"") {
          if (c == "\"") q = ""
          if (c == "(" || c == ")" || c == "`") { out = out "\n"; continue }
          out = out c; continue
        }
        if (c == sq || c == "\"") { q = c; out = out c; continue }
        if (index(";&|()`", c) > 0) { out = out "\n"; continue }
        out = out c
      }
      # 行末の \ (単一引用符の外) は行継続なので、次の行とつなげてから区切る (cat \<改行> api/.env)
      if (q != sq && substr(out, length(out), 1) == "\\") {
        pending = pending substr(out, 1, length(out) - 1) " "
        next
      }
      print pending out
      pending = ""
    }
    END { if (pending != "") print pending }'
}

check_bash() {
  local cmd dir seg words w i n first base t
  cmd=$(jq -r '.tool_input.command // empty' <<<"$input")
  [ -n "$cmd" ] || return 0
  dir=$cwd
  switched_branch="" switched_root=""

  # ; & | ( ) ` と改行で区切り、区切りごとに先頭のコマンドを見る ($(...) の中も 1 区切りになる)。
  # 引用符の中の記号 (sed -n '1,20p;20q' の ; など) では区切らない
  while IFS= read -r seg; do
    read -r -a words <<<"$seg" || true
    n=${#words[@]}
    [ "$n" -gt 0 ] || continue
    # 先頭の VAR=value は飛ばす
    first=0
    while [ $first -lt "$n" ] && [[ ${words[$first]} =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; do
      first=$((first + 1))
    done
    [ $first -lt "$n" ] || continue
    base=$(basename -- "$(unquote "${words[$first]}")")
    # command / env / exec などのラッパーは、そのオプションと VAR=value ごと読み飛ばして実際のコマンドを見る
    while [ $first -lt "$n" ]; do
      case "$base" in
        # if ...; then cat ...; fi の then のような予約語の後ろも、実際のコマンドとして見る
        command | builtin | exec | env | nohup | time | nice | sudo | doas | \
          if | then | else | elif | do | while | until | \! | \{) ;;
        *) break ;;
      esac
      first=$((first + 1))
      while [ $first -lt "$n" ]; do
        w=$(unquote "${words[$first]}")
        case "$w" in
          -u | -C | -n | -g | -S) first=$((first + 2)) ;;
          -*) first=$((first + 1)) ;;
          *)
            [[ $w =~ ^[A-Za-z_][A-Za-z0-9_]*= ]] || break
            first=$((first + 1))
            ;;
        esac
      done
      [ $first -lt "$n" ] || continue 2
      base=$(basename -- "$(unquote "${words[$first]}")")
    done
    case "$base" in
      cd)
        t=$(unquote "${words[$((first + 1))]:-}")
        case "$t" in
          "") ;;
          /*) dir=$t ;;
          *) dir="$dir/$t" ;;
        esac
        ;;
      # sed / awk も範囲指定や '1' で丸ごと表示できるので対象にする (キーの取り出しは grep で行う)
      cat | less | more | head | tail | bat | nl | strings | xxd | od | hexdump | open | tac | sed | awk | gawk | perl)
        # .env 系を丸ごと表示する (grep で特定のキーを取り出す使い方は止めない)。ひな形かどうかは引数ごとに見る
        for ((i = first + 1; i < n; i++)); do
          t=$(unquote "${words[$i]}")
          t=${t#<}
          if is_secret_env "$t"; then
            deny ".env 系のファイル ($(basename -- "$t")) を丸ごと表示するコマンドは実行しない決まりです (AGENTS.md の P0: 秘密情報)。特定のキーが要るなら grep で取り出して変数に入れ、値を出力しないでください。"
          fi
        done
        ;;
      git)
        check_push "$dir" "${words[@]:$((first + 1))}"
        ;;
    esac
  # cat<api/.env のような空白なしの入力リダイレクトも、< の前後に空白を入れてコマンド名と入力先に分ける
  done < <(printf '%s\n' "$cmd" | split_commands | sed 's/</ < /g')
}

case "$tool" in
  Read | Edit | Write | MultiEdit | NotebookEdit) check_file_tool ;;
  Bash) check_bash ;;
esac
exit 0
