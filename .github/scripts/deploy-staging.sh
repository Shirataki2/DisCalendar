#!/usr/bin/env bash
# staging ホスト上で動かすデプロイ手順 (deploy-staging.yml が ssh 経由で流し込む)。
#   bash deploy-staging.sh <compose のディレクトリ> <イメージタグ>
# 前提: ディレクトリに compose.yaml と .env (secrets、COMPOSE_PROFILES=bot,tunnel など) が置いてあること。
# .env の IMAGE_TAG を書き換えるので、手動で docker compose up しても同じタグが使われる
set -euo pipefail

dir="$1"
tag="$2"
cd "$dir"

if grep -q '^IMAGE_TAG=' .env; then
  sed -i.bak "s|^IMAGE_TAG=.*|IMAGE_TAG=${tag}|" .env && rm -f .env.bak
else
  printf 'IMAGE_TAG=%s\n' "$tag" >> .env
fi
echo "deploying IMAGE_TAG=${tag} in ${dir}"

docker compose pull --quiet
docker compose up -d --remove-orphans

# healthcheck を持つサービス (db / api / web) が全部 healthy になるまで待つ (bot / cloudflared は healthcheck なし)
unhealthy=""
for _ in $(seq 1 36); do
  # Health が空 (healthcheck なし) のときに列がずれないよう、空白ではなく | で区切る
  unhealthy=$(docker compose ps --all --format '{{.Service}}|{{.Health}}|{{.State}}' \
    | awk -F'|' '($3 != "running") || ($2 != "" && $2 != "healthy") { print $1 }' | tr '\n' ' ')
  [ -z "$unhealthy" ] && break
  sleep 5
done

docker compose ps
if [ -n "$unhealthy" ]; then
  echo "::error::not healthy: ${unhealthy}"
  docker compose logs --tail 50 --no-color
  exit 1
fi

# 古いイメージを掃除する (参照されていないものだけ)
docker image prune -f >/dev/null
echo "deployed IMAGE_TAG=${tag}"
