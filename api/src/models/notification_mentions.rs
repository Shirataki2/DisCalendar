//! 予定単位の通知メンション。Bot の同名モジュールと JSON 形式を揃える。
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    discord::{DiscordClient, Permissions, is_snowflake},
    error::ApiError,
};

pub const MENTIONS_MAX: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationMention {
    Everyone,
    Role { id: String },
    User { id: String },
}

pub fn validate(mentions: &[NotificationMention]) -> Result<(), ApiError> {
    if mentions.len() > MENTIONS_MAX {
        return Err(ApiError::BadRequest(
            "メンション先は10件以内で指定してください".into(),
        ));
    }
    for (index, mention) in mentions.iter().enumerate() {
        if matches!(mention, NotificationMention::Role { id } | NotificationMention::User { id }
            if !is_snowflake(id) || id.parse::<u64>().ok().is_none_or(|n| n == 0 || n.to_string() != *id))
        {
            return Err(ApiError::BadRequest("メンション先のIDが不正です".into()));
        }
        if mentions[..index].contains(mention) {
            return Err(ApiError::BadRequest("メンション先が重複しています".into()));
        }
    }
    Ok(())
}

/// 通常・管理 API の両方で、Bot を使ったメンション権限の迂回を防ぐ。
pub async fn validate_targets(
    discord: &DiscordClient,
    guild_id: &str,
    actor_id: &str,
    mentions: Option<&[NotificationMention]>,
) -> Result<(), ApiError> {
    let Some(mentions) = mentions.filter(|m| !m.is_empty()) else {
        return Ok(());
    };
    validate(mentions)?;
    let access = discord
        .member_access(guild_id, actor_id)
        .await?
        .ok_or_else(|| {
            ApiError::Forbidden("メンションの指定にはサーバーへの参加が必要です".into())
        })?;
    let can_mention_everyone = access.permissions.has(Permissions::MENTION_EVERYONE);
    for mention in mentions {
        match mention {
            NotificationMention::Everyone if !can_mention_everyone => {
                return Err(ApiError::Forbidden(
                    "@everyone の指定には「全てのロールにメンション」権限が必要です".into(),
                ));
            }
            NotificationMention::Role { id } => {
                let role = access
                    .guild
                    .editor_roles
                    .iter()
                    .find(|r| r.id == *id)
                    .ok_or_else(|| {
                        ApiError::BadRequest("このサーバーで選択できないロールです".into())
                    })?;
                if !role.mentionable && !can_mention_everyone {
                    return Err(ApiError::Forbidden(
                        "このロールの指定には「全てのロールにメンション」権限が必要です".into(),
                    ));
                }
            }
            NotificationMention::Everyone | NotificationMention::User { .. } => {}
        }
    }
    let members: Vec<bool> = stream::iter(mentions.iter().filter_map(|mention| match mention {
        NotificationMention::User { id } => Some(id),
        _ => None,
    }))
    .map(|id| discord.is_current_member(guild_id, id))
    .buffered(4)
    .try_collect()
    .await?;
    if members.contains(&false) {
        return Err(ApiError::BadRequest(
            "メンション先のユーザーがサーバーに参加していません".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_bounded_unique_string_ids_and_json_contract() {
        let raw = json!([{"type":"everyone"}, {"type":"role","id":"123456789012345678"}, {"type":"user","id":"234567890123456789"}]);
        let mentions: Vec<NotificationMention> = serde_json::from_value(raw.clone()).unwrap();
        assert!(validate(&mentions).is_ok());
        assert_eq!(serde_json::to_value(&mentions).unwrap(), raw);
        for id in [
            "0",
            "01",
            "-1",
            "18446744073709551616",
            "1><@everyone",
            "../1",
        ] {
            assert!(validate(&[NotificationMention::User { id: id.into() }]).is_err());
        }
        assert!(validate(&vec![NotificationMention::Everyone; 11]).is_err());
        assert!(validate(&[NotificationMention::Everyone, NotificationMention::Everyone]).is_err());
        assert!(
            serde_json::from_value::<NotificationMention>(json!({"type":"user","id":123})).is_err()
        );
        assert!(serde_json::from_value::<NotificationMention>(json!({"type":"here"})).is_err());
    }
}
