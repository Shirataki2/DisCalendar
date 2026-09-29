//! 日程調整。投票・締切・確定は親の行ロックで直列化する。
use chrono::{NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection, PgPool};
use utoipa::ToSchema;

use super::{events::EventInput, now_jst};
use crate::error::ApiError;

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Poll {
    pub id: i32,
    pub guild_id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_by: String,
    pub deadline: Option<NaiveDateTime>,
    pub status: String,
    pub confirmed_event_id: Option<i32>,
    pub version: i32,
    pub created_at: NaiveDateTime,
}
impl Poll {
    pub fn effective_status(&self) -> &str {
        if self.status == "open" && self.deadline.is_some_and(|d| d <= now_jst()) {
            "closed"
        } else {
            &self.status
        }
    }
    pub fn check_version(&self, version: i32) -> Result<(), ApiError> {
        if self.version != version || self.status == "confirmed" {
            return Err(ApiError::Conflict(
                "日程調整が更新済み、または確定済みです。読み直してください".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct PollOption {
    pub id: i32,
    pub poll_id: i32,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub is_all_day: bool,
    pub position: i32,
    pub yes: i64,
    pub maybe: i64,
    pub no: i64,
}
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Vote {
    pub option_id: i32,
    pub user_id: String,
    pub answer: String,
}
#[derive(Serialize, ToSchema)]
pub struct PollDetail {
    #[serde(flatten)]
    pub poll: Poll,
    pub options: Vec<PollOption>,
    pub votes: Vec<Vote>,
    pub current_user_id: String,
}
#[derive(Clone, Deserialize, ToSchema)]
pub struct OptionInput {
    pub id: Option<i32>,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub is_all_day: bool,
}
#[derive(Deserialize, ToSchema)]
pub struct PollInput {
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<NaiveDateTime>,
    pub options: Vec<OptionInput>,
    pub expected_version: Option<i32>,
}
impl PollInput {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.title.trim().is_empty()
            || self.title.chars().count() > 32
            || self
                .description
                .as_ref()
                .is_some_and(|s| s.chars().count() > 1000)
        {
            return Err(ApiError::BadRequest(
                "タイトルは1〜32文字、説明は1000文字以内で入力してください".into(),
            ));
        }
        if !(1..=5).contains(&self.options.len()) {
            return Err(ApiError::BadRequest("候補は1〜5件にしてください".into()));
        }
        let mut ids = std::collections::HashSet::new();
        let mut dates = std::collections::HashSet::new();
        for o in &self.options {
            if o.end_at < o.start_at
                || (!o.is_all_day && o.end_at == o.start_at)
                || (o.is_all_day
                    && (o.start_at.time() != NaiveTime::MIN || o.end_at.time() != NaiveTime::MIN))
                || !dates.insert((o.start_at, o.end_at, o.is_all_day))
                || o.id.is_some_and(|id| !ids.insert(id))
            {
                return Err(ApiError::BadRequest(
                    "候補の日時・重複を確認してください。終日は開始・終了日の0時を指定します"
                        .into(),
                ));
            }
        }
        if self.deadline.is_some_and(|d| d <= now_jst()) {
            return Err(ApiError::BadRequest(
                "締切は未来の日時にしてください".into(),
            ));
        }
        Ok(())
    }
}

pub async fn list(pool: &PgPool, guild: &str) -> Result<Vec<Poll>, ApiError> {
    let mut rows = sqlx::query_as::<_, Poll>(
        "SELECT * FROM schedule_polls WHERE guild_id=$1 ORDER BY id DESC LIMIT 100",
    )
    .bind(guild)
    .fetch_all(pool)
    .await?;
    for row in &mut rows {
        row.status = row.effective_status().to_owned();
    }
    Ok(rows)
}
pub async fn lock(conn: &mut PgConnection, guild: &str, id: i32) -> Result<Poll, ApiError> {
    sqlx::query_as("SELECT * FROM schedule_polls WHERE guild_id=$1 AND id=$2 FOR UPDATE")
        .bind(guild)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| ApiError::NotFound("日程調整が見つかりません".into()))
}
pub async fn detail(
    pool: &PgPool,
    guild: &str,
    id: i32,
    user: &str,
) -> Result<PollDetail, ApiError> {
    let mut tx = pool.begin().await?;
    // 候補を編集している途中の状態を返さない。
    let mut poll: Poll =
        sqlx::query_as("SELECT * FROM schedule_polls WHERE guild_id=$1 AND id=$2 FOR SHARE")
            .bind(guild)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::NotFound("日程調整が見つかりません".into()))?;
    poll.status = poll.effective_status().to_owned();
    let options = sqlx::query_as("SELECT o.*, count(*) FILTER (WHERE v.answer='yes') AS yes, count(*) FILTER (WHERE v.answer='maybe') AS maybe, count(*) FILTER (WHERE v.answer='no') AS no FROM schedule_poll_options o LEFT JOIN schedule_poll_votes v ON v.option_id=o.id WHERE o.poll_id=$1 GROUP BY o.id ORDER BY o.position")
        .bind(id).fetch_all(&mut *tx).await?;
    let votes = sqlx::query_as("SELECT v.option_id, v.user_id, v.answer FROM schedule_poll_votes v JOIN schedule_poll_options o ON o.id=v.option_id WHERE o.poll_id=$1 ORDER BY v.user_id, o.position")
        .bind(id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(PollDetail {
        poll,
        options,
        votes,
        current_user_id: user.into(),
    })
}
pub async fn save(
    pool: &PgPool,
    guild: &str,
    user: &str,
    id: Option<i32>,
    input: &PollInput,
) -> Result<i32, ApiError> {
    input.validate()?;
    let mut tx = pool.begin().await?;
    let id = if let Some(id) = id {
        let poll = lock(&mut tx, guild, id).await?;
        poll.check_version(
            input
                .expected_version
                .ok_or_else(|| ApiError::BadRequest("expected_version is required".into()))?,
        )?;
        if poll.effective_status() != "open" {
            return Err(ApiError::Conflict(
                "締切済みの日程調整は編集できません".into(),
            ));
        }
        sqlx::query("UPDATE schedule_polls SET title=$2, description=$3, deadline=$4, version=version+1 WHERE id=$1")
            .bind(id).bind(&input.title).bind(&input.description).bind(input.deadline).execute(&mut *tx).await?;
        id
    } else {
        if input.options.iter().any(|o| o.id.is_some()) {
            return Err(ApiError::BadRequest("新規候補にIDは指定できません".into()));
        }
        sqlx::query_scalar("INSERT INTO schedule_polls (guild_id,title,description,created_by,deadline,created_at) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id")
            .bind(guild).bind(&input.title).bind(&input.description).bind(user).bind(input.deadline).bind(now_jst()).fetch_one(&mut *tx).await?
    };
    // IDを保って未変更候補の回答を残す。変更した候補だけ回答をリセットする。
    let old: Vec<(i32, NaiveDateTime, NaiveDateTime, bool)> = sqlx::query_as(
        "SELECT id,start_at,end_at,is_all_day FROM schedule_poll_options WHERE poll_id=$1",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    for option in &input.options {
        if let Some(oid) = option.id
            && !old.iter().any(|o| o.0 == oid)
        {
            return Err(ApiError::BadRequest(
                "別の日程調整の候補は指定できません".into(),
            ));
        }
    }
    let retained: Vec<i32> = input
        .options
        .iter()
        .filter_map(|o| {
            o.id.filter(|id| old.contains(&(*id, o.start_at, o.end_at, o.is_all_day)))
        })
        .collect();
    sqlx::query("DELETE FROM schedule_poll_options WHERE poll_id=$1 AND NOT (id=ANY($2))")
        .bind(id)
        .bind(&retained)
        .execute(&mut *tx)
        .await?;
    for (position, option) in input.options.iter().enumerate() {
        if let Some(oid) = option.id.filter(|id| retained.contains(id)) {
            sqlx::query("UPDATE schedule_poll_options SET position=$2 WHERE id=$1")
                .bind(oid)
                .bind(position as i32)
                .execute(&mut *tx)
                .await?;
        } else {
            // 日時変更時は新しいIDにし、古い画面やDiscordボタンからの投票を拒否する。
            sqlx::query("INSERT INTO schedule_poll_options (poll_id,start_at,end_at,is_all_day,position) VALUES ($1,$2,$3,$4,$5)")
                .bind(id).bind(option.start_at).bind(option.end_at).bind(option.is_all_day).bind(position as i32).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(id)
}
pub async fn vote(
    pool: &PgPool,
    guild: &str,
    id: i32,
    option: i32,
    user: &str,
    answer: &str,
) -> Result<(), ApiError> {
    if !matches!(answer, "yes" | "maybe" | "no") {
        return Err(ApiError::BadRequest(
            "回答はyes / maybe / noで指定してください".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let poll = lock(&mut tx, guild, id).await?;
    if poll.effective_status() != "open" {
        return Err(ApiError::Conflict("投票は締め切られています".into()));
    }
    let changed = sqlx::query("INSERT INTO schedule_poll_votes (option_id,user_id,answer,updated_at) SELECT id,$3,$4,$5 FROM schedule_poll_options WHERE poll_id=$1 AND id=$2 ON CONFLICT (option_id,user_id) DO UPDATE SET answer=EXCLUDED.answer,updated_at=EXCLUDED.updated_at")
        .bind(id).bind(option).bind(user).bind(answer).bind(now_jst()).execute(&mut *tx).await?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::NotFound("候補が見つかりません".into()));
    }
    tx.commit().await?;
    Ok(())
}
#[derive(Clone, Deserialize, ToSchema)]
pub struct Confirmation {
    pub poll_id: i32,
    pub option_id: i32,
    pub expected_version: i32,
}
pub async fn validate_confirmation(
    conn: &mut PgConnection,
    guild: &str,
    confirmation: &Confirmation,
    event: &EventInput,
) -> Result<(), ApiError> {
    let poll = lock(conn, guild, confirmation.poll_id).await?;
    poll.check_version(confirmation.expected_version)?;
    let option: Option<(NaiveDateTime, NaiveDateTime, bool)> = sqlx::query_as(
        "SELECT start_at,end_at,is_all_day FROM schedule_poll_options WHERE poll_id=$1 AND id=$2",
    )
    .bind(poll.id)
    .bind(confirmation.option_id)
    .fetch_optional(conn)
    .await?;
    if option != Some((event.start_at, event.end_at, event.is_all_day))
        || event
            .recurrence
            .as_ref()
            .is_some_and(|r| !matches!(r, crate::recurrence::Rule::None))
    {
        return Err(ApiError::BadRequest(
            "確定する日時は選択した候補と揃え、繰り返しなしにしてください".into(),
        ));
    }
    Ok(())
}
pub async fn confirm(
    conn: &mut PgConnection,
    guild: &str,
    confirmation: &Confirmation,
    event: &EventInput,
    event_id: i32,
) -> Result<(), ApiError> {
    validate_confirmation(conn, guild, confirmation, event).await?;
    sqlx::query("UPDATE schedule_polls SET status='confirmed',confirmed_event_id=$2,version=version+1 WHERE id=$1")
        .bind(confirmation.poll_id).bind(event_id).execute(conn).await?;
    Ok(())
}
