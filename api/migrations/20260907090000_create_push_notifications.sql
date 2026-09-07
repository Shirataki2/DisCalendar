-- 個人の通知範囲は端末をすべて解除しても保持する。
CREATE TABLE user_push_settings (
    user_id TEXT PRIMARY KEY,
    scope TEXT NOT NULL DEFAULT 'off' CHECK (scope IN ('all', 'created', 'off'))
);
CREATE TABLE push_subscriptions (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    endpoint TEXT NOT NULL UNIQUE CHECK (length(endpoint) <= 2048),
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    device_name TEXT NOT NULL CHECK (char_length(device_name) BETWEEN 1 AND 80),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_sent_at TIMESTAMPTZ,
    failure_count INTEGER NOT NULL DEFAULT 0,
    disabled BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX idx_push_subscriptions_user ON push_subscriptions (user_id);
DO $$
BEGIN
    IF to_regclass('public."user"') IS NOT NULL THEN
        ALTER TABLE user_push_settings ADD CONSTRAINT user_push_settings_user_id_fkey
            FOREIGN KEY (user_id) REFERENCES "user" (id) ON DELETE CASCADE;
        ALTER TABLE push_subscriptions ADD CONSTRAINT push_subscriptions_user_id_fkey
            FOREIGN KEY (user_id) REFERENCES "user" (id) ON DELETE CASCADE;
    END IF;
END
$$;
-- Bot は発火だけを記録。API が宛先を展開し、端末ごとの送信・再試行を行う。
CREATE TABLE push_outbox (
    id BIGSERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    fire_at TIMESTAMP NOT NULL,
    start_at TIMESTAMP NOT NULL,
    expanded BOOLEAN NOT NULL DEFAULT false,
    UNIQUE (event_id, fire_at)
);
CREATE TABLE push_deliveries (
    outbox_id BIGINT NOT NULL REFERENCES push_outbox(id) ON DELETE CASCADE,
    subscription_id INTEGER NOT NULL REFERENCES push_subscriptions(id) ON DELETE CASCADE,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    done BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (outbox_id, subscription_id)
);
CREATE INDEX idx_push_deliveries_pending ON push_deliveries (next_attempt_at) WHERE NOT done;
