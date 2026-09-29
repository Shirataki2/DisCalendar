//! Web/APIとDiscordで同じ締切・候補・上書きルールを使う。呼び出し元でメンバー認証する。
use chrono::NaiveDateTime;
use sqlx::PgPool;

#[derive(Debug, PartialEq)]
pub enum VoteOutcome {
    Saved,
    Closed,
    NotFound,
    InvalidAnswer,
}

pub async fn vote(
    pool: &PgPool,
    guild: &str,
    poll_id: i32,
    option_id: i32,
    user: &str,
    answer: &str,
) -> sqlx::Result<VoteOutcome> {
    if !matches!(answer, "yes" | "maybe" | "no") {
        return Ok(VoteOutcome::InvalidAnswer);
    }
    let mut tx = pool.begin().await?;
    let poll: Option<(String, Option<NaiveDateTime>)> = sqlx::query_as(
        "SELECT status,deadline FROM schedule_polls WHERE guild_id=$1 AND id=$2 FOR UPDATE",
    )
    .bind(guild)
    .bind(poll_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, deadline)) = poll else {
        return Ok(VoteOutcome::NotFound);
    };
    let now = crate::models::now_jst();
    if status != "open" || deadline.is_some_and(|d| d <= now) {
        return Ok(VoteOutcome::Closed);
    }
    let changed=sqlx::query("INSERT INTO schedule_poll_votes (option_id,user_id,answer,updated_at) SELECT id,$3,$4,$5 FROM schedule_poll_options WHERE poll_id=$1 AND id=$2 ON CONFLICT (option_id,user_id) DO UPDATE SET answer=EXCLUDED.answer,updated_at=EXCLUDED.updated_at")
        .bind(poll_id).bind(option_id).bind(user).bind(answer).bind(now).execute(&mut *tx).await?;
    if changed.rows_affected() == 0 {
        return Ok(VoteOutcome::NotFound);
    }
    tx.commit().await?;
    Ok(VoteOutcome::Saved)
}
