#!/usr/bin/env bash
# staging / 本番ホスト上で動かすデプロイ手順 (deploy-staging.yml / deploy-production.yml が ssh 経由で流し込む)。
#   bash deploy.sh <compose のディレクトリ> <イメージタグ> [web のイメージタグ]
# 前提: ディレクトリに compose.yaml と .env (secrets、COMPOSE_PROFILES など。本番は COMPOSE_PROJECT_NAME も) が置いてあること。
# .env の IMAGE_TAG / WEB_IMAGE_TAG を書き換えるので、手動で docker compose up しても同じタグが使われる。
# web は OGP などの絶対 URL をビルド時に焼き込む (#87) ため staging では別ビルドを使う。第 3 引数でそのタグを渡す
# (省略したら web も IMAGE_TAG と同じ、つまり本番ドメインを焼き込んだイメージになる)
set -euo pipefail

dir="$1"
tag="$2"
web_tag="${3:-$2}"
cd "$dir"

set_env() {
  if grep -q "^$1=" .env; then
    sed -i.bak "s|^$1=.*|$1=$2|" .env && rm -f .env.bak
  else
    printf '%s=%s\n' "$1" "$2" >> .env
  fi
}

set_env IMAGE_TAG "$tag"
set_env WEB_IMAGE_TAG "$web_tag"
echo "deploying IMAGE_TAG=${tag} (web: ${web_tag}) in ${dir}"

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

# 古いイメージを掃除する。sha-* タグが付いたままの旧イメージは dangling にならないので、
# metadata-action が付ける OCI ラベルでこのリポジトリのイメージに絞り、コンテナから参照されていないものを消す
# (同じホストの他のイメージには触れない。ロールバックは GHCR から pull し直す)
docker image prune -af --filter "label=org.opencontainers.image.source=https://github.com/Shirataki2/DisCalendarV3-new" >/dev/null
echo "deployed IMAGE_TAG=${tag}"
