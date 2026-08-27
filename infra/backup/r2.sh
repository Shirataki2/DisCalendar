#!/usr/bin/env bash
# R2 を aws-cli で操作するラッパ (#102)。設定は backup.env から読む。
#   ./r2.sh s3 ls "s3://$R2_BUCKET/production/"
#   ./r2.sh s3 cp "s3://$R2_BUCKET/production/discalendar-20260827T190000Z.dump" .
#
# ホストに aws-cli を入れずに済むよう docker で動かす。カレントディレクトリを /work にマウントして
# 作業ディレクトリにするので、ローカル側のパスは相対パスで渡すこと (絶対パスはコンテナから見えない)。
# backup-db.sh もアップロードにこれを使う。
set -euo pipefail

# systemd から呼ばれるときは EnvironmentFile で入っている。手で叩くときはここで読む
env_file="${BACKUP_ENV_FILE:-/etc/discalendar/backup.env}"
if [ -z "${R2_BUCKET:-}" ] && [ -r "$env_file" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$env_file"
  set +a
fi

for name in R2_ACCOUNT_ID R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY R2_BUCKET; do
  if [ -z "${!name:-}" ]; then
    echo "[r2] $name が設定されていない ($env_file を確認する)" >&2
    exit 1
  fi
done

# 値を docker run の引数に書くと ps に出るので、名前だけ渡して環境から継承させる
export AWS_ACCESS_KEY_ID="$R2_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$R2_SECRET_ACCESS_KEY"
# R2 はリージョンを持たないが aws-cli が要求するので auto を渡す
export AWS_DEFAULT_REGION=auto
# 新しい AWS SDK は既定で CRC32 チェックサムを付けるが R2 は受け付けないので、必要なときだけにする
export AWS_REQUEST_CHECKSUM_CALCULATION=WHEN_REQUIRED
export AWS_RESPONSE_CHECKSUM_VALIDATION=WHEN_REQUIRED

exec docker run --rm -i \
  --user "$(id -u):$(id -g)" \
  -e AWS_ACCESS_KEY_ID \
  -e AWS_SECRET_ACCESS_KEY \
  -e AWS_DEFAULT_REGION \
  -e AWS_REQUEST_CHECKSUM_CALCULATION \
  -e AWS_RESPONSE_CHECKSUM_VALIDATION \
  -v "$PWD:/work" \
  -w /work \
  "${AWS_CLI_IMAGE:-public.ecr.aws/aws-cli/aws-cli:2.36.32}" \
  --endpoint-url "https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com" \
  "$@"
