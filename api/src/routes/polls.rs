//! ログイン済みのギルドメンバーだけが日程調整を閲覧・投票できる。
use super::{
    GuildMember,
    events::{create_for_member, ensure_can_edit},
};
use crate::{
    error::{ApiError, ErrorBody},
    models::{
        event_links,
        events::{Event, EventInput},
        guilds,
        polls::{self, Confirmation, Poll, PollDetail, PollInput},
    },
    state::AppState,
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, IntoParams)]
pub struct PollPath {
    #[allow(dead_code)]
    pub guild_id: String,
    pub poll_id: i32,
}
#[derive(Deserialize, ToSchema)]
pub struct VersionInput {
    pub expected_version: i32,
}
#[derive(Deserialize, ToSchema)]
pub struct VoteInput {
    pub option_id: i32,
    pub answer: String,
}
#[derive(Deserialize, ToSchema)]
pub struct ConfirmInput {
    pub option_id: i32,
    pub expected_version: i32,
    pub event: EventInput,
}
#[derive(serde::Serialize, ToSchema)]
pub struct PollResult {
    pub poll: PollDetail,
    /// 案内投稿に失敗しても保存結果は成功として返す。
    pub announcement: String,
}
#[derive(serde::Serialize, ToSchema)]
pub struct ConfirmResult {
    pub event: Event,
    pub announcement: String,
}

#[utoipa::path(tag="polls", params(("guild_id"=String,Path)), responses((status=200,body=Vec<Poll>),(status=403,body=ErrorBody)))]
#[get("/{guild_id}")]
pub async fn list(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<Poll>>, ApiError> {
    Ok(web::Json(
        polls::list(&state.pool, member.guild_id()).await?,
    ))
}
#[utoipa::path(tag="polls", params(PollPath), responses((status=200,body=PollDetail),(status=404,body=ErrorBody)))]
#[get("/{guild_id}/{poll_id}")]
pub async fn detail(
    member: GuildMember,
    path: web::Path<PollPath>,
    state: web::Data<AppState>,
) -> Result<web::Json<PollDetail>, ApiError> {
    Ok(web::Json(
        polls::detail(
            &state.pool,
            member.guild_id(),
            path.poll_id,
            &member.user.discord_user_id,
        )
        .await?,
    ))
}
#[utoipa::path(tag="polls", params(("guild_id"=String,Path)),request_body=PollInput,responses((status=201,body=PollResult)))]
#[post("/{guild_id}")]
pub async fn create(
    member: GuildMember,
    body: web::Json<PollInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let poll = polls::save(
        &state.pool,
        member.guild_id(),
        &member.user.discord_user_id,
        None,
        &body,
    )
    .await?;
    let announcement = announce(&state, &poll, false).await;
    Ok(HttpResponse::Created().json(PollResult { poll, announcement }))
}
#[utoipa::path(tag="polls",params(PollPath),request_body=PollInput,responses((status=200,body=PollDetail)))]
#[put("/{guild_id}/{poll_id}")]
pub async fn update(
    member: GuildMember,
    path: web::Path<PollPath>,
    body: web::Json<PollInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<PollDetail>, ApiError> {
    let _writer = event_links::lock_writer(&state.pool, member.guild_id()).await?;
    ensure_can_edit(&state.pool, &member).await?;
    let poll = polls::save(
        &state.pool,
        member.guild_id(),
        &member.user.discord_user_id,
        Some(path.poll_id),
        &body,
    )
    .await?;
    Ok(web::Json(poll))
}
#[utoipa::path(tag="polls",params(PollPath),request_body=VoteInput,responses((status=204),(status=409,body=ErrorBody)))]
#[put("/{guild_id}/{poll_id}/vote")]
pub async fn vote(
    member: GuildMember,
    path: web::Path<PollPath>,
    body: web::Json<VoteInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    polls::vote(
        &state.pool,
        member.guild_id(),
        path.poll_id,
        body.option_id,
        &member.user.discord_user_id,
        &body.answer,
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}
#[utoipa::path(tag="polls",params(PollPath),request_body=VersionInput,responses((status=204)))]
#[post("/{guild_id}/{poll_id}/close")]
pub async fn close(
    member: GuildMember,
    path: web::Path<PollPath>,
    body: web::Json<VersionInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _writer = event_links::lock_writer(&state.pool, member.guild_id()).await?;
    ensure_can_edit(&state.pool, &member).await?;
    let mut tx = state.pool.begin().await?;
    polls::lock(&mut tx, member.guild_id(), path.poll_id)
        .await?
        .check_version(body.expected_version)?;
    sqlx::query("UPDATE schedule_polls SET status='closed',version=version+1 WHERE id=$1")
        .bind(path.poll_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}
#[utoipa::path(tag="polls",params(PollPath),responses((status=204)))]
#[delete("/{guild_id}/{poll_id}")]
pub async fn remove(
    member: GuildMember,
    path: web::Path<PollPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _writer = event_links::lock_writer(&state.pool, member.guild_id()).await?;
    ensure_can_edit(&state.pool, &member).await?;
    let mut tx = state.pool.begin().await?;
    polls::lock(&mut tx, member.guild_id(), path.poll_id).await?;
    sqlx::query("DELETE FROM schedule_polls WHERE id=$1")
        .bind(path.poll_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}
#[utoipa::path(tag="polls",params(PollPath),request_body=ConfirmInput,responses((status=201,body=ConfirmResult),(status=409,body=ErrorBody)))]
#[post("/{guild_id}/{poll_id}/confirm")]
pub async fn confirm(
    member: GuildMember,
    path: web::Path<PollPath>,
    body: web::Json<ConfirmInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let confirmation = Confirmation {
        poll_id: path.poll_id,
        option_id: body.option_id,
        expected_version: body.expected_version,
    };
    let event = create_for_member(&member, &body.event, &state, Some(&confirmation)).await?;
    // 保存後の案内失敗で再確定させない。
    let announcement = match polls::detail(
        &state.pool,
        member.guild_id(),
        path.poll_id,
        &member.user.discord_user_id,
    )
    .await
    {
        Ok(poll) => announce(&state, &poll, true).await,
        Err(err) => {
            tracing::warn!(error=%err,"poll saved but announcement unavailable");
            "failed".into()
        }
    };
    Ok(HttpResponse::Created().json(ConfirmResult {
        event,
        announcement,
    }))
}
async fn announce(state: &AppState, poll: &PollDetail, confirmed: bool) -> String {
    let result: Result<&str, ApiError> = async {
        let config = guilds::get_config(&state.pool, &poll.poll.guild_id).await?;
        let Some(channel) = config.notification_channel_id else {
            return Ok("not_configured");
        };
        let locale = crate::i18n::Locale::resolve(&config.locale);
        let label = if confirmed {
            locale.text("日程が確定しました", "Schedule confirmed")
        } else {
            locale.text(
                "日程調整への回答をお願いします",
                "Please respond to the scheduling poll",
            )
        };
        let deadline = poll
            .poll
            .deadline
            .map(|d| locale.datetime(d))
            .unwrap_or_else(|| locale.text("なし", "None").into());
        let url = format!(
            "{}/dashboard/{}/polls/{}",
            state.site_base_url, poll.poll.guild_id, poll.poll.id
        );
        state
            .discord
            .post_poll_announcement(
                &channel,
                &announcement(locale, poll, label, &deadline, &url),
            )
            .await?;
        Ok("sent")
    }
    .await;
    match result {
        Ok(status) => status.into(),
        Err(err) => {
            tracing::warn!(poll_id=poll.poll.id,error=%err,"poll announcement failed");
            "failed".into()
        }
    }
}

fn announcement(
    locale: crate::i18n::Locale,
    poll: &PollDetail,
    label: &str,
    deadline: &str,
    url: &str,
) -> serde_json::Value {
    let description = if locale == crate::i18n::Locale::En {
        format!(
            "{} {} · Deadline: {deadline}",
            locale.count(poll.options.len()),
            if poll.options.len() == 1 {
                "option"
            } else {
                "options"
            }
        )
    } else {
        format!("候補 {} 件・締切 {deadline}", poll.options.len())
    };
    serde_json::json!({ "allowed_mentions": { "parse": [] }, "embeds": [{ "title": format!("{label}: {}", poll.poll.title), "url": url, "description": description, "color": 5793266 }] })
}

#[cfg(test)]
mod locale_tests {
    use super::*;
    use crate::i18n::Locale;
    #[sqlx::test(migrations = "./migrations")]
    async fn announcement_preserves_title_and_localizes_count_and_deadline(pool: sqlx::PgPool) {
        let start = "2099-12-31T21:00:00".parse().unwrap();
        let end = "2099-12-31T22:00:00".parse().unwrap();
        let input = PollInput {
            title: "日本語の予定".into(),
            description: None,
            deadline: Some(start),
            expected_version: None,
            options: vec![polls::OptionInput {
                id: None,
                start_at: start,
                end_at: end,
                is_all_day: false,
            }],
        };
        let poll = polls::save(&pool, "111", "333", None, &input)
            .await
            .unwrap();
        let english = announcement(
            Locale::En,
            &poll,
            "Please respond to the scheduling poll",
            &Locale::En.datetime(start),
            "https://example.com/poll",
        );
        assert_eq!(
            english["embeds"][0]["title"],
            "Please respond to the scheduling poll: 日本語の予定"
        );
        assert_eq!(
            english["embeds"][0]["description"],
            "1 option · Deadline: Dec 31, 2099 21:00 JST"
        );
        assert_eq!(english["allowed_mentions"]["parse"], serde_json::json!([]));
        let japanese = announcement(
            Locale::resolve("fr"),
            &poll,
            "日程調整への回答をお願いします",
            &Locale::Ja.datetime(start),
            "https://example.com/poll",
        );
        assert_eq!(
            japanese["embeds"][0]["description"],
            "候補 1 件・締切 2099/12/31 21:00 JST"
        );
    }
}
