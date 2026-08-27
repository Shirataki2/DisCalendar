-- 予定の通知設定を旧形式から JSONB に移す (#15)。
--
-- 旧: events.notifications TEXT[]  各要素が {"key":0,"num":30,"type":"分前"} の JSON 文字列
--     (旧 Web が保存し旧 Bot が読んでいた形式。key は旧 Web の v-for 用で意味がなく、
--      type は日本語ラベルなので拡張しづらい)
-- 新: events.notifications JSONB  [{"num":30,"unit":"minutes"}]
--     (api / bot が API 表現のまま読み書きでき、両方に置いていた変換コードが不要になる)
--
-- 旧 Bot / 旧 Web は本番切替 (#12) で停止済みなので、この非互換な型変更ができる。

-- 退避と型変換の間に events が書き換わると、退避した内容と実際に変換された値がずれる
-- (INSERT ... SELECT が取るのは ACCESS SHARE なので通常の INSERT / UPDATE と競合せず、
--  後続の ALTER TABLE が排他ロックを得るまでに作られた行は変換対象になるのに退避表には無い、
--  更新された行は退避表だけ古い値のまま残る)。その状態の退避表から戻すと通知設定を失うので、
-- どのみち ALTER TABLE ... TYPE が取る排他ロックを最初に取って writer を止めてしまう
LOCK TABLE events IN ACCESS EXCLUSIVE MODE;

-- 変換前の生データを退避する。変換で捨てられる要素 (壊れた JSON・未知の単位・範囲外の num) は
-- 元々 api / bot が無視していて利用者には見えていないが、非可逆な変換なのでここに残しておく。
-- 旧形式に戻す必要がないと確認できたら別のマイグレーションで DROP する
CREATE TABLE events_notifications_legacy (
    event_id INTEGER PRIMARY KEY,
    notifications TEXT[] NOT NULL,
    saved_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO events_notifications_legacy (event_id, notifications)
SELECT id, notifications FROM events;

-- 旧形式 → API 表現の変換。api (Notification::from_legacy) / bot が読めていた要素だけを残し、
-- 読めなかった要素は同じ規則で捨てる。旧 decoder は serde の
-- `Legacy { key: i64, num: i64, type: String }` で、フィールドが 1 つでも欠けたり型が合わなければ
-- その要素のデシリアライズに失敗して捨てていたので、次のいずれかに当たるものは残さない:
--   - JSON として壊れている、オブジェクトでない
--   - key / num / type のどれかが重複している (serde は duplicate field エラーにするが、
--     jsonb は最後の値だけを残すので、素通しにすると読めなかった要素が有効な通知に化ける)
--   - key が無い、整数でない、i64 に収まらない (旧 Web の v-for 用で値自体は使わないが必須だった)
--   - num が非負整数でない、u32 に収まらない (api / bot ともに u32 で受けている)
--   - type が既知の 4 種類 (分前 / 時間前 / 日前 / 週間前) でない
-- 配列の順序は元のまま保つ (通知の一覧表示の順序が変わらないように)。
-- ALTER TABLE ... USING にはサブクエリを直接書けないので関数に包み、変換後に落とす
CREATE FUNCTION convert_legacy_notifications(legacy TEXT[]) RETURNS JSONB
LANGUAGE sql IMMUTABLE AS $$
    SELECT COALESCE(
        (
            SELECT jsonb_agg(
                       jsonb_build_object('num', (item->>'num')::bigint, 'unit', unit)
                       ORDER BY ord
                   )
            FROM unnest(legacy) WITH ORDINALITY AS raw_items(raw, ord)
            CROSS JOIN LATERAL (
                SELECT CASE WHEN pg_input_is_valid(raw_items.raw, 'jsonb') THEN raw_items.raw::jsonb END
            ) AS parsed(item)
            CROSS JOIN LATERAL (
                SELECT CASE item->>'type'
                           WHEN '分前' THEN 'minutes'
                           WHEN '時間前' THEN 'hours'
                           WHEN '日前' THEN 'days'
                           WHEN '週間前' THEN 'weeks'
                       END
            ) AS mapped(unit)
            CROSS JOIN LATERAL (
                -- jsonb はキーが重複していると最後の値だけを残すので、重複の有無は
                -- 元の文字列を json (入力をそのまま保持する型) として読み直して見る。
                -- オブジェクトでない値に json_object_keys は使えないため CASE の中で呼ぶ
                SELECT CASE
                           WHEN jsonb_typeof(parsed.item) = 'object'
                           THEN (
                               SELECT count(*) <> count(DISTINCT key_name)
                               FROM json_object_keys(raw_items.raw::json) AS keys(key_name)
                           )
                           ELSE FALSE
                       END
            ) AS dup(has_duplicate_keys)
            WHERE jsonb_typeof(item) = 'object'
              AND NOT dup.has_duplicate_keys
              -- jsonb の number は numeric なので、整数かどうかは文字列表現で見る
              AND jsonb_typeof(item->'key') = 'number'
              AND (item->>'key') ~ '^-?[0-9]+$'
              AND (item->>'key')::numeric BETWEEN -9223372036854775808 AND 9223372036854775807
              AND jsonb_typeof(item->'num') = 'number'
              AND (item->>'num') ~ '^[0-9]+$'
              AND (item->>'num')::numeric <= 4294967295
              AND unit IS NOT NULL
        ),
        '[]'::jsonb
    );
$$;

ALTER TABLE events
    ALTER COLUMN notifications TYPE JSONB
    USING convert_legacy_notifications(notifications);

DROP FUNCTION convert_legacy_notifications(TEXT[]);

ALTER TABLE events
    ALTER COLUMN notifications SET DEFAULT '[]'::jsonb;

-- api / bot は常に配列を書き込む。管理コンソールの定型操作などが誤って
-- オブジェクトや null を入れても読み出し側が壊れないよう、配列であることだけ DB で保証する
ALTER TABLE events
    ADD CONSTRAINT events_notifications_is_array
    CHECK (jsonb_typeof(notifications) = 'array');
