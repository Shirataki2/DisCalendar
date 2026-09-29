//! BotとWebで同じ回答と締切を扱い、編集versionを投票で変えない。
use discalendar_bot::poll_votes::{VoteOutcome, vote};
use sqlx::PgPool;
#[sqlx::test(migrations = "../api/migrations")]
async fn votes_are_shared_serialized_and_mark_the_message_dirty(pool: PgPool) {
    let poll:i32=sqlx::query_scalar("INSERT INTO schedule_polls(guild_id,title,created_by,created_at) VALUES ('1','調整','3',now()) RETURNING id").fetch_one(&pool).await.unwrap();
    let option:i32=sqlx::query_scalar("INSERT INTO schedule_poll_options(poll_id,start_at,end_at,is_all_day,position) VALUES ($1,'2099-01-01','2099-01-01',true,0) RETURNING id").bind(poll).fetch_one(&pool).await.unwrap();
    assert_eq!(
        vote(&pool, "2", poll, option, "3", "yes").await.unwrap(),
        VoteOutcome::NotFound
    );
    let (a, b) = tokio::join!(
        vote(&pool, "1", poll, option, "3", "yes"),
        vote(&pool, "1", poll, option, "3", "maybe")
    );
    assert_eq!(a.unwrap(), VoteOutcome::Saved);
    assert_eq!(b.unwrap(), VoteOutcome::Saved);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM schedule_poll_votes WHERE option_id=$1")
            .bind(option)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let state: (i32, i64) =
        sqlx::query_as("SELECT version,discord_revision FROM schedule_polls WHERE id=$1")
            .bind(poll)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, (1, 3));
    sqlx::query("UPDATE schedule_polls SET status='confirmed' WHERE id=$1")
        .bind(poll)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        vote(&pool, "1", poll, option, "3", "no").await.unwrap(),
        VoteOutcome::Closed
    );
    sqlx::query("DELETE FROM schedule_polls WHERE id=$1")
        .bind(poll)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        vote(&pool, "1", poll, option, "3", "yes").await.unwrap(),
        VoteOutcome::NotFound
    );
}
