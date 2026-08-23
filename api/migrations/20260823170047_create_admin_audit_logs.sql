-- 管理コンソール (/admin) からの操作を記録する監査ログ (#34)。
-- 新規テーブルなので旧実装 / 旧 Bot とは共有しない。日時は既存テーブル (JST naive の TIMESTAMP) と違い
-- TIMESTAMPTZ で持つ (旧 Bot が読まないので互換性の制約がない)
CREATE TABLE admin_audit_logs (
    id BIGSERIAL PRIMARY KEY,
    -- 操作したユーザー (Better Auth の user.id と、連携済み Discord アカウントのユーザー ID)
    actor_user_id TEXT NOT NULL,
    actor_discord_user_id TEXT NOT NULL,
    -- 操作の種類 ("event.update" / "guild_config.update" / "sql.select" など。値は api 側の定数)
    action TEXT NOT NULL,
    -- 操作対象 ("event" + events.id、"guild" + guild_id など。対象がなければ NULL)
    target_type TEXT,
    target_id TEXT,
    -- 変更前後のスナップショット (読み取り操作では NULL)。SQL 実行など付随情報は detail に入れる
    before JSONB,
    after JSONB,
    detail JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 監査ログ画面は新しい順に見るのが基本
CREATE INDEX idx_admin_audit_logs_created_at ON admin_audit_logs (created_at DESC);
