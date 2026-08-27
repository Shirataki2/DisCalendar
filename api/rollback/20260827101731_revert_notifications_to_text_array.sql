-- #15 で JSONB にした events.notifications を旧形式の TEXT[] に戻す (ロールバック用)。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 使うのは「#15 が入った版から、その前の版のイメージへ戻す」ときだけ。
-- 古い api / bot は notifications を TEXT[] としてデコード・書き込みするので、
-- DB を JSONB のままにして古いイメージを起動すると、予定に関するクエリが失敗し続ける。
-- さらに `sqlx::migrate!` は既定 (ignore_missing = false) なので、古い api は
-- 自分の migrations/ に無いバージョンが `_sqlx_migrations` にあるだけで起動に失敗する。
-- そのため型と記録の両方を戻す。
--
-- 手順 (compose の環境。README「DB のバックアップと復元」と同じ要領):
--
--   1. api / bot を止める (web は落としたままでよい): docker compose stop api bot
--   2. 先にダンプを取る:                              docker compose exec -T db pg_dump -U discalendar -d discalendar -Fc > 戻す前.dump
--   3. このファイルを流す:                            docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
--   4. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)
--
-- 注意: 変換のときに捨てられた要素 (壊れた JSON・未知の単位など) を記録した
-- events_notifications_legacy もここで落とす。元々 api / bot が無視していて利用者には
-- 見えていない値だが、必要なら手順 2 のダンプから拾う。

LOCK TABLE events IN ACCESS EXCLUSIVE MODE;

-- API 表現 → 旧形式。key は旧 Web と同じく配列内の位置 (0 始まり) を振り直す。
-- 解釈できない要素は捨てる (api / bot が元から無視していたものなので、戻す先でも読めない)。
-- 移行前にあった未知のフィールドは戻らない (旧 decoder も serde が読み飛ばしていた値なので、
-- 戻した先の挙動は変わらない)。
--
-- json_build_object はキーと値の間に空白を入れる (`{"key" : 0, ...}`) ので、旧 Web / 旧 api が
-- 書いていた表記に揃うよう文字列で組み立てる。読む側は serde_json / JSON.parse なので
-- 空白があっても動くが、戻したデータが元と違う見た目で残ると後で混乱するため。
-- num は数値、type は下の CASE が返す既知の 4 語なので、エスケープが要る文字は入らない
CREATE FUNCTION revert_notifications_to_legacy(current JSONB) RETURNS TEXT[]
LANGUAGE sql IMMUTABLE AS $$
    SELECT COALESCE(
        (
            SELECT array_agg(
                       '{"key":' || (ord - 1)
                           || ',"num":' || (item->>'num')::bigint
                           || ',"type":"' || label || '"}'
                       ORDER BY ord
                   )
            FROM jsonb_array_elements(
                     CASE WHEN jsonb_typeof(current) = 'array' THEN current ELSE '[]'::jsonb END
                 ) WITH ORDINALITY AS items(item, ord)
            CROSS JOIN LATERAL (
                SELECT CASE item->>'unit'
                           WHEN 'minutes' THEN '分前'
                           WHEN 'hours' THEN '時間前'
                           WHEN 'days' THEN '日前'
                           WHEN 'weeks' THEN '週間前'
                       END
            ) AS mapped(label)
            WHERE jsonb_typeof(item) = 'object'
              AND jsonb_typeof(item->'num') = 'number'
              AND (item->>'num') ~ '^[0-9]+$'
              AND (item->>'num')::numeric <= 4294967295
              AND label IS NOT NULL
        ),
        '{}'::text[]
    );
$$;

ALTER TABLE events DROP CONSTRAINT events_notifications_is_array;
ALTER TABLE events ALTER COLUMN notifications DROP DEFAULT;

ALTER TABLE events
    ALTER COLUMN notifications TYPE TEXT[]
    USING revert_notifications_to_legacy(notifications);

DROP FUNCTION revert_notifications_to_legacy(JSONB);

DROP TABLE events_notifications_legacy;

-- 複合インデックスは古い api でも無害だが、マイグレーションの記録を消す以上
-- 一緒に戻しておく (再度ロールフォワードしたときに CREATE INDEX が衝突しないように)
DROP INDEX IF EXISTS idx_events_guild_id_start_at;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)。
-- 戻したあと再びロールフォワードすれば、この 2 本が改めて適用される
DELETE FROM _sqlx_migrations WHERE version IN (20260827101731, 20260827101818);
