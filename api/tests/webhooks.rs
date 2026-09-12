//! 配信の永続性・削除スナップショット・停止閾値を実 DB で確認する。
use discalendar_api::{webhook_outbox, webhooks};
use serde_json::Value;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn durable_outbox_and_failure_limit(pool: PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id,name,locale) VALUES ('111','Webhook','ja')")
        .execute(&pool)
        .await
        .unwrap();
    let hook: i64 = sqlx::query_scalar("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) VALUES ('111','http://127.0.0.1','json','test-only','222') RETURNING id").fetch_one(&pool).await.unwrap();
    let event: i32 = sqlx::query_scalar("INSERT INTO events (guild_id,name,color,is_all_day,start_at,end_at,created_at) VALUES ('111','削除前','#5865F2',false,now(),now(),now()) RETURNING id").fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    webhook_outbox::enqueue(&mut tx, "111", event, "event.created", "222")
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    // ワーカーが Webhook 行を処理中でも、予定保存の外部キー確認を待たせない。
    let mut worker_tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM guild_webhooks WHERE id=$1 FOR NO KEY UPDATE")
        .bind(hook)
        .execute(&mut *worker_tx)
        .await
        .unwrap();
    let mut save_tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *save_tx)
        .await
        .unwrap();
    webhook_outbox::enqueue(&mut save_tx, "111", event, "event.created", "222")
        .await
        .unwrap();
    save_tx.rollback().await.unwrap();
    worker_tx.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let mut tx = pool.begin().await.unwrap();
    webhook_outbox::enqueue(&mut tx, "111", event, "event.deleted", "222")
        .await
        .unwrap();
    sqlx::query("DELETE FROM events WHERE id=$1")
        .bind(event)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let snapshot: Value = sqlx::query_scalar("SELECT payload FROM guild_webhook_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(snapshot["name"], "削除前");
    assert_eq!(snapshot["guild_id"], "111");
    for attempt in 1..=10 {
        // SSRF 拒否を利用し、外部に通信せず同じ配信ワーカーの失敗経路を通す。
        if attempt > 1 && attempt % 3 == 1 {
            sqlx::query("INSERT INTO guild_webhook_outbox (webhook_id,kind,payload,actor_id) VALUES ($1,'webhook.test','null','222')").bind(hook).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE guild_webhook_outbox SET next_attempt_at=now()")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            webhooks::deliver_one(&pool, "https://discalendar.app")
                .await
                .unwrap()
        );
        let (enabled, failures): (bool, i32) =
            sqlx::query_as("SELECT enabled,consecutive_failures FROM guild_webhooks WHERE id=$1")
                .bind(hook)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(failures, attempt);
        assert_eq!(enabled, attempt < 10);
        if attempt % 3 == 0 {
            let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_outbox")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(pending, 0, "3回で打ち切る");
        }
    }
    assert!(
        !webhooks::deliver_one(&pool, "https://discalendar.app")
            .await
            .unwrap()
    );
    let logs: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(logs, 10);
}

#[sqlx::test(migrations = "./migrations")]
async fn bulk_delete_keeps_every_snapshot_and_discards_old_generations(pool: PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id,name) VALUES ('111','Webhook')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) VALUES ('111','http://127.0.0.1','json','test-only','222')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO events (guild_id,name,start_at,end_at) SELECT '111','削除前',now(),now() FROM generate_series(1,205)").execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let (snapshots, count) =
        discalendar_api::models::admin_ops::delete_guild_events(&mut tx, "111", "222")
            .await
            .unwrap();
    assert_eq!(snapshots.len(), 200);
    assert_eq!(count, 205);
    tx.commit().await.unwrap();
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_outbox WHERE actor_id='222' AND payload->>'name'='削除前'").fetch_one(&pool).await.unwrap();
    assert_eq!(queued, 205);
    // 停止・再開と並行して旧世代の予約が到着しても送信しない。
    sqlx::query("UPDATE guild_webhooks SET generation=1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        webhooks::deliver_one(&pool, "https://discalendar.app")
            .await
            .unwrap()
    );
    let attempts: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn deleting_a_webhook_concurrently_does_not_abort_event_save(pool: PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id,name) VALUES ('111','Webhook')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) VALUES ('111','https://example.com','json','test-only','222')").execute(&pool).await.unwrap();
    let event: i32 = sqlx::query_scalar("INSERT INTO events (guild_id,name,start_at,end_at) VALUES ('111','保存する予定',now(),now()) RETURNING id").fetch_one(&pool).await.unwrap();
    let mut deleting = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM guild_webhooks WHERE guild_id='111'")
        .execute(&mut *deleting)
        .await
        .unwrap();
    let mut saving = pool.begin().await.unwrap();
    {
        let enqueue = webhook_outbox::enqueue(&mut saving, "111", event, "event.created", "222");
        tokio::pin!(enqueue);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), &mut enqueue)
                .await
                .is_err()
        );
        deleting.commit().await.unwrap();
        enqueue.await.unwrap();
    }
    saving.commit().await.unwrap();
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn workers_skip_busy_webhooks(pool: PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id,name) VALUES ('111','Webhook')")
        .execute(&pool)
        .await
        .unwrap();
    let hooks: Vec<i64> = sqlx::query_scalar("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) SELECT '111','http://127.0.0.1','json','test-only','222' FROM generate_series(1,2) RETURNING id").fetch_all(&pool).await.unwrap();
    sqlx::query("INSERT INTO guild_webhook_outbox (webhook_id,kind,payload,actor_id) SELECT id,'webhook.test','null','222' FROM guild_webhooks").execute(&pool).await.unwrap();
    let mut busy = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM guild_webhooks WHERE id=$1 FOR NO KEY UPDATE")
        .bind(hooks[0])
        .execute(&mut *busy)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM guild_webhook_outbox WHERE webhook_id=$1 FOR UPDATE")
        .bind(hooks[0])
        .execute(&mut *busy)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            webhooks::deliver_one(&pool, "https://discalendar.app")
        )
        .await
        .unwrap()
        .unwrap()
    );
    let delivered: Vec<i64> = sqlx::query_scalar("SELECT webhook_id FROM guild_webhook_deliveries")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(delivered, vec![hooks[1]]);
    busy.rollback().await.unwrap();
}
