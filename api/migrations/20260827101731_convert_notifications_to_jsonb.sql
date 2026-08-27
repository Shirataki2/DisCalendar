-- 予定の通知設定を旧形式から JSONB に移す (#15)。
--
-- 旧: events.notifications TEXT[]  各要素が {"key":0,"num":30,"type":"分前"} の JSON 文字列
--     (旧 Web が保存し旧 Bot が読んでいた形式。key は旧 Web の v-for 用で意味がなく、
--      type は日本語ラベルなので拡張しづらい)
-- 新: events.notifications JSONB  [{"num":30,"unit":"minutes"}]
--     (api / bot が API 表現のまま読み書きでき、両方に置いていた変換コードが不要になる)
--
-- 旧 Bot / 旧 Web は本番切替 (#12) で停止済みなので、この非互換な型変更ができる。
-- 戻し方は api/rollback/20260827101731_revert_notifications_to_text_array.sql。
--
-- 適用中は events への書き込みが止まる (下の LOCK と ALTER TABLE ... TYPE の全表書き換え)。
-- Bot の通知タスクは起動時に直近 5 分ぶんしか遡らない (bot の STARTUP_LOOKBACK) ので、
-- デプロイの停止時間がそれを超えると、その間に発火するはずだった通知は送られないまま終わる。
-- events が小さければ一瞬で終わるが、適用前に規模を確かめること
-- (README の「DB を書き換えるマイグレーションを適用するとき」)。

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

-- 旧形式 → API 表現の変換。api (Notification::from_legacy) / bot が読めていた要素だけを残す。
-- 旧 decoder は serde の `Legacy { key: i64, num: i64, type: String }` で、
-- **未知のフィールドは無視するが、既知の 3 つが欠ける・型が合わない・重複するとその要素ごと捨てていた**。
-- 同じ結果になるよう、次のいずれかに当たるものは残さない:
--   - JSON として壊れている、オブジェクトでない
--   - key / num / type のどれかが重複している (serde は duplicate field エラー。
--     未知のフィールドの重複は serde が無視するので、ここでも判定に含めない)
--   - key が無い、整数でない、i64 に収まらない (旧 Web の v-for 用で値自体は使わないが必須だった)
--   - num が無い、整数でない、0 未満、u32 に収まらない (api / bot ともに u32 で受けている)
--   - type が既知の 4 種類 (分前 / 時間前 / 日前 / 週間前) でない
-- 配列の順序は元のまま保つ (通知の一覧表示の順序が変わらないように)。
-- ALTER TABLE ... USING にはサブクエリを直接書けないので関数に包み、変換後に落とす
CREATE FUNCTION convert_legacy_notifications(legacy TEXT[]) RETURNS JSONB
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE
    -- 旧 api / bot はこの列を Vec<String> として読むので、多次元配列や NULL 要素を含む配列は
    -- 列のデコードごと失敗していた (その行の通知が読めないどころか、予定の取得自体が失敗する)。
    -- 同じ結果になるよう、そういう配列は中身を拾わず空にする。
    -- CASE は上から順に評価されるので、多次元を先に弾いてから array_position を呼べる
    -- (多次元配列に array_position は使えない)
    WHEN array_ndims(legacy) > 1 THEN '[]'::jsonb
    WHEN array_position(legacy, NULL) IS NOT NULL THEN '[]'::jsonb
    ELSE COALESCE(
        (
            SELECT jsonb_agg(
                       jsonb_build_object(
                           'num', (parsed.raw_json->'num')::text::bigint,
                           'unit', unit
                       )
                       ORDER BY ord
                   )
            FROM unnest(legacy) WITH ORDINALITY AS raw_items(raw, ord)
            CROSS JOIN LATERAL (
                -- NUL (U+0000) のエスケープ (JSON で NUL を書く唯一の方法) を空白のエスケープに
                -- 置き換える。serde は NUL を含む文字列を読めていた (Rust の String は NUL を持てる)
                -- ので要素は捨てないが、PostgreSQL は json / jsonb のどちらでも NUL を text にできず、
                -- json_object_keys や `->>` の評価で落ちてしまう。既知の 3 フィールドの値に
                -- 入っていた場合も、既知の 4 種類と一致しなくなるだけで結果は変わらない。
                -- バックスラッシュは chr(92) で組み立てて、このファイルに制御文字を書かない
                SELECT replace(raw_items.raw, chr(92) || 'u0000', chr(92) || 'u0020')
            ) AS sanitized(raw)
            CROSS JOIN LATERAL (
                -- json と jsonb の両方に通るものだけを対象にする。NUL を除いたあとに jsonb が
                -- 受け付けない = text にデコードできない Unicode エスケープ (単独サロゲートなど) が
                -- あるということで、serde も未知フィールドの値であれ読み飛ばす際にパースするので
                -- 要素ごと捨てていた。正しいサロゲートペア (絵文字) やエスケープされた
                -- バックスラッシュ (リテラルの \uD800 という文字列) はどちらも通るので、
                -- 正規表現で見分けるより確実。
                -- 判定自体は入力の表記を保持する json 側で行う (jsonb は数値を正規化して
                -- 1e2 を 100 にし、重複キーを最後の値に潰してしまう)。
                -- 出力は既知の 3 フィールドから組み立て直すので、未知フィールドは JSONB に持ち込まない
                SELECT CASE
                           WHEN pg_input_is_valid(sanitized.raw, 'json')
                                AND pg_input_is_valid(sanitized.raw, 'jsonb')
                           THEN sanitized.raw::json
                       END
            ) AS parsed(raw_json)
            CROSS JOIN LATERAL (
                -- type は `->>` でエスケープを解いた文字列と比べる。旧 decoder (serde) も
                -- デコード後の String で見ていたので、日本語が Unicode エスケープで
                -- 書かれていても ("分前" など) 同じように拾える。
                -- `->>` が落ちるのは NUL を含むときだけで、それは上で無害化済み
                SELECT CASE parsed.raw_json->>'type'
                           WHEN '分前' THEN 'minutes'
                           WHEN '時間前' THEN 'hours'
                           WHEN '日前' THEN 'days'
                           WHEN '週間前' THEN 'weeks'
                       END
            ) AS mapped(unit)
            CROSS JOIN LATERAL (
                -- 重複しているキーは json_object_keys が重複したまま返す
                -- (オブジェクトでない値には使えないので CASE の中で呼ぶ)
                SELECT CASE
                           WHEN json_typeof(parsed.raw_json) = 'object'
                           THEN EXISTS (
                               SELECT 1
                               FROM json_object_keys(parsed.raw_json) AS keys(key_name)
                               WHERE key_name IN ('key', 'num', 'type')
                               GROUP BY key_name
                               HAVING count(*) > 1
                           )
                           ELSE FALSE
                       END
            ) AS dup(has_duplicate_known_keys)
            WHERE json_typeof(parsed.raw_json) = 'object'
              AND NOT dup.has_duplicate_known_keys
              -- 数値は `->` の text 表現 (JSON のトークンのまま) で見る。数値ならトークンのまま
              -- (30 / 1e2 / -0)、文字列なら引用符付き ("30")、キーが無ければ NULL になるので、
              -- 整数として書かれていたかどうかがこれで分かる。
              -- 符号は表記として許し、値の範囲で弾く (serde の i64 は -0 を 0 として読めていた)。
              --
              -- 桁数はまず正規表現で抑える: numeric の上限を超える桁数の整数トークンがあると
              -- ::numeric が overflow でエラーになり、マイグレーション全体が中断してしまう
              -- (旧 decoder はその要素を範囲外として無視するだけだった)。
              -- i64 は最大 19 桁、u32 は最大 10 桁なので、それを超えるものは必ず範囲外。
              -- WHERE の AND は評価順が保証されず、プランナが先に ::numeric を評価しうるので、
              -- 桁数を確かめてからキャストすることを CASE (評価順が保証される) で明示する
              AND CASE
                      WHEN (parsed.raw_json->'key')::text ~ '^-?[0-9]{1,19}$'
                      THEN (parsed.raw_json->'key')::text::numeric
                           BETWEEN -9223372036854775808 AND 9223372036854775807
                      ELSE FALSE
                  END
              AND CASE
                      WHEN (parsed.raw_json->'num')::text ~ '^-?[0-9]{1,10}$'
                      THEN (parsed.raw_json->'num')::text::numeric BETWEEN 0 AND 4294967295
                      ELSE FALSE
                  END
              AND unit IS NOT NULL
        ),
        '[]'::jsonb
    )
    END;
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
