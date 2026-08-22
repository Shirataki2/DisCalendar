-- Add migration script here
-- events.start_at に貼るインデックス (次のマイグレーションで CREATE INDEX CONCURRENTLY する)
-- の下準備。CONCURRENTLY での作成中に接続切断やプロセス停止が起きると、同名の INVALID な
-- (使われない) インデックスが残ることがある。その状態のまま次のマイグレーションを実行すると
-- IF NOT EXISTS があっても構文上は "既に存在する" ので誤ってスキップされてしまい、
-- IF NOT EXISTS を外すと逆に "already exists" で毎回失敗してサービスが起動できなくなる。
-- ここで無効なインデックスだけを検出して削除しておくことで、次のマイグレーションが
-- 「未作成」「有効なインデックスが既にある (CREATE 自体は成功したが完了記録前にクラッシュした)」
-- のどちらのケースでも安全に (再) 実行できるようにする。
-- 通常はこのインデックスがまだ存在しないか有効な状態なので、この DO ブロックは何もしない
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE c.relname = 'idx_events_start_at' AND NOT i.indisvalid
    ) THEN
        DROP INDEX idx_events_start_at;
    END IF;
END $$;
