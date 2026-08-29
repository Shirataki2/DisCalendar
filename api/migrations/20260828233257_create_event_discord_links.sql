-- 予定と Discord スケジュールイベントの対応付け (#94)。
-- ダイアログで「Discord のイベントとしても作成する」を有効にした予定について、
-- api が Guild Scheduled Events API で作ったイベントの ID をここで対応付ける。
-- 作成・更新・削除の同期はこの表を読んだ api が明示的に Discord を呼んで行う。
-- 新規テーブルなので旧実装 / 旧 Bot とは共有しない
CREATE TABLE event_discord_links (
    -- 予定が消えたら対応付けも一緒に消す (Discord 側のイベント削除は api が削除前に
    -- この行を読んで呼ぶので、CASCADE は取りこぼしの保険も兼ねる)
    event_id INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    -- Snowflake ID は文字列で持つ (events.guild_id と同じ慣習)。
    -- 予定と同様に「guild_id + event_id」で絞って他ギルドからの読み書きを防ぐ
    guild_id TEXT NOT NULL,
    scheduled_event_id TEXT NOT NULL,
    -- JST の壁時計時刻 (events.created_at と同じ慣習)
    created_at TIMESTAMP NOT NULL
);

-- ギルド単位の操作 (管理コンソールの一括削除) 用
CREATE INDEX idx_event_discord_links_guild_id ON event_discord_links (guild_id);
