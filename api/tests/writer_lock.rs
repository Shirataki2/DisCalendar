//! 専用接続の上限・通常プールの独立性・解放を実DBで検証する。
use discalendar_api::{error::ApiError, models::event_links::lock_writer};
use sqlx::{PgPool, postgres::PgPoolOptions};

#[sqlx::test(migrations = "./migrations")]
async fn writer_connections_are_bounded_and_released(pool: PgPool) {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let mut guards = Vec::new();
    for guild in 0..5 {
        guards.push(lock_writer(&pool, &guild.to_string()).await.unwrap());
    }
    assert!(matches!(
        lock_writer(&pool, "overflow").await,
        Err(ApiError::Conflict(_))
    ));
    // 専用枠を使い切っても通常プールの読み書きは進む。
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT 1").execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    guards.pop();
    let guard = lock_writer(&pool, "replacement").await.unwrap();
    drop(guard);
    guards.clear();
    // 同じギルドの競合で返した接続・枠も解放する。
    let guard = lock_writer(&pool, "held").await.unwrap();
    assert!(matches!(
        lock_writer(&pool, "held").await,
        Err(ApiError::Conflict(_))
    ));
    for guild in 0..4 {
        guards.push(lock_writer(&pool, &format!("fresh-{guild}")).await.unwrap());
    }
    drop(guard);
}
