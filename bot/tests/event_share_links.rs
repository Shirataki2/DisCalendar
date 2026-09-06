//! 通知は明示的に発行された共有リンクだけを読み、無効化・削除後は案内しない。

use discalendar_bot::models::{
    event_share_links,
    events::{self, NewEvent},
};
use sqlx::PgPool;

#[sqlx::test(migrations = "../api/migrations")]
async fn reads_only_published_links_for_the_event_and_guild(pool: PgPool) {
    let date = "2026-08-23T10:00:00".parse().unwrap();
    let event = events::create(
        &pool,
        &NewEvent {
            guild_id: "123",
            name: "定例",
            description: None,
            notifications: &[],
            color: "#2196F3",
            is_all_day: false,
            start_at: date,
            end_at: date,
            created_at: date,
            created_by: "456",
        },
    )
    .await
    .unwrap();
    assert_eq!(
        event_share_links::get_token(&pool, "123", event.id)
            .await
            .unwrap(),
        None
    );
    let token = "a".repeat(64);
    sqlx::query("INSERT INTO event_share_links (event_id, token) VALUES ($1, $2)")
        .bind(event.id)
        .bind(&token)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        event_share_links::get_token(&pool, "123", event.id)
            .await
            .unwrap(),
        Some(token.clone())
    );
    assert_eq!(
        event_share_links::get_token(&pool, "999", event.id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        event_share_links::get_token(&pool, "123", event.id + 1)
            .await
            .unwrap(),
        None
    );
    sqlx::query("DELETE FROM event_share_links WHERE event_id = $1")
        .bind(event.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        event_share_links::get_token(&pool, "123", event.id)
            .await
            .unwrap(),
        None
    );
    sqlx::query("INSERT INTO event_share_links (event_id, token) VALUES ($1, $2)")
        .bind(event.id)
        .bind(&token)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM events WHERE id = $1")
        .bind(event.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        event_share_links::get_token(&pool, "123", event.id)
            .await
            .unwrap(),
        None
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM event_share_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
