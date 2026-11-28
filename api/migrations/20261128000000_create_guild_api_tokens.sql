-- サーバー用の API トークン (#405)。外部システム・スクリプトからの予定作成・更新・削除に使う。
-- トークン本体は SHA-256 のハッシュのみ保存し、平文は発行・ローテーション時のレスポンスに 1 回だけ返す。

CREATE TABLE guild_api_tokens (
    id BIGSERIAL PRIMARY KEY,
    guild_id VARCHAR(20) NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- 一覧で識別するための先頭プレフィックス (例: dct_a1b2c3d4)
    token_prefix TEXT NOT NULL,
    -- 平文トークン全体の SHA-256 (小文字 16 進)
    token_hash TEXT NOT NULL UNIQUE,
    -- create: 予定の作成のみ / write: 作成・更新・削除
    scope TEXT NOT NULL CHECK (scope IN ('create', 'write')),
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_guild_api_tokens_guild_id ON guild_api_tokens (guild_id, id DESC);

-- トークンごとの直近の呼び出しログ (エラー種別のみ記録し、秘密情報や入力値は残さない)
CREATE TABLE guild_api_token_logs (
    id BIGSERIAL PRIMARY KEY,
    token_id BIGINT NOT NULL REFERENCES guild_api_tokens(id) ON DELETE CASCADE,
    guild_id VARCHAR(20) NOT NULL,
    method TEXT NOT NULL,
    action TEXT NOT NULL,
    event_id INTEGER,
    external_id TEXT,
    status INTEGER NOT NULL,
    error_kind TEXT,
    called_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX idx_guild_api_token_logs_token_id ON guild_api_token_logs (token_id, id DESC);

-- Idempotency-Key ヘッダによる再送時の重複作成防止。response_body は 24 時間後に NULL 化し墓標として残す
CREATE TABLE guild_api_token_operations (
    guild_id VARCHAR(20) NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    token_id BIGINT NOT NULL REFERENCES guild_api_tokens(id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    status INTEGER NOT NULL,
    event_id INTEGER,
    response_body JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (guild_id, token_id, key_hash)
);

CREATE INDEX idx_guild_api_token_operations_expiry
    ON guild_api_token_operations (created_at)
    WHERE response_body IS NOT NULL;

-- 外部システム側の ID と予定の対応付け (同じ外部 ID で更新・削除するため)
CREATE TABLE event_external_ids (
    guild_id VARCHAR(20) NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    event_id INTEGER NOT NULL UNIQUE REFERENCES events(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (guild_id, external_id)
);

-- 変更履歴に「API トークン: <名前>」を残せるようにする
ALTER TABLE event_history ADD COLUMN actor_label TEXT;
ALTER TABLE event_history DROP CONSTRAINT event_history_source_check;
ALTER TABLE event_history
    ADD CONSTRAINT event_history_source_check
    CHECK (source IN ('web', 'bot', 'admin', 'mcp', 'api_token'));

-- API トークンから繰り返し予定を作成・更新したとき created_by を NULL にできるようにする
ALTER TABLE event_series ALTER COLUMN created_by DROP NOT NULL;
