-- 外部カレンダーから購読する iCal フィードのトークン (#95)。
-- サーバー管理権限を持つメンバーがサーバー設定から発行し、`GET /feeds/<token>.ics` (認証なし) で
-- そのギルドの予定を iCalendar 形式で配信する。URL を知っている人は誰でも読めるので、
-- 漏れたときは再発行 (行の置き換え) で古い URL を失効させる。
-- トークンは平文で保存する: 発行後もサーバー設定ダイアログでいつでも URL を表示・コピーできるようにするため
-- (Google カレンダーの「限定公開 URL」と同じ運用。ハッシュにすると発行時に一度しか見せられない)。
-- 新規テーブルなので旧実装 / bot とは共有しない
CREATE TABLE guild_feed_tokens (
    -- 1 ギルド 1 本。再発行は同じ行を置き換える。Snowflake ID は文字列で持つ (events.guild_id と同じ慣習)
    guild_id TEXT PRIMARY KEY,
    -- 32 バイトの乱数を 16 進にした 64 文字。配信時はこの値で行を引く
    token TEXT NOT NULL UNIQUE,
    -- JST の壁時計時刻 (events.created_at と同じ慣習)
    created_at TIMESTAMP NOT NULL,
    -- 発行した Discord ユーザー ID (文字列)
    created_by TEXT NOT NULL
);
