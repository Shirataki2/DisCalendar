//! 上限と削除経路を実DBで確認する。R2の通信・認可はE2Eでも検証する。
use discalendar_api::{
    attachments::Store,
    models::attachments::{self, FILE_MAX_BYTES, GUILD_MAX_BYTES, UploadInput},
};
use sqlx::PgPool;

async fn event(pool: &PgPool, guild: &str) -> i32 {
    sqlx::query_scalar("INSERT INTO events(guild_id,name,color,is_all_day,start_at,end_at,notifications) VALUES ($1,'添付テスト','#000000',false,now(),now(),'[]') RETURNING id")
        .bind(guild).fetch_one(pool).await.unwrap()
}
fn input(size: i64) -> UploadInput {
    UploadInput {
        filename: "案内.png".into(),
        size,
        content_type: "image/png".into(),
    }
}

#[test]
fn file_validation_and_signatures() {
    for size in [1, FILE_MAX_BYTES] {
        input(size).validate().unwrap();
    }
    for size in [0, -1, FILE_MAX_BYTES + 1] {
        assert!(input(size).validate().is_err());
    }
    for name in [
        "案内.svg",
        "../案内.png",
        "改\n行.png",
        "a\\b.png",
        &"x".repeat(256),
    ] {
        let mut value = input(10);
        value.filename = name.into();
        assert!(value.validate().is_err());
    }
    for (bytes, mime) in [
        (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
        (b"\xff\xd8\xff", "image/jpeg"),
        (b"RIFF1234WEBP", "image/webp"),
        (b"%PDF-1.7", "application/pdf"),
    ] {
        assert_eq!(attachments::detected_type(bytes), Some(mime));
    }
    assert_eq!(attachments::detected_type(b"<svg></svg>"), None);
    assert_eq!(attachments::detected_type(b""), None);
}

#[sqlx::test(migrations = "./migrations")]
async fn reservations_are_private_and_quota_is_atomic(pool: PgPool) {
    let id = event(&pool, "111").await;
    assert!(
        attachments::reserve(&pool, "222", id, &input(1))
            .await
            .is_err()
    );
    let mut tasks = Vec::new();
    for _ in 0..15 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            attachments::reserve(&pool, "111", id, &input(1))
                .await
                .is_ok()
        }));
    }
    let mut successes = 0;
    for task in tasks {
        successes += usize::from(task.await.unwrap());
    }
    assert_eq!(successes, 10);
    assert!(
        attachments::list(&pool, "111", id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(attachments::used_bytes(&pool, "111").await.unwrap(), 10);
}

#[sqlx::test(migrations = "./migrations")]
async fn guild_capacity_includes_other_events_and_pending_files(pool: PgPool) {
    let id = event(&pool, "111").await;
    // 既存添付の合計を1GiBの1バイト手前にする。各行はファイル上限を守る。
    sqlx::query("INSERT INTO event_attachments(id,event_id,guild_id,filename,size,content_type,temporary_key,object_key) SELECT 'q'||i,$1,'111','a.png',LEAST(10485760,1073741823-(i-1)*10485760),'image/png','temporary/q'||i,'attachments/q'||i FROM generate_series(1,103) i")
        .bind(id).execute(&pool).await.unwrap();
    let another = event(&pool, "111").await;
    assert_eq!(
        attachments::used_bytes(&pool, "111").await.unwrap(),
        GUILD_MAX_BYTES - 1
    );
    attachments::reserve(&pool, "111", another, &input(1))
        .await
        .unwrap();
    assert!(
        attachments::reserve(&pool, "111", another, &input(1))
            .await
            .is_err()
    );
    let other_guild = event(&pool, "222").await;
    attachments::reserve(&pool, "222", other_guild, &input(1))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn deletion_outbox_survives_all_sql_paths_but_not_rollback(pool: PgPool) {
    sqlx::query("INSERT INTO guilds(guild_id,name) VALUES ('111','テスト')")
        .execute(&pool)
        .await
        .unwrap();
    let id = event(&pool, "111").await;
    let row = attachments::reserve(&pool, "111", id, &input(1))
        .await
        .unwrap();
    sqlx::query("DELETE FROM guilds WHERE guild_id='111'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(attachments::used_bytes(&pool, "111").await.unwrap(), 1);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM events WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM attachment_deletions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DELETE FROM events WHERE guild_id='111'")
        .execute(&pool)
        .await
        .unwrap();
    let keys: Vec<String> = sqlx::query_scalar("SELECT object_key FROM attachment_deletions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&row.object_key) && keys.contains(&row.temporary_key));
}

#[sqlx::test(migrations = "./migrations")]
async fn signed_urls_bind_size_type_and_never_expose_upload_key_for_reading(pool: PgPool) {
    let id = event(&pool, "111").await;
    let row = attachments::reserve(&pool, "111", id, &input(10))
        .await
        .unwrap();
    let store = Store::new(
        "https://example.r2.cloudflarestorage.com",
        "attachments",
        "test-key",
        "test-secret",
    );
    let upload = store.upload_url(&row).await.unwrap();
    let parsed = reqwest::Url::parse(&upload.url).unwrap();
    let signed = parsed
        .query_pairs()
        .find(|(key, _)| key == "X-Amz-SignedHeaders")
        .unwrap()
        .1;
    assert!(
        signed.contains("content-length")
            && signed.contains("content-type")
            && signed.contains("if-none-match")
    );
    assert!(!upload.headers.contains_key("content-length"));
    assert_eq!(upload.headers["content-type"], "image/png");
    assert_eq!(upload.headers["if-none-match"], "*");
    assert!(parsed.path().contains("/temporary/"));
    let read = store.read_url(&row, false).await.unwrap();
    let parsed = reqwest::Url::parse(&read.url).unwrap();
    assert!(!parsed.path().contains("/temporary/"));
    let disposition = parsed
        .query_pairs()
        .find(|(key, _)| key == "response-content-disposition")
        .unwrap()
        .1;
    assert!(disposition.starts_with("attachment;") && disposition.contains("filename*=UTF-8''"));
    assert!(
        parsed
            .query_pairs()
            .any(|(key, value)| key == "response-cache-control" && value == "no-store")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_and_failed_deletions_retry_after_restart(pool: PgPool) {
    use actix_web::{App, HttpResponse, HttpServer, web};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let failing = Arc::new(AtomicBool::new(true));
    let state = failing.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .default_service(web::to(|flag: web::Data<Arc<AtomicBool>>| async move {
                if flag.load(Ordering::Relaxed) {
                    HttpResponse::ServiceUnavailable().finish()
                } else {
                    HttpResponse::NoContent().finish()
                }
            }))
    })
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let id = event(&pool, "111").await;
    attachments::reserve(&pool, "111", id, &input(1))
        .await
        .unwrap();
    sqlx::query("UPDATE event_attachments SET expires_at=now()-INTERVAL '1 hour',created_at=now()-INTERVAL '2 days'").execute(&pool).await.unwrap();
    let store = Store::new(&format!("http://{addr}"), "test", "key", "secret");
    discalendar_api::attachments::cleanup_once(&pool, &store)
        .await
        .unwrap();
    assert_eq!(attachments::used_bytes(&pool, "111").await.unwrap(), 0);
    let attempts: i32 = sqlx::query_scalar("SELECT max(attempts) FROM attachment_deletions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 1);
    failing.store(false, Ordering::Relaxed);
    sqlx::query("UPDATE attachment_deletions SET next_attempt_at=now()")
        .execute(&pool)
        .await
        .unwrap();
    // 別のStoreを作って、ワーカー再起動後もDBの記録から再開できることを確認する。
    let restarted = Store::new(&format!("http://{addr}"), "test", "key", "secret");
    discalendar_api::attachments::cleanup_once(&pool, &restarted)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM attachment_deletions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    handle.stop(true).await;
}
