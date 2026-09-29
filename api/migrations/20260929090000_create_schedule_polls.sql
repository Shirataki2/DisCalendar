CREATE TABLE schedule_polls (
    id SERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL,
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 32),
    description TEXT CHECK (char_length(description) <= 1000),
    created_by TEXT NOT NULL,
    deadline TIMESTAMP,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'confirmed')),
    confirmed_event_id INTEGER REFERENCES events(id) ON DELETE SET NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL
);
CREATE INDEX schedule_polls_guild_id ON schedule_polls (guild_id, id DESC);
CREATE TABLE schedule_poll_options (
    id SERIAL PRIMARY KEY,
    poll_id INTEGER NOT NULL REFERENCES schedule_polls(id) ON DELETE CASCADE,
    start_at TIMESTAMP NOT NULL,
    end_at TIMESTAMP NOT NULL CHECK (end_at >= start_at),
    is_all_day BOOLEAN NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 4),
    UNIQUE (poll_id, position) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE schedule_poll_votes (
    option_id INTEGER NOT NULL REFERENCES schedule_poll_options(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    answer TEXT NOT NULL CHECK (answer IN ('yes', 'maybe', 'no')),
    updated_at TIMESTAMP NOT NULL,
    PRIMARY KEY (option_id, user_id)
);
