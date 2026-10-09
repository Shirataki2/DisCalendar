-- 予定の変更履歴 (#165)。利用者・Bot・管理コンソールからの作成・更新を予定ごとに残し、予定の詳細から見られるようにする。
-- 構造は admin_audit_logs (#34) に寄せる。created_at は TIMESTAMPTZ (表示の変換は web) だが、
-- before / after の中の日時は events と同じ JST naive の文字列 (`YYYY-MM-DDTHH:MM:SS`) で持つ
CREATE TABLE event_history (
    id BIGSERIAL PRIMARY KEY,
    -- 予定が完全に消えたら履歴も要らないので一緒に消す
    event_id INTEGER NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    -- 絞り込み (他ギルドの予定の履歴を返さない) と後片付け用
    guild_id TEXT NOT NULL,
    -- 操作した Discord ユーザー。利用者以外の操作 (将来の自動処理など) は NULL
    actor_discord_user_id TEXT,
    -- web: 通常の画面・API / bot: スラッシュコマンド / admin: 管理コンソール / mcp: AI アシスタント (MCP)
    source TEXT NOT NULL CHECK (source IN ('web', 'bot', 'admin', 'mcp')),
    -- delete / restore はゴミ箱 (#159) で予定が論理削除になったら使う
    action TEXT NOT NULL CHECK (action IN ('create', 'update', 'delete', 'restore')),
    -- 予定のスナップショット (name, description, location, color, is_all_day, start_at, end_at,
    -- notifications, notification_mentions, discord_linked)。create は before が NULL、delete は after が NULL
    before JSONB,
    after JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 予定の詳細から新しい順に読む
CREATE INDEX idx_event_history_event_id ON event_history (event_id, id DESC);
-- 保持期間 (180 日) を過ぎた行の定期削除用
CREATE INDEX idx_event_history_created_at ON event_history (created_at);
