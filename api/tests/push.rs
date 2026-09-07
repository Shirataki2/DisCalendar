//! 購読の本人境界・上限・外部キー・配信対象を実 DB で確認する。
use actix_web::{App, HttpResponse, HttpServer, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use discalendar_api::{
    models::{
        push::{self, PushScope, SubscriptionInput},
        user_activity,
    },
    push::expand,
};
use sqlx::PgPool;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

fn input(id: usize) -> SubscriptionInput {
    let mut public = [1; 65];
    public[0] = 4;
    SubscriptionInput {
        endpoint: format!("https://fcm.googleapis.com/fcm/send/test-{id}"),
        p256dh: URL_SAFE_NO_PAD.encode(public),
        auth: URL_SAFE_NO_PAD.encode([1; 16]),
        device_name: format!("端末{id}"),
    }
}
#[test]
fn validates_subscription_without_allowing_arbitrary_http_targets() {
    assert!(input(1).validate().is_ok());
    for endpoint in [
        "http://fcm.googleapis.com/a",
        "https://localhost/a",
        "https://127.0.0.1/a",
        "https://fcm.googleapis.com.evil.example/a",
        "https://user@fcm.googleapis.com/a",
        "https://fcm.googleapis.com:444/a",
        "https://evil.example/?host=fcm.googleapis.com",
    ] {
        let mut i = input(1);
        i.endpoint = endpoint.into();
        assert!(i.validate().is_err());
    }
    let mut i = input(1);
    i.auth = "bad".into();
    assert!(i.validate().is_err());
    i = input(1);
    i.device_name = " ".into();
    assert!(i.validate().is_err());
}
#[sqlx::test(migrations = "./migrations")]
async fn owns_devices_and_serializes_the_ten_device_limit(pool: PgPool) {
    push::set_scope(&pool, "u1", &PushScope::Created)
        .await
        .unwrap();
    for n in 0..9 {
        push::subscribe(&pool, "u1", &input(n)).await.unwrap();
    }
    let a = input(9);
    let b = input(10);
    let (a, b) = tokio::join!(
        push::subscribe(&pool, "u1", &a),
        push::subscribe(&pool, "u1", &b)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(matches!(
        push::subscribe(&pool, "u2", &input(0)).await,
        Err(discalendar_api::error::ApiError::Conflict(_))
    ));
    let settings = push::get(&pool, "u1").await.unwrap();
    assert_eq!(settings.scope, "created");
    assert_eq!(settings.subscriptions.len(), 10);
    assert!(
        push::get(&pool, "u2")
            .await
            .unwrap()
            .subscriptions
            .is_empty()
    );
    let id = settings.subscriptions[0].id;
    push::remove(&pool, "u2", id).await.unwrap();
    push::remove_endpoint(&pool, "u2", &input(0).endpoint)
        .await
        .unwrap();
    assert_eq!(
        push::get(&pool, "u1").await.unwrap().subscriptions.len(),
        10
    );
    let json = serde_json::to_string(&settings).unwrap();
    assert!(!json.contains("endpoint"));
    assert!(!json.contains("p256dh"));
    assert!(!json.contains("auth"));
    push::remove(&pool, "u1", id).await.unwrap();
    assert_eq!(push::get(&pool, "u1").await.unwrap().subscriptions.len(), 9);
    push::subscribe(&pool, "u1", &input(1)).await.unwrap();
    assert_eq!(push::get(&pool, "u1").await.unwrap().subscriptions.len(), 9);
}
#[sqlx::test(migrations = "./migrations")]
async fn fk_is_added_after_better_auth_and_cascades_deletion(pool: PgPool) {
    let mut conn = pool.acquire().await.unwrap();
    assert!(!user_activity::ensure_user_fk(&mut conn).await.unwrap());
    let complete = tokio::spawn(user_activity::complete_user_fks(pool.clone()));
    push::subscribe(&pool, "ghost", &input(0)).await.unwrap();
    push::set_scope(&pool, "ghost", &PushScope::All)
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE \"user\" (id TEXT PRIMARY KEY); INSERT INTO \"user\" VALUES ('u1');",
    )
    .execute(&pool)
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), complete)
        .await
        .unwrap()
        .unwrap();
    assert!(user_activity::ensure_user_fk(&mut conn).await.unwrap());
    assert!(
        push::get(&pool, "ghost")
            .await
            .unwrap()
            .subscriptions
            .is_empty()
    );
    assert_eq!(push::get(&pool, "ghost").await.unwrap().scope, "off");
    push::subscribe(&pool, "u1", &input(1)).await.unwrap();
    push::set_scope(&pool, "u1", &PushScope::All).await.unwrap();
    sqlx::query("DELETE FROM \"user\" WHERE id='u1'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        push::get(&pool, "u1")
            .await
            .unwrap()
            .subscriptions
            .is_empty()
    );
    assert_eq!(push::get(&pool, "u1").await.unwrap().scope, "off");
}
#[sqlx::test(migrations = "./migrations")]
async fn outbox_targets_scope_creator_and_registration_time_idempotently(pool: PgPool) {
    sqlx::raw_sql(r#"
      CREATE TABLE "account" ("userId" TEXT, "providerId" TEXT, "accountId" TEXT);
      INSERT INTO "account" VALUES ('all','discord','1'),('created','discord','2'),('other','discord','3'),('off','discord','4'),('new','discord','5'),('nonmember','discord','6'),('retry','discord','7');
      INSERT INTO guilds (guild_id,name,locale) VALUES ('111','サーバー','ja');
      INSERT INTO guild_config (guild_id,restricted) VALUES ('111',true);
      INSERT INTO events (guild_id,name,color,is_all_day,start_at,end_at,created_by)
        SELECT '111','予定','#5865F2',false,now() AT TIME ZONE 'Asia/Tokyo',now() AT TIME ZONE 'Asia/Tokyo','2' FROM generate_series(1,2);
    "#).execute(&pool).await.unwrap();
    for (n, user, scope) in [
        (0, "all", PushScope::All),
        (1, "created", PushScope::Created),
        (2, "other", PushScope::Created),
        (3, "off", PushScope::Off),
        (4, "new", PushScope::All),
        (5, "nonmember", PushScope::All),
        (6, "retry", PushScope::All),
    ] {
        push::subscribe(&pool, user, &input(n)).await.unwrap();
        push::set_scope(&pool, user, &scope).await.unwrap();
    }
    sqlx::raw_sql(r#"
      UPDATE push_subscriptions SET created_at = now() - interval '1 hour' WHERE user_id <> 'new';
      INSERT INTO push_outbox (event_id,fire_at,start_at) SELECT id, (now() AT TIME ZONE 'Asia/Tokyo') - interval '1 minute', start_at FROM events;
    "#).execute(&pool).await.unwrap();
    let single_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let transient = Arc::new(AtomicBool::new(true));
    let server_calls = calls.clone();
    let server_pool = single_pool.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        let calls = server_calls.clone();
        let pool = server_pool.clone();
        let transient = transient.clone();
        App::new().route(
            "/guilds/111/members/{id}",
            web::get().to(move |id: web::Path<String>| {
                let calls = calls.clone();
                let pool = pool.clone();
                let transient = transient.clone();
                async move {
                    // 所属確認中に唯一の接続を占有していないことも確認する。
                    sqlx::query("SELECT 1").execute(&pool).await.unwrap();
                    calls.fetch_add(1, Ordering::SeqCst);
                    match id.as_str() {
                        "6" => HttpResponse::NotFound().finish(),
                        "7" if transient.swap(false, Ordering::SeqCst) => {
                            HttpResponse::InternalServerError().finish()
                        }
                        _ => HttpResponse::Ok().json(serde_json::json!({"roles":[]})),
                    }
                }
            }),
        )
    })
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let discord =
        discalendar_api::discord::DiscordClient::new("test", &format!("http://{address}")).unwrap();
    tokio::time::timeout(Duration::from_secs(5), expand(&single_pool, &discord))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4); // 2予定でも所属確認は利用者ごとに1回。
    let targets: Vec<String>=sqlx::query_scalar("SELECT s.user_id FROM push_deliveries d JOIN push_subscriptions s ON s.id=d.subscription_id ORDER BY s.user_id").fetch_all(&pool).await.unwrap();
    assert_eq!(targets, vec!["all", "all", "created", "created"]);
    let expanded: bool = sqlx::query_scalar("SELECT bool_or(expanded) FROM push_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!expanded);
    expand(&single_pool, &discord).await.unwrap();
    expand(&single_pool, &discord).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 5); // 一時エラーだけ再照会する。
    let targets: Vec<String>=sqlx::query_scalar("SELECT s.user_id FROM push_deliveries d JOIN push_subscriptions s ON s.id=d.subscription_id ORDER BY s.user_id").fetch_all(&pool).await.unwrap();
    assert_eq!(
        targets,
        vec!["all", "all", "created", "created", "retry", "retry"]
    );
    let expanded: bool = sqlx::query_scalar("SELECT bool_and(expanded) FROM push_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(expanded);
    handle.stop(true).await;
    push::remove_endpoint(&pool, "all", &input(0).endpoint)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM push_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 4);
    sqlx::query("DELETE FROM events")
        .execute(&pool)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM push_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn rollback_removes_only_push_tables_and_its_migration(pool: PgPool) {
    sqlx::raw_sql(include_str!(
        "../rollback/20260907090000_drop_push_notifications.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let missing: bool=sqlx::query_scalar("SELECT to_regclass('push_subscriptions') IS NULL AND to_regclass('user_push_settings') IS NULL AND to_regclass('push_outbox') IS NULL AND to_regclass('push_deliveries') IS NULL AND to_regclass('events') IS NOT NULL AND to_regclass('user_daily_activity') IS NOT NULL")
        .fetch_one(&pool).await.unwrap();
    assert!(missing);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version = 20260907090000")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
