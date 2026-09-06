//! ギルドのチャンネル一覧と、Bot がそこに通知を投稿できるかの判定 (#181。#175 でも使う)。
//!
//! `GET /guilds/{id}/channels` はスレッドを含まない (Bot が見える通常のチャンネルだけ)。
//! 通知先にできるテキスト (type 0) とアナウンス (type 5) のチャンネルに絞り、カテゴリ (type 4) は
//! 表示用のグループ名として添える。Bot の権限はロールにチャンネルの上書きを反映して計算する
//! ([`compute_channel_permissions`])。判定の基準 (「チャンネルを見る」「メッセージを送信」
//! 「埋め込みリンク」) は Bot の `/init` (`bot/src/checks.rs` の `notification_permissions`) と同じ

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    DiscordClient, DiscordError, checked_id,
    permissions::{PermissionOverwrite, Permissions, compute_channel_permissions},
};

const CHANNEL_TYPE_TEXT: u8 = 0;
const CHANNEL_TYPE_CATEGORY: u8 = 4;
const CHANNEL_TYPE_ANNOUNCEMENT: u8 = 5;

/// `GET /guilds/{id}/channels` の 1 件のうち使う部分
#[derive(Debug, Deserialize)]
pub(super) struct ApiChannel {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    pub name: Option<String>,
    #[serde(default)]
    pub position: i64,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub permission_overwrites: Vec<ApiOverwrite>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiOverwrite {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    /// Discord は permissions を文字列化した整数で返す
    pub allow: String,
    pub deny: String,
}

impl ApiOverwrite {
    fn into_overwrite(self) -> PermissionOverwrite {
        PermissionOverwrite {
            id: self.id,
            kind: self.kind,
            allow: self.allow.parse().unwrap_or(0),
            deny: self.deny.parse().unwrap_or(0),
        }
    }
}

/// 通知の投稿に必要な権限。足りないものを利用者に示すために名前で返す
/// (表示名は web 側が持つ。Bot の `describe_permissions` と同じ 3 つ)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPermission {
    ViewChannel,
    SendMessages,
    EmbedLinks,
}

impl NotificationPermission {
    /// API の表現 (snake_case) と同じ名前。エラーメッセージに入れる
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ViewChannel => "view_channel",
            Self::SendMessages => "send_messages",
            Self::EmbedLinks => "embed_links",
        }
    }

    const ALL: [(Self, u64); 3] = [
        (Self::ViewChannel, Permissions::VIEW_CHANNEL),
        (Self::SendMessages, Permissions::SEND_MESSAGES),
        (Self::EmbedLinks, Permissions::EMBED_LINKS),
    ];

    /// `permissions` に足りないもの (全部そろっていれば空)
    fn missing(permissions: Permissions) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|(_, bit)| !permissions.has(*bit))
            .map(|(name, _)| name)
            .collect()
    }
}

/// 通知先に選べるチャンネル (テキスト / アナウンス)。Discord の表示順 (カテゴリ → 位置) に並ぶ
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct GuildChannel {
    #[schema(example = "782502586817314820")]
    pub id: String,
    #[schema(example = "予定の通知")]
    pub name: String,
    /// 所属するカテゴリの名前 (無ければ null)。表示のグループ分けに使う
    pub category: Option<String>,
    /// Bot がこのチャンネルに通知を投稿できるか (「チャンネルを見る」「メッセージを送信」
    /// 「埋め込みリンク」を持つか)。false のチャンネルは通知先に設定できない
    pub can_post: bool,
    /// `can_post` が false のときに足りない権限 (true なら空)
    pub missing_permissions: Vec<NotificationPermission>,
}

/// Discord の応答から一覧を組み立てる (純粋関数。テスト用に分けてある)
pub(super) fn build_channel_list(
    guild_id: &str,
    bot_user_id: &str,
    base: Permissions,
    bot_roles: &[String],
    channels: Vec<ApiChannel>,
) -> Vec<GuildChannel> {
    // カテゴリの id → (位置, 名前)。カテゴリ自体の並びで先にグループ分けする
    let categories: HashMap<String, (i64, String)> = channels
        .iter()
        .filter(|c| c.kind == CHANNEL_TYPE_CATEGORY)
        .map(|c| {
            (
                c.id.clone(),
                (c.position, c.name.clone().unwrap_or_default()),
            )
        })
        .collect();
    let mut sortable: Vec<(i64, i64, u64, GuildChannel)> = channels
        .into_iter()
        .filter(|c| matches!(c.kind, CHANNEL_TYPE_TEXT | CHANNEL_TYPE_ANNOUNCEMENT))
        .map(|c| {
            let overwrites: Vec<PermissionOverwrite> = c
                .permission_overwrites
                .into_iter()
                .map(ApiOverwrite::into_overwrite)
                .collect();
            let permissions =
                compute_channel_permissions(base, guild_id, bot_user_id, bot_roles, &overwrites);
            let missing = NotificationPermission::missing(permissions);
            let category = c.parent_id.as_ref().and_then(|id| categories.get(id));
            // カテゴリに属さないチャンネルは Discord でも一番上に出る
            let category_position = category.map_or(i64::MIN, |(position, _)| *position);
            let id_order = c.id.parse::<u64>().unwrap_or(u64::MAX);
            (
                category_position,
                c.position,
                id_order,
                GuildChannel {
                    id: c.id,
                    name: c.name.unwrap_or_default(),
                    category: category.map(|(_, name)| name.clone()),
                    can_post: missing.is_empty(),
                    missing_permissions: missing,
                },
            )
        })
        .collect();
    sortable.sort_by_key(|(category, position, id, _)| (*category, *position, *id));
    sortable.into_iter().map(|(_, _, _, c)| c).collect()
}

impl DiscordClient {
    /// 通知先に選べるチャンネルの一覧と Bot の投稿可否。
    /// [`super::CHANNELS_TTL`] の間はキャッシュを返す (チャンネルの追加・改名や権限変更の反映は
    /// その分だけ遅れる。`refresh_permissions` (#122) で捨てられる)。
    /// Bot が未参加 (キャッシュ上は参加していても Discord から 403 / 404) なら [`DiscordError::GuildGone`]
    pub async fn guild_channels(
        &self,
        guild_id: &str,
    ) -> Result<Arc<Vec<GuildChannel>>, DiscordError> {
        if let Some(cached) = self.channels.get(guild_id).await {
            return Ok(cached);
        }
        let bot_user_id = self.bot_user_id().await?;
        let access = self
            .member_access(guild_id, &bot_user_id)
            .await?
            .ok_or(DiscordError::GuildGone)?;
        let raw: Vec<ApiChannel> = self
            .get_json(&format!("/guilds/{}/channels", checked_id(guild_id)?))
            .await?
            .ok_or(DiscordError::GuildGone)?;
        let list = Arc::new(build_channel_list(
            guild_id,
            &bot_user_id,
            access.permissions,
            &access.roles,
            raw,
        ));
        self.channels
            .insert(guild_id.to_owned(), list.clone())
            .await;
        Ok(list)
    }

    /// 通知先にできる (`can_post` の) チャンネルか。一覧に無いチャンネル (スレッド・ボイス・存在しない ID) は false
    pub async fn can_post_to(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> Result<bool, DiscordError> {
        Ok(self
            .guild_channels(guild_id)
            .await?
            .iter()
            .any(|c| c.id == channel_id && c.can_post))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(json: serde_json::Value) -> ApiChannel {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn lists_text_and_announcement_channels_in_display_order() {
        let base = Permissions::from_bits(
            Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS,
        );
        let channels = vec![
            channel(
                serde_json::json!({"id": "30", "type": 0, "name": "in-category", "position": 0, "parent_id": "20"}),
            ),
            channel(serde_json::json!({"id": "20", "type": 4, "name": "予定", "position": 1})),
            channel(serde_json::json!({"id": "12", "type": 2, "name": "voice", "position": 0})),
            channel(serde_json::json!({"id": "11", "type": 5, "name": "news", "position": 1})),
            channel(serde_json::json!({"id": "10", "type": 0, "name": "general", "position": 0})),
        ];
        let list = build_channel_list("g", "bot", base, &[], channels);
        let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
        // カテゴリ無し (位置順) → カテゴリ内。ボイスとカテゴリ自体は出ない
        assert_eq!(names, ["general", "news", "in-category"]);
        assert_eq!(list[2].category.as_deref(), Some("予定"));
        assert!(
            list.iter()
                .all(|c| c.can_post && c.missing_permissions.is_empty())
        );
    }

    #[test]
    fn reports_missing_permissions_from_overwrites() {
        let base = Permissions::from_bits(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES);
        let channels = vec![
            // ロールで埋め込みを許可されている
            channel(serde_json::json!({
                "id": "10", "type": 0, "name": "ok", "position": 0,
                "permission_overwrites": [{"id": "r1", "type": 0, "allow": "16384", "deny": "0"}]
            })),
            // @everyone で閲覧を禁止されている (埋め込みも元々無い)
            channel(serde_json::json!({
                "id": "11", "type": 0, "name": "staff", "position": 1,
                "permission_overwrites": [{"id": "g", "type": 0, "allow": "0", "deny": "1024"}]
            })),
        ];
        let list = build_channel_list("g", "bot", base, &["r1".to_owned()], channels);
        assert!(list[0].can_post);
        assert!(!list[1].can_post);
        assert_eq!(
            list[1].missing_permissions,
            [
                NotificationPermission::ViewChannel,
                NotificationPermission::EmbedLinks
            ]
        );
    }

    #[test]
    fn missing_permissions_serialize_in_snake_case() {
        for (name, _) in NotificationPermission::ALL {
            let json = serde_json::to_string(&name).unwrap();
            assert_eq!(json, format!("\"{}\"", name.as_str()));
        }
    }
}
