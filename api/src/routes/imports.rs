use std::collections::HashSet;

use actix_web::{HttpResponse, post, web};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use utoipa::ToSchema;

use super::{GuildMember, events::ensure_can_edit};
use crate::{
    error::{ApiError, ErrorBody},
    ical_import::{
        self, IMPORT_MAX_EVENTS, IMPORT_MAX_OCCURRENCES, ImportEventInput, SkippedImport,
    },
    models::{
        event_links,
        events::{self, Event},
        now_jst,
    },
    state::AppState,
};

/// `/events` スコープの raw body 上限。ICS 本文を JSON で包む分の余裕を含む。
pub const PAYLOAD_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportPreviewRequest {
    pub ics: String,
    /// 絞り込み開始日 (JST、両端を含む)
    pub start_date: Option<NaiveDate>,
    /// 絞り込み終了日 (JST、両端を含む)
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportPreviewItem {
    pub source_index: usize,
    pub event: ImportEventInput,
    pub truncated_fields: Vec<String>,
    pub estimated_occurrences: usize,
    pub duplicate: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportPreviewResponse {
    pub items: Vec<ImportPreviewItem>,
    pub total_count: usize,
    pub matched_count: usize,
    pub available_start_date: Option<NaiveDate>,
    pub available_end_date: Option<NaiveDate>,
    pub skipped: Vec<SkippedImport>,
    pub over_event_limit: bool,
    pub estimated_occurrences: usize,
    pub over_occurrence_limit: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkImportRequest {
    pub events: Vec<ImportEventInput>,
}

fn parse_json<T: DeserializeOwned>(body: &web::Bytes) -> Result<T, ApiError> {
    serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("JSON を読み取れません: {error}")))
}

fn validate_range(start: Option<NaiveDate>, end: Option<NaiveDate>) -> Result<(), ApiError> {
    if start.zip(end).is_some_and(|(start, end)| start > end) {
        return Err(ApiError::BadRequest(
            "start_date must not be after end_date".into(),
        ));
    }
    Ok(())
}

/// ICS 本文を解析し、保存する予定をプレビューする。
#[utoipa::path(
    tag = "events",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = ImportPreviewRequest,
    responses(
        (status = 200, body = ImportPreviewResponse),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[post("/{guild_id}/import/preview")]
pub async fn preview(
    member: GuildMember,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<web::Json<ImportPreviewResponse>, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let request: ImportPreviewRequest = parse_json(&body)?;
    validate_range(request.start_date, request.end_date)?;
    let parsed = web::block(move || ical_import::parse(&request.ics))
        .await
        .map_err(|error| ApiError::Internal(anyhow::anyhow!(error)))?
        .map_err(ApiError::BadRequest)?;
    let available_start_date = parsed
        .events
        .iter()
        .map(|item| item.event.start_at.date())
        .min();
    let available_end_date = parsed
        .events
        .iter()
        .map(|item| item.event.end_at.date())
        .max();
    let mut matched = Vec::new();
    for item in parsed.events {
        let include = match (request.start_date, request.end_date) {
            (None, None) => Ok(true),
            (Some(start), None) => ical_import::overlaps(&item.event, start, NaiveDate::MAX),
            (None, Some(end)) => ical_import::overlaps(&item.event, NaiveDate::MIN, end),
            (Some(start), Some(end)) => ical_import::overlaps(&item.event, start, end),
        }
        .map_err(ApiError::BadRequest)?;
        if include {
            matched.push(item);
        }
    }
    matched.sort_by_key(|item| (item.event.start_at, item.source_index));
    let matched_count = matched.len();
    let estimated_occurrences = matched
        .iter()
        .take(IMPORT_MAX_EVENTS)
        .map(|item| item.estimated_occurrences)
        .sum();
    matched.truncate(IMPORT_MAX_EVENTS);

    let names: Vec<_> = matched.iter().map(|item| item.event.name.clone()).collect();
    let starts: Vec<_> = matched.iter().map(|item| item.event.start_at).collect();
    let duplicates: HashSet<(String, NaiveDateTime)> = if matched.is_empty() {
        HashSet::new()
    } else {
        sqlx::query_as::<_, (String, NaiveDateTime)>(
            "SELECT DISTINCT e.name,e.start_at FROM events e JOIN UNNEST($2::text[],$3::timestamp[]) AS input(name,start_at) ON input.name=e.name AND input.start_at=e.start_at WHERE e.guild_id=$1",
        )
        .bind(member.guild_id())
        .bind(&names)
        .bind(&starts)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect()
    };
    let items = matched
        .into_iter()
        .map(|item| ImportPreviewItem {
            duplicate: duplicates.contains(&(item.event.name.clone(), item.event.start_at)),
            source_index: item.source_index,
            event: item.event,
            truncated_fields: item.truncated_fields,
            estimated_occurrences: item.estimated_occurrences,
        })
        .collect();
    Ok(web::Json(ImportPreviewResponse {
        items,
        total_count: parsed.total_count,
        matched_count,
        available_start_date,
        available_end_date,
        skipped: parsed.skipped,
        over_event_limit: matched_count > IMPORT_MAX_EVENTS,
        estimated_occurrences,
        over_occurrence_limit: estimated_occurrences > IMPORT_MAX_OCCURRENCES,
    }))
}

/// プレビューで選んだ予定を1トランザクションで作成する。
#[utoipa::path(
    tag = "events",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = BulkImportRequest,
    responses(
        (status = 201, body = Vec<Event>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[post("/{guild_id}/bulk")]
pub async fn bulk(
    member: GuildMember,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _writer = event_links::lock_writer(&state.pool, member.guild_id()).await?;
    ensure_can_edit(&state.pool, &member).await?;
    let request: BulkImportRequest = parse_json(&body)?;
    if request.events.is_empty() || request.events.len() > IMPORT_MAX_EVENTS {
        return Err(ApiError::BadRequest(format!(
            "events must contain between 1 and {IMPORT_MAX_EVENTS} items"
        )));
    }
    let mut inputs = Vec::with_capacity(request.events.len());
    let mut occurrences = 0usize;
    for (index, event) in request.events.iter().enumerate() {
        let input = event.to_event_input();
        input
            .validate()
            .map_err(|error| ApiError::BadRequest(format!("events[{}]: {error}", index + 1)))?;
        occurrences =
            occurrences
                .checked_add(ical_import::estimate_occurrences(event).map_err(|error| {
                    ApiError::BadRequest(format!("events[{}]: {error}", index + 1))
                })?)
                .ok_or_else(|| ApiError::BadRequest("予定数が多すぎます".into()))?;
        inputs.push(input);
    }
    if occurrences > IMPORT_MAX_OCCURRENCES {
        return Err(ApiError::BadRequest(format!(
            "生成される予定は合計 {IMPORT_MAX_OCCURRENCES} 件以下にしてください"
        )));
    }

    let guild_id = member.guild_id();
    let actor = &member.user.discord_user_id;
    let created_at = now_jst();
    let mut tx = state.pool.begin().await?;
    let mut created = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let row = events::create(&mut *tx, guild_id, input, created_at, actor).await?;
        crate::recurring::attach_created(&mut tx, guild_id, row.id, input, actor).await?;
        let event = crate::recurring::decorate(&mut tx, Event::from(row)).await?;
        crate::webhook_outbox::enqueue(&mut tx, guild_id, event.id, "event.created", actor).await?;
        created.push(event);
    }
    tx.commit().await?;
    tracing::info!(guild_id, count = created.len(), user_id = %actor, "events imported from ICS");
    Ok(HttpResponse::Created().json(created))
}
