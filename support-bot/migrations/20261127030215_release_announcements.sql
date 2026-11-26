-- 本番に出た版の更新内容を「更新通知」チャンネルへ告知した記録 (#430)。
-- 起動時に自分の版 (CARGO_PKG_VERSION) を見て、ここにある版より新しい正式版のときだけ告知する
-- (再起動・デプロイの再実行で同じ版を二度出さず、前の版へのロールバックで古い版を告知し直さないため)。
--
-- IF NOT EXISTS にしてあるのは、ロールバック (support-bot/rollback/) で `_sqlx_migrations` の記録だけを消して
-- テーブルは残すため。もう一度この版へ進めたとき、このマイグレーションが当たり直っても告知の記録は消えない
CREATE TABLE IF NOT EXISTS support_release_announcements (
    -- 告知した版 (3.31.0 の形。v は付けない)
    version TEXT PRIMARY KEY,
    -- 投稿したメッセージ。更新履歴に節が無い版 (告知しなかった版) は NULL
    message_id BIGINT,
    channel_id BIGINT,
    announced_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
