-- 投稿時刻は JST。投稿済み日付は Bot だけが更新し、設定保存では維持する。
CREATE TABLE guild_digest_settings (
    guild_id TEXT PRIMARY KEY,
    daily_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    daily_time TIME NOT NULL DEFAULT '08:00',
    weekly_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    weekly_day SMALLINT NOT NULL DEFAULT 0 CHECK (weekly_day BETWEEN 0 AND 6),
    weekly_time TIME NOT NULL DEFAULT '08:00',
    skip_empty BOOLEAN NOT NULL DEFAULT TRUE,
    last_daily_date DATE,
    last_weekly_date DATE,
    CHECK (daily_time < '24:00'::TIME AND EXTRACT(SECOND FROM daily_time) = 0),
    CHECK (weekly_time < '24:00'::TIME AND EXTRACT(SECOND FROM weekly_time) = 0)
);
