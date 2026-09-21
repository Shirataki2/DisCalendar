-- 繰り返しの定義と、既存予定IDを持つ各開催回。日時は既存どおりJST。
CREATE TABLE event_series (
    id SERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL,
    template JSONB NOT NULL CHECK (jsonb_typeof(template) = 'object'),
    recurrence JSONB NOT NULL CHECK (jsonb_typeof(recurrence) = 'object'),
    rrule TEXT NOT NULL,
    start_at TIMESTAMP NOT NULL,
    end_at TIMESTAMP NOT NULL CHECK (end_at >= start_at),
    end_before TIMESTAMP,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL,
    created_by TEXT NOT NULL,
    updated_at TIMESTAMP,
    updated_by TEXT,
    generated_until TIMESTAMP,
    UNIQUE (id, guild_id)
);
CREATE INDEX ON event_series (guild_id);
ALTER TABLE events
    ADD COLUMN series_id INTEGER,
    ADD COLUMN original_start_at TIMESTAMP,
    ADD CONSTRAINT events_series_pair CHECK ((series_id IS NULL) = (original_start_at IS NULL)),
    ADD CONSTRAINT events_series_guild_fk FOREIGN KEY (series_id, guild_id)
        REFERENCES event_series (id, guild_id),
    ADD CONSTRAINT events_series_occurrence_unique UNIQUE (series_id, original_start_at);
CREATE TABLE event_series_exceptions (
    series_id INTEGER NOT NULL REFERENCES event_series(id) ON DELETE CASCADE,
    original_start_at TIMESTAMP NOT NULL,
    event_id INTEGER UNIQUE REFERENCES events(id) ON DELETE SET NULL,
    PRIMARY KEY (series_id, original_start_at)
);
