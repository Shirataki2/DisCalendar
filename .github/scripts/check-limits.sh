#!/usr/bin/env bash
# web と api / bot で同じ意味を持つ上限値 (タイトル 32 文字・説明 1000 文字・通知 10 件など) が揃っているか確かめる。
# CI の version ジョブと .agents/scripts/pre-pr-check.sh が使う。
#   .github/scripts/check-limits.sh
#
# 上限値は各言語の定数として定義されている (web は export const、Rust は const)。片方だけ変えると、
# web の入力チェックは通るのに api が 400 を返す (またはその逆) ずれが起きるので、対応表で照合する。
# 新しい上限値を web と api の両方に定義したら、下の LIMITS に 1 行足す。
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

# 名前|ファイル:定数名|ファイル:定数名[|...]
LIMITS=(
  "タイトルの文字数|web/src/lib/event-form.ts:NAME_MAX_CHARS|api/src/models/events.rs:NAME_MAX_CHARS|bot/src/models/events.rs:NAME_MAX_CHARS"
  "説明の文字数|web/src/lib/event-form.ts:DESCRIPTION_MAX_CHARS|api/src/models/events.rs:DESCRIPTION_MAX_CHARS|bot/src/models/events.rs:DESCRIPTION_MAX_CHARS"
  "場所の文字数|web/src/lib/event-form.ts:LOCATION_MAX_CHARS|api/src/models/events.rs:LOCATION_MAX_CHARS|bot/src/models/events.rs:LOCATION_MAX_CHARS"
  "通知の件数|web/src/lib/event-form.ts:NOTIFICATIONS_MAX|api/src/models/events.rs:NOTIFICATIONS_MAX"
  "通知の数値の上限|web/src/lib/event-form.ts:NOTIFICATION_NUM_MAX|api/src/models/notifications.rs:NOTIFICATION_NUM_MAX"
  "通知のメンション数|web/src/lib/event-form.ts:MENTIONS_MAX|api/src/models/notification_mentions.rs:MENTIONS_MAX"
  "添付ファイルの容量|web/src/lib/attachments.ts:ATTACHMENT_MAX_BYTES|api/src/models/attachments.rs:FILE_MAX_BYTES"
  "添付ファイルの件数|web/src/lib/attachments.ts:ATTACHMENT_MAX_FILES|api/src/models/attachments.rs:EVENT_MAX_FILES"
  "カバー画像の容量|web/src/lib/attachments.ts:COVER_MAX_BYTES|api/src/models/attachments.rs:COVER_MAX_BYTES"
  "ICS 取り込みの容量|web/src/components/ics-import-dialog.tsx:FILE_MAX_BYTES|api/src/ical_import.rs:ICS_MAX_BYTES"
  "ICS 取り込みの展開数|web/src/components/ics-import-dialog.tsx:OCCURRENCE_MAX|api/src/ical_import.rs:IMPORT_MAX_OCCURRENCES"
  "横断表示のギルド数|web/src/lib/api/types.ts:JOINED_EVENTS_MAX_GUILDS|api/src/routes/events.rs:JOINED_MAX_IDS"
  "管理: ギルド一覧のページ上限|web/src/lib/api/types.ts:ADMIN_GUILDS_MAX_PAGE|api/src/models/admin_guilds.rs:MAX_PAGE"
  "管理: ユーザー一覧のページ上限|web/src/lib/api/types.ts:ADMIN_USERS_MAX_PAGE|api/src/models/admin_users.rs:MAX_PAGE"
  "管理: 監査ログのページ上限|web/src/lib/api/types.ts:ADMIN_AUDIT_LOGS_MAX_PAGE|api/src/models/admin_audit.rs:MAX_PAGE"
  "管理: ユーザーのセッション件数|web/src/lib/api/types.ts:ADMIN_USER_SESSION_LIMIT|api/src/models/admin_users.rs:SESSION_LIMIT"
  "管理: SQL の行数|web/src/lib/api/types.ts:ADMIN_SQL_MAX_ROWS|api/src/models/admin_sql.rs:MAX_ROWS"
  "管理: SQL の文字数|web/src/lib/api/types.ts:ADMIN_SQL_MAX_CHARS|api/src/models/admin_sql.rs:MAX_SQL_CHARS"
  "管理: SQL のセルの文字数|web/src/lib/api/types.ts:ADMIN_SQL_MAX_CELL_CHARS|api/src/models/admin_sql.rs:MAX_CELL_CHARS"
  "管理: 同期一覧の件数|web/src/lib/api/types.ts:ADMIN_SYNC_LIST_LIMIT|api/src/routes/admin_status.rs:SYNC_LIST_LIMIT"
  "管理: 削除スナップショットの件数|web/src/lib/api/types.ts:ADMIN_DELETE_SNAPSHOT_LIMIT|api/src/models/admin_ops.rs:DELETE_SNAPSHOT_LIMIT"
)

# 定数の値を数値にする。`10 * 1024 * 1024` や `1_000_000` のような整数の式だけを受け付ける
const_value() {
  local file=$1 name=$2 expr
  [ -f "$file" ] || return 1
  expr=$(sed -nE "s/^[[:space:]]*(export[[:space:]]+)?(pub(\([a-z]+\))?[[:space:]]+)?const[[:space:]]+$name([[:space:]]*:[[:space:]]*[A-Za-z0-9_]+)?[[:space:]]*=[[:space:]]*([^;]+);?[[:space:]]*(\/\/.*)?$/\5/p" "$file" | head -1)
  expr=${expr//_/}
  [[ $expr =~ ^[0-9[:space:]*+]+$ ]] || return 1
  echo $((expr))
}

fail=0
for row in "${LIMITS[@]}"; do
  IFS='|' read -r -a cols <<<"$row"
  label=${cols[0]}
  first=""
  detail=""
  mismatch=0
  for ref in "${cols[@]:1}"; do
    file=${ref%%:*}
    name=${ref##*:}
    if ! value=$(const_value "$file" "$name"); then
      echo "取得できない: $label ($file の $name。定数名を変えたら check-limits.sh の対応表も直す)" >&2
      fail=1
      continue 2
    fi
    detail+=" $file:$name=$value"
    if [ -z "$first" ]; then
      first=$value
    elif [ "$value" != "$first" ]; then
      mismatch=1
    fi
  done
  if [ "$mismatch" -eq 1 ]; then
    echo "ずれ: $label ->$detail" >&2
    fail=1
  fi
done

if [ "$fail" -eq 0 ]; then
  echo "上限値: ${#LIMITS[@]} 件すべて一致"
fi
exit "$fail"
