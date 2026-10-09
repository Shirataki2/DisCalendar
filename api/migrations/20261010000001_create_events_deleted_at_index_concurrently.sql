-- no-transaction
-- ゴミ箱 (#159) の一覧 (ギルドごとに新しい順) と期限切れの定期削除用。削除済みの行だけを持つ小さな部分インデックス。
--
-- 通常の CREATE INDEX は events 全体を走査する間、予定の作成・更新を止めてしまうので、
-- 既存の 20260827101818 と同じく CREATE INDEX CONCURRENTLY を使う
-- (トランザクション内では実行できないので先頭の "-- no-transaction" が要る。
-- 列を足す直前のマイグレーションは別トランザクションなので、ここに来る時点で排他ロックは解放されている)。
--
-- CONCURRENTLY が接続切断・キャンセルなどで失敗すると同名の INVALID なインデックスが
-- 残ることがある。その掃除は `api::cleanup_invalid_concurrent_indexes` (api/src/lib.rs) が
-- マイグレーション実行の直前に毎回試みる
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_deleted_at
    ON events (guild_id, deleted_at) WHERE deleted_at IS NOT NULL;
