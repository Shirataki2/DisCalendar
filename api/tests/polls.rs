//! 候補変更・締切・ギルド境界・確定の原子性を実DBで検証する。
use discalendar_api::{
    error::ApiError,
    models::{
        events::{self, EventInput},
        polls::{self, Confirmation, OptionInput, PollInput},
    },
};
use sqlx::PgPool;

fn input() -> PollInput {
    PollInput {
        title: "日程調整".into(),
        description: Some("説明".into()),
        deadline: None,
        expected_version: None,
        options: vec![
            OptionInput {
                id: None,
                start_at: "2099-10-01T20:00:00".parse().unwrap(),
                end_at: "2099-10-01T21:00:00".parse().unwrap(),
                is_all_day: false,
            },
            OptionInput {
                id: None,
                start_at: "2099-10-02T00:00:00".parse().unwrap(),
                end_at: "2099-10-02T00:00:00".parse().unwrap(),
                is_all_day: true,
            },
        ],
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn votes_edit_deadline_and_atomic_confirmation(pool: PgPool) {
    let mut input = input();
    let id = polls::save(&pool, "111", "333", None, &input)
        .await
        .unwrap();
    let detail = polls::detail(&pool, "111", id, "333").await.unwrap();
    let first = detail.options[0].id;
    let second = detail.options[1].id;
    assert!(matches!(
        polls::vote(&pool, "222", id, first, "333", "yes").await,
        Err(ApiError::NotFound(_))
    ));
    polls::vote(&pool, "111", id, first, "333", "yes")
        .await
        .unwrap();
    polls::vote(&pool, "111", id, first, "333", "maybe")
        .await
        .unwrap();
    polls::vote(&pool, "111", id, second, "444", "no")
        .await
        .unwrap();
    let detail = polls::detail(&pool, "111", id, "333").await.unwrap();
    assert_eq!(detail.votes.len(), 2);
    assert_eq!(detail.options[0].maybe, 1);
    input.expected_version = Some(1);
    input.options[0].id = Some(first);
    input.options[1].id = Some(second);
    input.options[1].start_at = "2099-10-03T00:00:00".parse().unwrap();
    input.options[1].end_at = input.options[1].start_at;
    polls::save(&pool, "111", "333", Some(id), &input)
        .await
        .unwrap();
    let detail = polls::detail(&pool, "111", id, "333").await.unwrap();
    assert_eq!(detail.votes.len(), 1);
    assert_eq!(detail.options[0].id, first);
    assert_ne!(detail.options[1].id, second);
    assert!(
        polls::vote(&pool, "111", id, second, "333", "yes")
            .await
            .is_err()
    );
    assert!(matches!(
        polls::save(&pool, "111", "333", Some(id), &input).await,
        Err(ApiError::Conflict(_))
    ));
    sqlx::query("UPDATE schedule_polls SET deadline='2000-01-01' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        polls::detail(&pool, "111", id, "333")
            .await
            .unwrap()
            .poll
            .status,
        "closed"
    );
    assert!(matches!(
        polls::vote(&pool, "111", id, first, "333", "yes").await,
        Err(ApiError::Conflict(_))
    ));
    let event:EventInput=serde_json::from_value(serde_json::json!({"name":"確定予定","description":"説明","color":"#123456","start_at":input.options[0].start_at,"end_at":input.options[0].end_at,"notifications":[{"num":30,"unit":"minutes"}]})).unwrap();
    let confirmation = Confirmation {
        poll_id: id,
        option_id: first,
        expected_version: 2,
    };
    let mut tx = pool.begin().await.unwrap();
    let row = events::create(&mut *tx, "111", &event, event.start_at, "333")
        .await
        .unwrap();
    polls::confirm(&mut tx, "111", &confirmation, &event, row.id)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        polls::detail(&pool, "111", id, "333")
            .await
            .unwrap()
            .poll
            .status,
        "closed"
    );
    let mut tx = pool.begin().await.unwrap();
    let row = events::create(&mut *tx, "111", &event, event.start_at, "333")
        .await
        .unwrap();
    polls::confirm(&mut tx, "111", &confirmation, &event, row.id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let detail = polls::detail(&pool, "111", id, "333").await.unwrap();
    assert_eq!(detail.poll.confirmed_event_id, Some(row.id));
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        polls::confirm(&mut tx, "111", &confirmation, &event, row.id).await,
        Err(ApiError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let saved: serde_json::Value =
        sqlx::query_scalar("SELECT notifications FROM events WHERE id=$1")
            .bind(row.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(saved, serde_json::json!([{"num":30,"unit":"minutes"}]));
}
#[test]
fn validation_matches_event_limits_and_all_day_convention() {
    let mut input = input();
    assert!(input.validate().is_ok());
    input.options.push(input.options[0].clone());
    assert!(input.validate().is_err());
    input.options.pop();
    input.options[0].is_all_day = true;
    assert!(input.validate().is_err());
}
