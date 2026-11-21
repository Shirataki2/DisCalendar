-- 予定の添付画像を Discord スケジュールイベントのカバー画像にする (#403)。
--
-- どの添付をカバーにするかは添付の行にフラグで持つ。events に参照カラムを足すと、
-- events を読み書きする api / bot の全クエリと Webhook の送信内容 (to_jsonb(e)) に波及するうえ、
-- 添付を消したときに参照が宙に浮く。添付側に持てば、添付の削除でカバーも一緒に消える。
--
-- DEFAULT 付きの NOT NULL カラムの追加は PostgreSQL 11 以降テーブルを書き換えない。
-- 古い api は event_attachments を `SELECT *` で読むが、知らないカラムは無視されるので動く
-- (ただしバージョンの記録があると起動できないので、戻すときは rollback/ の同名ファイルを流す)。
ALTER TABLE event_attachments
    ADD COLUMN discord_cover BOOLEAN NOT NULL DEFAULT false,
    -- Discord に送れるのは画像だけ。PDF がカバーになっている状態を作らない
    ADD CONSTRAINT event_attachments_discord_cover_image
        CHECK (NOT discord_cover OR content_type IN ('image/jpeg', 'image/png', 'image/webp'));

-- カバーは 1 予定 1 枚
CREATE UNIQUE INDEX event_attachments_one_discord_cover
    ON event_attachments (event_id) WHERE discord_cover;
