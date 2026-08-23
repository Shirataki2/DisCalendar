#!/usr/bin/env bash
# Claude Code のクラウドセッション (claude.ai/code / `claude --cloud`) が始まるときに、
# .claude/settings.json の SessionStart hook から呼ばれる。ローカルのセッションでは何もしない
# (クラウドの VM だけが CLAUDE_CODE_REMOTE=true を持つ)。
#
# やること (どれか失敗しても hook 自体は 0 で終わり、セッションは始まる):
#   1. PostgreSQL (プリインストールだが未起動) を起動し、discalendar_dev を作る
#      → cargo test (#[sqlx::test]) / cargo sqlx prepare / web の pnpm db:migrate が動く
#   2. web/ の pnpm install (web/AGENTS.md が node_modules/next/dist/docs/ を読ませるので必須)
#   3. .env.example からローカル開発用の .env を作る (値はダミー。Bot トークンなどの本物は入れない)
#
# クラウド環境 (Environment) 側の設定 (環境変数 SQLX_OFFLINE / DATABASE_URL、Setup script) は
# README.md「Claude Code のクラウド環境で使う」を参照。
set -u

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

root=${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$root" || exit 0

# 標準出力は SessionStart hook の結果として Claude に渡るので、状況が分かるように短く出す
log() { echo "[cloud-session-start] $*"; }

db_url="postgres://postgres:postgres@127.0.0.1:5432/discalendar_dev"

# root なら runuser、そうでなければ sudo で postgres OS ユーザーとして実行する
# (hook がどのユーザーで走るかはドキュメントに明記がないので両対応)
as_postgres() {
  if [ "$(id -u)" = 0 ]; then
    runuser -u postgres -- "$@"
  else
    sudo -n -u postgres "$@"
  fi
}

# 1. PostgreSQL
if command -v pg_isready >/dev/null 2>&1; then
  if [ "$(id -u)" = 0 ]; then
    service postgresql start >/dev/null 2>&1 || true
  else
    sudo -n service postgresql start >/dev/null 2>&1 || true
  fi
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    pg_isready -q -h 127.0.0.1 && break
    sleep 1
  done
  if pg_isready -q -h 127.0.0.1; then
    # パスワードは毎回付け直す (冪等)。DB は無いときだけ作る
    as_postgres psql -qc "ALTER USER postgres PASSWORD 'postgres'" >/dev/null 2>&1 || true
    if ! as_postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname = 'discalendar_dev'" 2>/dev/null | grep -q 1; then
      as_postgres createdb discalendar_dev >/dev/null 2>&1 || true
    fi
    log "PostgreSQL: 起動済み。DATABASE_URL=${db_url}"
  else
    log "PostgreSQL: 起動できなかった (service postgresql start を試すこと)"
  fi
else
  log "PostgreSQL: 見つからない (pg_isready が無い)"
fi

# 2. web の依存
if [ -f web/package.json ]; then
  # pnpm を web/package.json の packageManager の版に揃える (corepack があれば)
  if command -v corepack >/dev/null 2>&1; then
    corepack enable >/dev/null 2>&1 || true
  fi
  if (cd web && pnpm install --frozen-lockfile >/dev/null 2>&1); then
    log "web: pnpm install 完了 (pnpm $(cd web && pnpm -v 2>/dev/null || echo '?'))"
  else
    log "web: pnpm install に失敗。web/ で pnpm install --frozen-lockfile を実行して原因を見ること"
  fi
fi

# 3. ローカル開発用 .env (無いときだけ作る)
make_env() { # $1: .env.example, $2: 出力先
  [ -f "$1" ] || return 0
  [ -f "$2" ] && return 0
  sed \
    -e "s#^DATABASE_URL=.*#DATABASE_URL=${db_url}#" \
    -e "s#^BETTER_AUTH_SECRET=.*#BETTER_AUTH_SECRET=cloud-dummy-secret-not-used-at-runtime#" \
    "$1" > "$2"
  log "$2 を $1 から作成 (ダミー値)"
}
make_env web/.env.example web/.env.local
make_env api/.env.example api/.env
make_env bot/.env.example bot/.env

# 4. 補足 (Claude に伝えたい前提)
if [ "${SQLX_OFFLINE:-}" != "true" ]; then
  log "注意: 環境変数 SQLX_OFFLINE=true が未設定。query! のコンパイルには DB にマイグレーション適用済みである必要がある (README 参照)"
fi
log "このセッションはクラウド。旧実装 tmp/DisCalendarV2/ は無い。ブラウザ / Discord ログインでの動作確認はできないので検証は CI 相当のコマンドまで (AGENTS.md 参照)"
exit 0
