-- 予定ごとに明示的に発行する共有リンク (#154)。再コピーのため平文で保存する。
-- 32 バイトの乱数を 16 進にした推測不能なトークン。予定の削除と同時に失効する。
CREATE TABLE event_share_links (
    event_id INTEGER PRIMARY KEY REFERENCES events(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE CHECK (token ~ '^[0-9a-f]{64}$')
);
