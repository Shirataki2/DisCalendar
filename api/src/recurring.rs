//! Webの繰り返し操作。外部サービスへの同期を伴わず、シリーズと各回を同時に確定する。
use crate::{
    error::ApiError,
    models::{
        events::{self, Event, EventInput, EventRow},
        now_jst,
    },
    recurrence::{self, Ending, Rule},
    recurring_events::{self as store, Series},
};
use actix_web::post;
use chrono::{Datelike, Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeScope {
    #[default]
    This,
    Future,
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct DeleteOptions {
    #[serde(default)]
    pub scope: ChangeScope,
    pub expected_series_version: Option<i32>,
}

pub fn validate(input: &EventInput) -> Result<(), ApiError> {
    if let Some(rule) = input.recurrence.as_ref().filter(|r| r.enabled()) {
        rule.rrule(input.start_at).map_err(ApiError::BadRequest)?;
        validate_recurring_fields(input)?;
    }
    Ok(())
}

pub fn validate_recurring_fields(input: &EventInput) -> Result<(), ApiError> {
    if input.is_all_day
        && (input.start_at.time() != chrono::NaiveTime::MIN
            || input.end_at.time() != chrono::NaiveTime::MIN)
    {
        return Err(ApiError::BadRequest(
            "終日の繰り返し予定は開始・終了を0時で指定してください".into(),
        ));
    }
    if input.discord_scheduled_event == Some(true) {
        return Err(ApiError::BadRequest(
            "繰り返し予定はDiscordイベントと連携できません".into(),
        ));
    }
    crate::models::notifications::Notification::validate_list(
        &input.notifications,
        "notifications",
        events::NOTIFICATIONS_MAX,
    )
}

pub async fn decorate(conn: &mut PgConnection, mut event: Event) -> Result<Event, ApiError> {
    event.recurrence = store::info(conn, &event.guild_id, event.id).await?;
    Ok(event)
}

pub async fn decorate_all(
    pool: &sqlx::PgPool,
    rows: Vec<EventRow>,
) -> Result<Vec<Event>, ApiError> {
    let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
    let metadata: Vec<(i32, sqlx::types::Json<store::Info>)> = sqlx::query_as("SELECT e.id,jsonb_build_object('series_id',s.id,'original_start_at',e.original_start_at,'is_exception',EXISTS(SELECT 1 FROM event_series_exceptions x WHERE x.event_id=e.id),'rule',s.recurrence,'version',s.version) FROM events e JOIN event_series s ON s.id=e.series_id AND s.guild_id=e.guild_id WHERE e.id=ANY($1)")
        .bind(&ids).fetch_all(pool).await?;
    let mut metadata: std::collections::HashMap<_, _> = metadata
        .into_iter()
        .map(|(id, info)| (id, info.0))
        .collect();
    Ok(rows
        .into_iter()
        .map(|row| {
            let mut event = Event::from(row);
            event.recurrence = metadata.remove(&event.id);
            event
        })
        .collect())
}

fn template(input: &EventInput) -> Result<serde_json::Value, ApiError> {
    let mut value = serde_json::to_value(input).map_err(anyhow::Error::from)?;
    for key in [
        "recurrence_rule",
        "scope",
        "expected_series_version",
        "discord_scheduled_event",
    ] {
        value.as_object_mut().unwrap().remove(key);
    }
    Ok(value)
}

pub async fn attach_created(
    conn: &mut PgConnection,
    guild: &str,
    event: i32,
    input: &EventInput,
    actor: &str,
) -> Result<(), ApiError> {
    let Some(rule) = input.recurrence.as_ref().filter(|r| r.enabled()) else {
        return Ok(());
    };
    validate(input)?;
    let series = store::create_series(
        conn,
        guild,
        template(input)?,
        rule,
        input.start_at,
        input.end_at,
        actor,
        now_jst(),
    )
    .await?;
    sqlx::query(
        "UPDATE events SET series_id=$1,original_start_at=start_at WHERE guild_id=$2 AND id=$3",
    )
    .bind(series.id)
    .bind(guild)
    .bind(event)
    .execute(&mut *conn)
    .await?;
    let now = now_jst();
    store::fill(
        conn,
        &series,
        now,
        now + Duration::days(recurrence::LOOKAHEAD_DAYS + 1),
    )
    .await?;
    Ok(())
}

fn check_version(series: &Series, expected: Option<i32>) -> Result<(), ApiError> {
    if expected != Some(series.version) {
        return Err(ApiError::Conflict(
            "繰り返し予定が変更されました。再取得してから操作してください".into(),
        ));
    }
    Ok(())
}

fn remaining_count(series: &Series, original: NaiveDateTime, count: u32) -> Result<u32, ApiError> {
    let consumed = recurrence::between(&series.rrule, series.start_at, series.start_at, original)
        .map_err(ApiError::BadRequest)?
        .len() as u32;
    Ok(count.saturating_sub(consumed).max(1))
}

/// Someは繰り返し操作を処理済み、Noneは従来の単発更新へ進む。
pub async fn update(
    conn: &mut PgConnection,
    guild: &str,
    id: i32,
    input: &EventInput,
    actor: &str,
) -> Result<Option<EventRow>, ApiError> {
    let mut input = input.clone();
    if input.notification_mentions.is_none() {
        let mentions: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT notification_mentions FROM events WHERE guild_id=$1 AND id=$2",
        )
        .bind(guild)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
        input.notification_mentions =
            mentions.map(|v| serde_json::from_value(v).unwrap_or_default());
    }
    let input = &input;
    let series = store::lock_for_event(conn, guild, id).await?;
    let Some(series) = series else {
        if input.recurrence.as_ref().is_some_and(Rule::enabled) {
            let old = events::find_by_id_for_update(conn, guild, id)
                .await?
                .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
            if old.discord_scheduled_event_id.is_some() {
                return Err(ApiError::BadRequest(
                    "Discordイベント連携を解除してから繰り返しを設定してください".into(),
                ));
            }
            let row = events::update(&mut *conn, guild, id, input, actor, now_jst()).await?;
            attach_created(conn, guild, id, input, actor).await?;
            return Ok(row);
        }
        return Ok(None);
    };
    validate_recurring_fields(input)?;
    let info = store::info(conn, guild, id)
        .await?
        .ok_or_else(|| ApiError::Conflict("予定が変更されました".into()))?;
    if input.scope == ChangeScope::This {
        if input
            .recurrence
            .as_ref()
            .is_some_and(|r| serde_json::to_value(r).ok().as_ref() != Some(&series.recurrence))
        {
            return Err(ApiError::BadRequest(
                "繰り返し条件の変更は「この回以降」を選択してください".into(),
            ));
        }
        let row = events::update(&mut *conn, guild, id, input, actor, now_jst()).await?;
        store::record_exception(conn, guild, id, false).await?;
        return Ok(row);
    }
    check_version(&series, input.expected_series_version)?;
    let mut rule: Rule = input
        .recurrence
        .clone()
        .unwrap_or(serde_json::from_value(series.recurrence.clone()).map_err(anyhow::Error::from)?);
    // フォームで条件を変えなかった場合、既に消費した開催枠を回数から引く。
    if (input.recurrence.is_none()
        || input
            .recurrence
            .as_ref()
            .is_some_and(|r| serde_json::to_value(r).ok().as_ref() == Some(&series.recurrence)))
        && let Some(Ending::Count { count }) = rule.ending_mut()
    {
        *count = remaining_count(&series, info.original_start_at, *count)?;
    }
    if input.recurrence.is_none() && input.start_at.date() != info.original_start_at.date() {
        let shift = (input.start_at.date() - info.original_start_at.date()).num_days();
        match &mut rule {
            Rule::Weekly { weekdays, .. } | Rule::Biweekly { weekdays, .. } => {
                for day in weekdays {
                    *day = (i64::from(*day) + shift).rem_euclid(7) as u8;
                }
            }
            Rule::MonthlyDate { day, .. } => *day = input.start_at.day() as u8,
            Rule::MonthlyWeekday { nth, weekday, .. } => {
                *nth = ((input.start_at.day() - 1) / 7 + 1) as u8;
                *weekday = input.start_at.weekday().num_days_from_monday() as u8;
            }
            _ => (),
        }
    }
    if rule.enabled() {
        rule.rrule(input.start_at).map_err(ApiError::BadRequest)?;
    }
    sqlx::query("UPDATE event_series SET end_before=$2,version=version+1,updated_at=$3,updated_by=$4 WHERE id=$1")
        .bind(series.id).bind(info.original_start_at).bind(now_jst()).bind(actor).execute(&mut *conn).await?;
    let next = if rule.enabled() {
        let mut next = store::create_series(
            conn,
            guild,
            template(input)?,
            &rule,
            input.start_at,
            input.end_at,
            &series.created_by,
            series.created_at,
        )
        .await?;
        next.updated_at = Some(now_jst());
        next.updated_by = Some(actor.to_owned());
        sqlx::query("UPDATE event_series SET updated_at=$2,updated_by=$3 WHERE id=$1")
            .bind(next.id)
            .bind(next.updated_at)
            .bind(&next.updated_by)
            .execute(&mut *conn)
            .await?;
        Some(next)
    } else {
        None
    };
    let rows: Vec<(i32,NaiveDateTime,bool,bool)> = sqlx::query_as("SELECT e.id,e.original_start_at,EXISTS(SELECT 1 FROM event_series_exceptions x WHERE x.event_id=e.id),EXISTS(SELECT 1 FROM event_attachments a WHERE a.event_id=e.id) FROM events e WHERE e.series_id=$1 AND e.original_start_at >= $2 ORDER BY e.original_start_at FOR UPDATE")
        .bind(series.id).bind(info.original_start_at).fetch_all(&mut *conn).await?;
    let cancelled: Vec<NaiveDateTime> = sqlx::query_scalar("SELECT original_start_at FROM event_series_exceptions WHERE series_id=$1 AND original_start_at >= $2 AND event_id IS NULL")
        .bind(series.id).bind(info.original_start_at).fetch_all(&mut *conn).await?;
    let mut used_starts = std::collections::HashSet::new();
    for (event, original, exception, attachments) in rows {
        // 対象回自身にはフォームの変更を反映。将来の例外は日付で新しい開催枠と照合する。
        let new_start = if event == id {
            Some(input.start_at)
        } else if let Some(next) = &next {
            let day = original.date().and_hms_opt(0, 0, 0).unwrap();
            recurrence::between(&next.rrule, next.start_at, day, day + Duration::days(1))
                .map_err(ApiError::BadRequest)?
                .into_iter()
                .next()
        } else {
            None
        };
        let new_start = new_start.filter(|start| used_starts.insert(*start));
        sqlx::query("DELETE FROM event_series_exceptions WHERE event_id=$1")
            .bind(event)
            .execute(&mut *conn)
            .await?;
        if let (Some(next), Some(start)) = (&next, new_start) {
            sqlx::query("UPDATE events SET series_id=$1,original_start_at=$2 WHERE id=$3")
                .bind(next.id)
                .bind(start)
                .bind(event)
                .execute(&mut *conn)
                .await?;
            if exception && event != id {
                store::record_exception(conn, guild, event, false).await?;
            } else {
                let mut changed = input.clone();
                changed.start_at = start;
                changed.end_at = start + (input.end_at - input.start_at);
                events::update(&mut *conn, guild, event, &changed, actor, now_jst()).await?;
            }
        } else if event == id || exception || attachments {
            sqlx::query("UPDATE events SET series_id=NULL,original_start_at=NULL WHERE id=$1")
                .bind(event)
                .execute(&mut *conn)
                .await?;
            if event == id {
                events::update(&mut *conn, guild, event, input, actor, now_jst()).await?;
            }
        } else {
            events::delete(&mut *conn, guild, event).await?;
        }
    }
    if let Some(next) = next {
        for original in cancelled {
            let day = original.date().and_hms_opt(0, 0, 0).unwrap();
            if let Some(start) =
                recurrence::between(&next.rrule, next.start_at, day, day + Duration::days(1))
                    .map_err(ApiError::BadRequest)?
                    .into_iter()
                    .next()
            {
                if used_starts.contains(&start) {
                    return Err(ApiError::BadRequest(
                        "移動先は中止済みの開催日です。別の開始日を指定してください".into(),
                    ));
                }
                sqlx::query("INSERT INTO event_series_exceptions(series_id,original_start_at) VALUES ($1,$2) ON CONFLICT DO NOTHING").bind(next.id).bind(start).execute(&mut *conn).await?;
            }
        }
        store::fill(
            conn,
            &next,
            now_jst(),
            now_jst() + Duration::days(recurrence::LOOKAHEAD_DAYS + 1),
        )
        .await?;
    }
    events::find_by_id_for_update(conn, guild, id)
        .await
        .map_err(Into::into)
}

pub async fn before_delete(
    conn: &mut PgConnection,
    guild: &str,
    id: i32,
    options: &DeleteOptions,
) -> Result<(), ApiError> {
    let Some(series) = store::lock_for_event(conn, guild, id).await? else {
        return Ok(());
    };
    if options.scope == ChangeScope::This {
        store::record_exception(conn, guild, id, true).await?;
        return Ok(());
    }
    check_version(&series, options.expected_series_version)?;
    let info = store::info(conn, guild, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    sqlx::query("UPDATE event_series SET end_before=$2,version=version+1 WHERE id=$1")
        .bind(series.id)
        .bind(info.original_start_at)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM events WHERE series_id=$1 AND original_start_at >= $2 AND id<>$3")
        .bind(series.id)
        .bind(info.original_start_at)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PreviewInput {
    /// この回以降の編集時のみ指定。変更していない回数条件から消費済み枠を引く。
    pub event_id: Option<i32>,
    pub start_at: NaiveDateTime,
    #[schema(value_type=Object)]
    pub recurrence: Rule,
}

pub async fn preview_dates(
    pool: &sqlx::PgPool,
    guild: &str,
    input: &PreviewInput,
) -> Result<Vec<NaiveDateTime>, ApiError> {
    let mut rule = input.recurrence.clone();
    if let Some(id) = input.event_id {
        let current: Option<(NaiveDateTime, sqlx::types::Json<serde_json::Value>)> = sqlx::query_as("SELECT e.original_start_at,to_jsonb(s) FROM events e JOIN event_series s ON s.id=e.series_id WHERE e.guild_id=$1 AND e.id=$2")
            .bind(guild).bind(id).fetch_optional(pool).await?;
        if let Some((original, value)) = current {
            let series: Series = serde_json::from_value(value.0).map_err(anyhow::Error::from)?;
            if serde_json::to_value(&rule).map_err(anyhow::Error::from)? == series.recurrence
                && let Some(Ending::Count { count }) = rule.ending_mut()
            {
                *count = remaining_count(&series, original, *count)?;
            }
        } else if events::find_by_id(pool, guild, id).await?.is_none() {
            return Err(ApiError::NotFound("event not found".into()));
        }
    }
    recurrence::preview(&rule, input.start_at).map_err(ApiError::BadRequest)
}

#[utoipa::path(tag="events",request_body=PreviewInput,responses((status=200,body=Vec<NaiveDateTime>)))]
#[post("/{guild_id}/recurrence/preview")]
pub async fn preview(
    member: crate::routes::GuildMember,
    input: actix_web::web::Json<PreviewInput>,
    state: actix_web::web::Data<crate::state::AppState>,
) -> Result<actix_web::web::Json<Vec<NaiveDateTime>>, ApiError> {
    crate::routes::events::ensure_can_edit(&state.pool, &member).await?;
    Ok(actix_web::web::Json(
        preview_dates(&state.pool, member.guild_id(), &input).await?,
    ))
}

#[utoipa::path(tag="admin",request_body=PreviewInput,responses((status=200,body=Vec<NaiveDateTime>)))]
#[post("/guilds/{guild_id}/events/recurrence/preview")]
pub async fn admin_preview(
    _admin: crate::admin::AdminUser,
    path: actix_web::web::Path<String>,
    state: actix_web::web::Data<crate::state::AppState>,
    input: actix_web::web::Json<PreviewInput>,
) -> Result<actix_web::web::Json<Vec<NaiveDateTime>>, ApiError> {
    Ok(actix_web::web::Json(
        preview_dates(&state.pool, &path, &input).await?,
    ))
}
