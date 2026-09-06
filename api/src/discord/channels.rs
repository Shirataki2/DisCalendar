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
    DiscordClient, DiscordError, MemberAccess, checked_id,
    permissions::{
        PermissionOverwrite, Permissions, compute_base_permissions, compute_channel_permissions,
    },
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

/// キャッシュに持つ 1 件。呼び出し元ごとの「見えるか」の判定に権限上書きも残しておく
/// (Bot の投稿可否は Bot 共通なので計算済みで持つ)
#[derive(Debug, Clone)]
pub(super) struct CachedChannel {
    pub channel: GuildChannel,
    pub overwrites: Vec<PermissionOverwrite>,
}

impl CachedChannel {
    /// `viewer` がこのチャンネルを見られるか (「チャンネルを見る」を上書き込みで持つか)。
    /// Bot には見えても本人には見えないチャンネル (スタッフ専用など) を API から列挙できないようにする
    fn visible_to(&self, guild_id: &str, viewer: &MemberAccess) -> bool {
        compute_channel_permissions(
            viewer.permissions,
            guild_id,
            &viewer.user_id,
            &viewer.roles,
            &self.overwrites,
        )
        .has(Permissions::VIEW_CHANNEL)
    }
}

/// Discord の応答から一覧を組み立てる (純粋関数。テスト用に分けてある)
pub(super) fn build_channel_list(
    guild_id: &str,
    bot_user_id: &str,
    base: Permissions,
    bot_roles: &[String],
    channels: Vec<ApiChannel>,
) -> Vec<CachedChannel> {
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
    let mut sortable: Vec<(i64, i64, u64, CachedChannel)> = channels
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
                CachedChannel {
                    channel: GuildChannel {
                        id: c.id,
                        name: c.name.unwrap_or_default(),
                        category: category.map(|(_, name)| name.clone()),
                        can_post: missing.is_empty(),
                        missing_permissions: missing,
                    },
                    overwrites,
                },
            )
        })
        .collect();
    sortable.sort_by_key(|(category, position, id, _)| (*category, *position, *id));
    sortable.into_iter().map(|(_, _, _, c)| c).collect()
}

impl DiscordClient {
    /// `viewer` に見える、通知先に選べるチャンネルの一覧と Bot の投稿可否。
    /// Bot には見えても `viewer` 本人に「チャンネルを見る」が無いチャンネルは返さない
    /// (一般メンバーがスタッフ専用チャンネルの名前を API から列挙できないようにする)。
    /// Discord から取った一覧は [`super::CHANNELS_TTL`] の間キャッシュする (チャンネルの追加・改名や
    /// 権限変更の反映はその分だけ遅れる。`refresh_permissions` (#122) で捨てられる)。
    /// Bot が未参加 (キャッシュ上は参加していても Discord から 403 / 404) なら [`DiscordError::GuildGone`]
    pub async fn guild_channels(
        &self,
        guild_id: &str,
        viewer: &MemberAccess,
    ) -> Result<Vec<GuildChannel>, DiscordError> {
        let channels = self.all_channels(guild_id).await?;
        // 閲覧者の基本権限は、今のギルド情報 (一覧を作り直したときに取り直したロールごとの権限) から
        // 計算し直す。extractor が組み立てた `viewer.permissions` は最長 5 分前のギルドキャッシュ由来なので、
        // ロールから「チャンネルを見る」を外した直後に古い権限のまま非表示チャンネルを返さないため
        let guild = self.guild(guild_id).await?.ok_or(DiscordError::GuildGone)?;
        let viewer = MemberAccess {
            permissions: compute_base_permissions(
                &guild.id,
                &guild.owner_id,
                &viewer.user_id,
                &guild.role_permissions,
                &viewer.roles,
            ),
            guild,
            user_id: viewer.user_id.clone(),
            roles: viewer.roles.clone(),
        };
        Ok(channels
            .iter()
            .filter(|c| c.visible_to(guild_id, &viewer))
            .map(|c| c.channel.clone())
            .collect())
    }

    /// Bot から見える全チャンネル (キャッシュ込み)。呼び出し元ごとの絞り込みは [`Self::guild_channels`]
    async fn all_channels(&self, guild_id: &str) -> Result<Arc<Vec<CachedChannel>>, DiscordError> {
        if let Some(cached) = self.channels.get(guild_id).await {
            return Ok(cached);
        }
        // Bot の投稿可否はロールの権限 (ギルド情報、最長 5 分のキャッシュ) と Bot 自身のロールからも
        // 決まるので、一覧を作り直すときはそれらも取り直して、一覧と同じ鮮度 (1 分) にそろえる
        // (Bot のロールから「メッセージを送信」を外した直後に、古いロール権限で can_post を誤らないため)
        self.recache_guild_permissions(guild_id).await?;
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
        let list: Vec<GuildChannel> = build_channel_list("g", "bot", base, &[], channels)
            .into_iter()
            .map(|c| c.channel)
            .collect();
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
        assert!(list[0].channel.can_post);
        assert!(!list[1].channel.can_post);
        assert_eq!(
            list[1].channel.missing_permissions,
            [
                NotificationPermission::ViewChannel,
                NotificationPermission::EmbedLinks
            ]
        );
    }

    #[test]
    fn hides_channels_the_viewer_cannot_see() {
        let bot = Permissions::from_bits(
            Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS,
        );
        let channels = vec![
            channel(serde_json::json!({"id": "10", "type": 0, "name": "general", "position": 0})),
            // 「staff」ロールだけが見られる (@everyone は閲覧禁止、Bot はメンバー上書きで許可)
            channel(serde_json::json!({
                "id": "11", "type": 0, "name": "staff", "position": 1,
                "permission_overwrites": [
                    {"id": "g", "type": 0, "allow": "0", "deny": "1024"},
                    {"id": "staff", "type": 0, "allow": "1024", "deny": "0"},
                    {"id": "bot", "type": 1, "allow": "1024", "deny": "0"}
                ]
            })),
        ];
        let list = build_channel_list("g", "bot", bot, &[], channels);
        let viewer = |roles: &[&str], permissions: u64| MemberAccess {
            guild: Arc::new(super::super::GuildSnapshot {
                id: "g".to_owned(),
                name: "guild".to_owned(),
                icon: None,
                owner_id: "owner".to_owned(),
                role_permissions: HashMap::new(),
            }),
            user_id: "member".to_owned(),
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
            permissions: Permissions::from_bits(permissions),
        };
        let visible = |viewer: &MemberAccess| -> Vec<String> {
            list.iter()
                .filter(|c| c.visible_to("g", viewer))
                .map(|c| c.channel.name.clone())
                .collect()
        };
        // 一般メンバーには staff が見えない (Bot には見えて投稿もできるが、列挙させない)
        assert_eq!(
            visible(&viewer(&[], Permissions::VIEW_CHANNEL)),
            ["general"]
        );
        assert!(list[1].channel.can_post);
        // staff ロールを持つ人と管理者には見える
        assert_eq!(
            visible(&viewer(&["staff"], Permissions::VIEW_CHANNEL)),
            ["general", "staff"]
        );
        assert_eq!(
            visible(&viewer(&[], Permissions::ADMINISTRATOR)),
            ["general", "staff"]
        );
    }

    /// 閲覧者の権限は extractor が渡した値ではなく、取り直したギルド情報から計算し直す
    #[tokio::test]
    async fn viewer_permissions_are_recomputed_from_the_refreshed_guild() {
        use actix_web::{App, HttpResponse, HttpServer, web};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = HttpServer::new(move || {
            App::new()
                .route(
                    "/users/@me",
                    web::get()
                        .to(|| async { HttpResponse::Ok().json(serde_json::json!({"id": "333"})) }),
                )
                .route(
                    "/guilds/111",
                    web::get().to(|| async {
                        // @everyone は何も見られない。Bot のロールは全権限
                        HttpResponse::Ok().json(serde_json::json!({
                            "id": "111", "name": "guild", "owner_id": "555",
                            "roles": [
                                {"id": "111", "permissions": "0"},
                                {"id": "999", "permissions": "8"}
                            ]
                        }))
                    }),
                )
                .route(
                    "/guilds/111/members/{id}",
                    web::get().to(|id: web::Path<String>| async move {
                        let roles = if id.as_str() == "333" {
                            vec!["999"]
                        } else {
                            vec![]
                        };
                        HttpResponse::Ok().json(
                            serde_json::json!({"roles": roles, "user": {"username": id.as_str()}}),
                        )
                    }),
                )
                .route(
                    "/guilds/111/channels",
                    web::get().to(|| async {
                        HttpResponse::Ok().json(serde_json::json!([
                            {"id": "10", "type": 0, "name": "general", "position": 0}
                        ]))
                    }),
                )
        })
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        tokio::spawn(server);
        let client = DiscordClient::new("test", &format!("http://{address}")).unwrap();

        // extractor が (古いキャッシュから) 「チャンネルを見る」を持つと判断していた閲覧者。
        // 今のギルド情報では @everyone に権限が無いので、計算し直すと何も見えない
        let stale_viewer = MemberAccess {
            guild: Arc::new(super::super::GuildSnapshot {
                id: "111".to_owned(),
                name: "guild".to_owned(),
                icon: None,
                owner_id: "555".to_owned(),
                role_permissions: HashMap::from([("111".to_owned(), Permissions::VIEW_CHANNEL)]),
            }),
            user_id: "444".to_owned(),
            roles: vec![],
            permissions: Permissions::from_bits(Permissions::VIEW_CHANNEL),
        };
        assert!(
            client
                .guild_channels("111", &stale_viewer)
                .await
                .unwrap()
                .is_empty()
        );
        // オーナーは今の情報でも全権限なので見える (Bot の投稿可否も Bot のロールから出ている)
        let owner = MemberAccess {
            user_id: "555".to_owned(),
            ..stale_viewer.clone()
        };
        let list = client.guild_channels("111", &owner).await.unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].can_post);
        handle.stop(true).await;
    }

    #[test]
    fn missing_permissions_serialize_in_snake_case() {
        for (name, _) in NotificationPermission::ALL {
            let json = serde_json::to_string(&name).unwrap();
            assert_eq!(json, format!("\"{}\"", name.as_str()));
        }
    }
}
