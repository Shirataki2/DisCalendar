#!/usr/bin/env bash
# Claude Code / Codex cloud の clone 直後に、CI 相当の検証ができる開発環境を準備する。
#
# Setup script:       bash .agents/scripts/setup-cloud-environment.sh --install-tools
# Maintenance script: bash .agents/scripts/setup-cloud-environment.sh
#
# 失敗してもクラウドセッション自体は開始できるよう、各結果をログに残して最後は 0 で終了する。
set -u

install_tools=0
if [ "${1:-}" = "--install-tools" ]; then
  install_tools=1
elif [ "$#" -gt 0 ]; then
  echo "usage: $0 [--install-tools]" >&2
  exit 2
fi

if repo_root=$(git rev-parse --show-toplevel 2>/dev/null); then
  :
else
  repo_root=$(cd "$(dirname "$0")/../.." && pwd)
fi
cd "$repo_root" || exit 0

# shellcheck source=lib/cloud-tools.sh
source "$repo_root/.agents/scripts/lib/cloud-tools.sh"

log() { echo "[agent-setup] $*"; }
db_url="postgres://postgres:postgres@127.0.0.1:5432/discalendar_dev"

run_privileged() {
  if [ "$(id -u)" = 0 ]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo -n "$@"
  else
    return 1
  fi
}

as_postgres() {
  if [ "$(id -u)" = 0 ]; then
    runuser -u postgres -- "$@"
  else
    sudo -n -u postgres "$@"
  fi
}

if [ "$install_tools" = 1 ]; then
  if { ! postgresql_server_available || ! command -v pg_isready >/dev/null 2>&1 || ! command -v psql >/dev/null 2>&1; } &&
    command -v apt-get >/dev/null 2>&1; then
    if run_privileged apt-get update -qq && run_privileged apt-get install -y -qq postgresql postgresql-client; then
      log "PostgreSQL をインストールしました"
    else
      log "PostgreSQL のインストールに失敗しました"
    fi
  fi

  if ensure_rustup; then
    rustup toolchain install >/dev/null 2>&1 || log "rustup toolchain install に失敗しました"
  else
    log "rustup の導入に失敗しました"
  fi

  if command -v corepack >/dev/null 2>&1; then
    corepack enable >/dev/null 2>&1 || true
  fi

  # 独立した取得処理を並列化してセットアップ時間を抑える。
  if command -v cargo >/dev/null 2>&1; then
    (cargo fetch >/dev/null 2>&1 || log "cargo fetch に失敗しました") &
    if ! command -v sqlx >/dev/null 2>&1; then
      (cargo install sqlx-cli --no-default-features --features postgres,rustls >/dev/null 2>&1 || log "sqlx-cli のインストールに失敗しました") &
    fi
  fi
  if [ -f web/package.json ]; then
    ((cd web && pnpm install --frozen-lockfile >/dev/null 2>&1) || log "web: pnpm install に失敗しました") &
  fi
  wait
else
  if command -v corepack >/dev/null 2>&1; then
    corepack enable >/dev/null 2>&1 || true
  fi
  if [ -f web/package.json ]; then
    (cd web && pnpm install --frozen-lockfile >/dev/null 2>&1) || log "web: pnpm install に失敗しました"
  fi
fi

pg_ready=0
pg_note=""
if command -v pg_isready >/dev/null 2>&1 && command -v psql >/dev/null 2>&1; then
  run_privileged service postgresql start >/dev/null 2>&1 || pg_note="${pg_note} PostgreSQL の起動に失敗;"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    pg_isready -q -h 127.0.0.1 && break
    sleep 1
  done
  if pg_isready -q -h 127.0.0.1; then
    as_postgres psql -qc "ALTER USER postgres PASSWORD 'postgres'" >/dev/null 2>&1 || pg_note="${pg_note} postgres のパスワード設定に失敗;"
    if ! as_postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname = 'discalendar_dev'" 2>/dev/null | grep -q 1; then
      as_postgres createdb discalendar_dev >/dev/null 2>&1 || pg_note="${pg_note} discalendar_dev の作成に失敗;"
    fi
    if psql "$db_url" -tAc "SELECT 1" >/dev/null 2>&1; then
      pg_ready=1
    else
      pg_note="${pg_note} DATABASE_URL で接続できない;"
    fi
  else
    pg_note="${pg_note} PostgreSQL が ready にならない;"
  fi
else
  pg_note="${pg_note} pg_isready / psql がない;"
fi

if [ "$pg_ready" = 1 ] && command -v sqlx >/dev/null 2>&1; then
  if sqlx migrate run --source api/migrations --database-url "$db_url" >/dev/null 2>&1; then
    log "PostgreSQL: 準備完了 (api/migrations 適用済み)"
  else
    log "PostgreSQL: 接続済みですが api/migrations の適用に失敗しました"
  fi
elif [ "$pg_ready" = 1 ]; then
  log "PostgreSQL: 接続済みですが sqlx-cli がないためマイグレーション未適用です"
else
  log "PostgreSQL: 未準備。原因:${pg_note:- 不明}"
fi

make_env() {
  example=$1
  output=$2
  [ -f "$example" ] || return 0
  [ -f "$output" ] && return 0
  sed \
    -e "s#^DATABASE_URL=.*#DATABASE_URL=${db_url}#" \
    -e "s#^BETTER_AUTH_SECRET=.*#BETTER_AUTH_SECRET=cloud-dummy-secret-not-used-at-runtime#" \
    "$example" > "$output"
  log "$output をダミー値で作成しました"
}

make_env web/.env.example web/.env.local
make_env api/.env.example api/.env
make_env bot/.env.example bot/.env

if [ "${SQLX_OFFLINE:-}" != "true" ]; then
  log "注意: クラウド環境の Environment variables に SQLX_OFFLINE=true を設定してください"
fi
log "準備完了。実トークンは保存していないため Discord ログイン・Bot 実機確認はできません"
exit 0
