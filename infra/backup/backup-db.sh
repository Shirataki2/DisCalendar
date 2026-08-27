#!/usr/bin/env bash
# 本番 / staging ホストで日次に動かす DB のバックアップ (#102)。
# compose の db から pg_dump -Fc したダンプを Cloudflare R2 にアップロードする。
# 世代管理は R2 のライフサイクルルール (infra/terraform/r2_backup.tf) に任せるので、ここでは古い分を消さない。
#
#   sudo systemctl start discalendar-backup.service   # 手で 1 回動かす
#   BACKUP_ENV_FILE=./backup.env ./backup-db.sh       # 設定ファイルを指定して動かす
#
# 設定は backup.env (backup.env.example を参照)。systemd 経由なら EnvironmentFile で渡る。
set -euo pipefail

log() { printf '[backup-db] %s\n' "$*"; }

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

env_file="${BACKUP_ENV_FILE:-/etc/discalendar/backup.env}"
if [ -z "${R2_BUCKET:-}" ] && [ -r "$env_file" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$env_file"
  set +a
fi

compose_dir="${COMPOSE_DIR:-/opt/discalendar}"
db_user="${DB_USER:-discalendar}"
db_name="${DB_NAME:-discalendar}"
prefix="${BACKUP_PREFIX:-production}"
work_dir="${BACKUP_WORK_DIR:-/var/tmp}"

if [ ! -f "$compose_dir/compose.yaml" ]; then
  log "compose.yaml が $compose_dir に無い (COMPOSE_DIR を確認する)"
  exit 1
fi

cd "$compose_dir"

# db が動いていないと pg_dump は空のダンプを作らずに失敗するが、理由が分かりやすいよう先に見る
if [ "$(docker compose ps --status running --format '{{.Service}}' db | wc -l)" -eq 0 ]; then
  log "db サービスが動いていない ($compose_dir)"
  exit 1
fi

tmp_dir="$(mktemp -d "$work_dir/discalendar-backup.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

name="discalendar-$(date -u +%Y%m%dT%H%M%SZ).dump"
key="$prefix/$name"

log "pg_dump を実行する ($db_name @ $compose_dir)"
# -T は TTY 割り当てを止める (付けないとバイナリ出力が壊れることがある)
docker compose exec -T db pg_dump -U "$db_user" -d "$db_name" -Fc > "$tmp_dir/$name"

if [ ! -s "$tmp_dir/$name" ]; then
  log "ダンプが空になった"
  exit 1
fi

# 壊れたダンプをそのまま上げないよう、目録を読めるかだけ確かめる (ホストに pg_restore が無いので db 側で動かす)
docker compose exec -T db pg_restore -l < "$tmp_dir/$name" > /dev/null

size="$(wc -c < "$tmp_dir/$name" | tr -d ' ')"
log "ダンプを作成した ($name, ${size} bytes)"

log "R2 にアップロードする (s3://$R2_BUCKET/$key)"
# r2.sh はカレントディレクトリをコンテナにマウントするので、ダンプのある場所から相対パスで渡す
(cd "$tmp_dir" && "$script_dir/r2.sh" s3 cp "$name" "s3://$R2_BUCKET/$key" --only-show-errors)

# アップロードされた実体のサイズが手元と一致するか確かめる (途中で切れていないか)
uploaded="$("$script_dir/r2.sh" s3api head-object --bucket "$R2_BUCKET" --key "$key" --query ContentLength --output text | tr -d '\r')"
if [ "$uploaded" != "$size" ]; then
  log "アップロード後のサイズが合わない (手元 ${size} bytes / R2 ${uploaded} bytes)"
  exit 1
fi

log "完了 (s3://$R2_BUCKET/$key, ${size} bytes)"
