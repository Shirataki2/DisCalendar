#!/usr/bin/env bash
# サポート記録の MCP (#425) から本番の DB に繋ぐための SSH トンネルを張る。
#
#   support-bot/scripts/mcp-tunnel.sh <ssh 先> [compose のディレクトリ] [手元のポート]
#   例: support-bot/scripts/mcp-tunnel.sh deploy@discalendar-prod /opt/discalendar 15432
#
# db のポートはホストに公開していないので、ホスト上で db コンテナの IP を調べ、そこへ転送する
# (Linux の Docker ではホストからコンテナの IP に直接届く)。手元は 127.0.0.1 にだけ bind する。
# Ctrl-C で閉じる。コンテナを作り直すと IP が変わるので、繋がらなくなったら張り直す。
set -euo pipefail

if [ $# -lt 1 ]; then
  sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
fi

target=$1
compose_dir=${2:-/opt/discalendar}
local_port=${3:-15432}

# compose_dir は手元で指定した値。リモートのシェルに渡すので printf %q でクォートする
remote_command="cd $(printf '%q' "$compose_dir") && docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' \"\$(docker compose ps -q db)\""
# shellcheck disable=SC2029 # compose_dir は意図して手元で展開する (クォート済み)
db_ip=$(ssh "$target" "$remote_command" | awk '{print $1}')
if ! [[ $db_ip =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "db コンテナの IP を取れませんでした (受け取った値: '${db_ip}')。compose のディレクトリと db が動いているかを確かめてください" >&2
  exit 1
fi

echo "127.0.0.1:${local_port} → ${target} → db (${db_ip}:5432)。Ctrl-C で閉じます" >&2
exec ssh -N -o ExitOnForwardFailure=yes -L "127.0.0.1:${local_port}:${db_ip}:5432" "$target"
