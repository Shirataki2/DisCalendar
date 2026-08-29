#!/usr/bin/env python3
"""guilds.joined_at の backfill SQL を入退出メッセージログの CSV から生成する (#125)。

20260830030048_add_guilds_joined_at.sql はカラムを追加するだけで、既存レコードの参加日時は
埋めない (本番ギルド ID を公開リポジトリに含めないため)。このスクリプトで CSV (git 管理外)
からギルドごとの最新 ENTER 日時を抽出した UPDATE 文を生成し、マイグレーション適用後に
一度だけ手で流す。

CSV の形式 (入退出時にログチャンネルへ送っていたメッセージから作成したもの):
    created_at_jst,kind,server_name,server_id
    2021-01-05 11:05:46,ENTER,MassahRoom,724335733841854514

- 日時はタイムゾーンなしの JST (DB の他のカラムと同じ慣習)
- kind が ENTER の行だけを使い、ギルドごとに最新の日時を採用する
- 生成した UPDATE は joined_at が NULL の行しか触らないので、bot が記録した参加日時を
  上書きせず、何度流しても結果は変わらない。guilds に無い guild_id は単に無視される

使い方 (リポジトリのルートで):

    python3 api/scripts/backfill_guilds_joined_at.py tmp/message_log.csv > tmp/backfill_guilds_joined_at.sql

本番への適用 (compose の環境。生成した SQL は本番ギルド ID を含むので配布しない):

    docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 \
        < tmp/backfill_guilds_joined_at.sql
"""

import csv
import sys
from datetime import datetime


def main() -> None:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        sys.exit(2)

    latest: dict[str, str] = {}
    # 旧ログ由来の CSV は BOM 付きのことがあるので utf-8-sig で読む
    with open(sys.argv[1], encoding="utf-8-sig", newline="") as f:
        for row in csv.DictReader(f):
            # 想定外の値はそのまま SQL に入れず、ここで落とす
            datetime.strptime(row["created_at_jst"], "%Y-%m-%d %H:%M:%S")
            if not row["server_id"].isdigit():
                raise ValueError(f"server_id が数値でない: {row['server_id']!r}")
            if row["kind"] != "ENTER":
                continue
            gid = row["server_id"]
            if gid not in latest or row["created_at_jst"] > latest[gid]:
                latest[gid] = row["created_at_jst"]

    if not latest:
        raise ValueError("ENTER の行が 1 件もない")

    print("-- guilds.joined_at の backfill (#125)。生成元:", sys.argv[1])
    print("UPDATE guilds")
    print("SET joined_at = v.joined_at::timestamp")
    print("FROM (")
    print("    VALUES")
    rows = sorted(latest.items())
    body = ",\n".join(f"    ('{gid}', '{ts}')" for gid, ts in rows)
    print(body)
    print(") AS v (guild_id, joined_at)")
    print("WHERE guilds.guild_id = v.guild_id AND guilds.joined_at IS NULL;")
    print(f"-- {len(rows)} ギルド分", file=sys.stderr)


if __name__ == "__main__":
    main()
