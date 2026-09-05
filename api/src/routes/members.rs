//! 予定の詳細を開いたときだけ、作成者・更新者の表示名をまとめて解決する。
use actix_web::{get, web};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::Deserialize;
use utoipa::IntoParams;

use super::GuildMember;
use crate::{
    discord::MemberProfile,
    error::{ApiError, ErrorBody},
    models::events,
    state::AppState,
};

const MAX_IDS: usize = 20;

#[derive(Deserialize, IntoParams)]
pub struct MembersQuery {
    /// カンマ区切りの Discord ユーザー ID (最大 20 件)
    ids: String,
}

fn parse_ids(ids: &str) -> Result<Vec<String>, ApiError> {
    let ids: Vec<_> = ids.split(',').collect();
    if ids.len() > MAX_IDS
        || ids
            .iter()
            .any(|id| id.parse::<u64>().ok().is_none_or(|n| n.to_string() != *id))
    {
        return Err(ApiError::BadRequest(
            "ids must contain 1 to 20 snowflakes".into(),
        ));
    }
    let mut ids: Vec<_> = ids.into_iter().map(str::to_owned).collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[utoipa::path(
    tag = "guilds",
    params(MembersQuery, ("guild_id" = String, Path, description = "ギルド ID")),
    responses((status = 200, body = Vec<MemberProfile>), (status = 400, body = ErrorBody),
              (status = 401, body = ErrorBody), (status = 403, body = ErrorBody))
)]
#[get("/{guild_id}/members")]
pub async fn profiles(
    member: GuildMember,
    query: web::Query<MembersQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<MemberProfile>>, ApiError> {
    let ids = parse_ids(&query.ids)?;
    // 任意 ID による Discord API の総当たりを防ぐ。クライアントは操作者を保存時に偽装できない。
    let authors = events::author_ids(&state.pool, member.guild_id(), &ids).await?;
    if authors.len() != ids.len() {
        return Err(ApiError::BadRequest(
            "ids must be authors of events in this guild".into(),
        ));
    }
    let profiles = stream::iter(ids)
        .map(|id| {
            let discord = &state.discord;
            let guild_id = member.guild_id();
            async move { discord.member_profile(guild_id, &id).await }
        })
        .buffered(4)
        .try_collect()
        .await?;
    Ok(web::Json(profiles))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_bounded_validated_and_deduplicated() {
        assert_eq!(parse_ids("333,111,333").unwrap(), vec!["111", "333"]);
        for ids in ["", "1,", "../1", "1/2", "01", "18446744073709551616"] {
            assert!(parse_ids(ids).is_err(), "{ids}");
        }
        assert!(parse_ids(&vec!["1"; 21].join(",")).is_err());
    }
}
