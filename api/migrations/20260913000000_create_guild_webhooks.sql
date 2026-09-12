-- 予定の変更と同じトランザクションで配信を予約する。予定の削除後も本文を保持する。
CREATE TABLE guild_webhooks (
    id bigserial PRIMARY KEY,
    guild_id varchar(20) NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    url text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('json', 'discord')),
    secret text NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    generation bigint NOT NULL DEFAULT 0,
    consecutive_failures integer NOT NULL DEFAULT 0,
    disabled_reason text,
    created_by text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ON guild_webhooks (guild_id);
CREATE TABLE guild_webhook_outbox (
    id bigserial PRIMARY KEY,
    webhook_id bigint NOT NULL REFERENCES guild_webhooks(id) ON DELETE CASCADE,
    event_id integer,
    generation bigint NOT NULL DEFAULT 0,
    kind text NOT NULL,
    payload jsonb NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    actor_id text NOT NULL,
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ON guild_webhook_outbox (webhook_id, id);
CREATE TABLE guild_webhook_deliveries (
    id bigserial PRIMARY KEY,
    webhook_id bigint NOT NULL REFERENCES guild_webhooks(id) ON DELETE CASCADE,
    delivery_id text NOT NULL,
    kind text NOT NULL,
    attempted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    status integer,
    error text
);
CREATE INDEX ON guild_webhook_deliveries (webhook_id, id DESC);
