use actix_web::{get, post, put, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{GuildMember, events::parse_guild_ids, member::is_snowflake};
use crate::{
    auth::AuthUser,
    discord::{DiscordError, GuildChannel, GuildRole, Permissions},
    error::{ApiError, ErrorBody},
    models::{
        events::NOTIFICATIONS_MAX,
        guilds::{self, Guild, GuildConfig, GuildConfigUpdate},
        notifications::Notification,
    },
    state::AppState,
};

#[derive(Deserialize, IntoParams)]
pub struct JoinedQuery {
    /// カンマ区切りのギルド ID
    #[param(example = "782502586817314816,123456789012345678")]
    pub guild_ids: String,
}

/// 指定したギルドのうち Bot が参加しているもの。
/// web 側でユーザーの所属ギルド一覧 (Discord API) を取ったあと、カレンダーが使えるものを絞り込むのに使う
#[utoipa::path(
    tag = "guilds",
    params(JoinedQuery),
    responses(
        (status = 200, body = Vec<Guild>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
    )
)]
#[get("/joined")]
pub async fn joined(
    _user: AuthUser,
    query: web::Query<JoinedQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<Guild>>, ApiError> {
    // 解析は横断カレンダー (#98) と共通 (Snowflake でない値は 400、上限は 200)
    let ids = parse_guild_ids(&query.guild_ids)?;
    Ok(web::Json(guilds::find_joined(&state.pool, &ids).await?))
}

/// ギルド情報。呼び出したユーザーがメンバーであることが条件
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = Guild),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / Bot 未参加", body = ErrorBody),
    )
)]
#[get("/{guild_id}")]
pub async fn get_guild(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<Guild>, ApiError> {
    // guilds テーブルは Bot が管理する。まだ書かれていなければ Discord から取った情報で補う
    let guild = match guilds::find_by_guild_id(&state.pool, member.guild_id()).await? {
        Some(guild) => guild,
        None => Guild {
            guild_id: member.access.guild.id.clone(),
            name: member.access.guild.name.clone(),
            avatar_url: member.access.guild.icon_url(),
            locale: "ja".to_owned(),
        },
    };
    Ok(web::Json(guild))
}

/// 呼び出したユーザーのギルド内での権限
#[derive(Serialize, ToSchema)]
pub struct MyPermissions {
    #[schema(example = "123456789012345678")]
    pub user_id: String,
    /// ギルドレベルの基本パーミッション (ビットを文字列化したもの)。
    /// administrator が true なら他のビットに関わらず全権限を持つ
    #[schema(example = "8")]
    pub permissions: String,
    pub administrator: bool,
    pub manage_guild: bool,
    pub manage_messages: bool,
    pub manage_roles: bool,
    /// 上記 4 つのいずれか。サーバー設定の変更可否に使う
    pub can_manage_server: bool,
    /// restricted と編集ロールを含む予定編集の最終判定 (#170)
    pub can_edit_events: bool,
    /// **このユーザー自身**が Discord の「イベントの作成」権限を持つか (#94)。
    /// 連携は Bot が代行するので、これを見ないと本人の権限では作れないイベントを
    /// web 経由で作れてしまう (権限昇格)。予定を連携させる操作の条件
    pub create_events: bool,
    /// **Bot 自身**が「イベントの作成」権限を持つか (#94)。
    /// 予定ダイアログの「Discord のイベントとしても作成する」を出し分けるのに使う。
    /// api 側のキャッシュにより、再招待などの変更が反映されるまで最大で数分の遅れがある
    /// (待てないときは `POST /guilds/{guild_id}/@me/permissions/refresh`、#122)
    pub bot_create_events: bool,
}

/// 応答を組み立てる。`bot_create_events` の求め方は呼び出し側で変える
/// (通常の取得は取れなくても false に倒し、明示的な再確認 (#122) はエラーを返す)
fn build_my_permissions(
    user_id: &str,
    p: Permissions,
    bot_create_events: bool,
    can_edit_events: bool,
) -> MyPermissions {
    MyPermissions {
        user_id: user_id.to_owned(),
        permissions: p.bits().to_string(),
        administrator: p.administrator(),
        manage_guild: p.manage_guild(),
        manage_messages: p.manage_messages(),
        manage_roles: p.manage_roles(),
        can_manage_server: p.can_manage_server(),
        can_edit_events,
        create_events: p.create_events(),
        bot_create_events,
    }
}

#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = MyPermissions),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/{guild_id}/@me/permissions")]
pub async fn my_permissions(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<MyPermissions>, ApiError> {
    // Bot 権限は付加情報なので、取得に失敗してもユーザー自身の権限の応答は返す
    // (ここで全体を失敗させると、連携チェックボックスの可否が不明なだけでカレンダーが開けなくなる)。
    // false 側に倒れるとチェックボックスは案内つきで無効になる
    let bot_create_events = match state.discord.bot_create_events(member.guild_id()).await {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(guild_id = member.guild_id(), error = %err, "failed to check the bot's create events permission");
            false
        }
    };
    Ok(web::Json(build_my_permissions(
        &member.user.discord_user_id,
        member.permissions(),
        bot_create_events,
        guilds::get_config(&state.pool, member.guild_id())
            .await?
            .can_edit_events(
                member.permissions().can_manage_server(),
                &member.access.roles,
            ),
    )))
}

/// 権限のキャッシュを捨てて取り直す (#122)。
///
/// Bot を招待し直したり、ロールを付けてもらった直後に、キャッシュの期限 (最大 5 分) を待たずに
/// 反映させるための操作。応答は [`my_permissions`] と同じで、**捨てたあとに Discord から
/// 取り直した値**で作る。
///
/// 認可は [`GuildMember`] のまま (そのギルドのメンバーであること以外に権限は要らない)。
/// キャッシュを捨てても判定に使うのは常に Discord の最新の値なので、これで権限が緩むことはない。
/// 連打への備えは [`crate::discord::DiscordClient::refresh_permissions`] のスロットル。
///
/// なお **Bot 自体が未参加**のときは extractor の時点で 403 になるので、この操作では直せない
/// (ギルドの負のキャッシュが切れるのを待つ)。誰でも任意のギルド ID のキャッシュを捨てられる形にして
/// Discord への問い合わせを起こせるようにするより、直せる範囲が狭い方を選んでいる
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = MyPermissions),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / Bot 未参加", body = ErrorBody),
        (status = 502, description = "Discord に問い合わせられなかった", body = ErrorBody),
    )
)]
#[post("/{guild_id}/@me/permissions/refresh")]
pub async fn refresh_my_permissions(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<MyPermissions>, ApiError> {
    let guild_id = member.guild_id().to_owned();
    let user_id = member.user.discord_user_id.clone();
    // extractor が取った権限はキャッシュを捨てる前のものなので、取り直した値で作り直す。
    //
    // 利用者が明示的に押した操作なので、Bot 権限を確かめられなかったらエラーを返す
    // (通常の取得 ([`my_permissions`]) のように false へ倒すと、確認できていないのに
    // 「まだ変わっていません」と伝えたうえで、web のキャッシュまで false で上書きしてしまう)。
    // 取り直した結果メンバーでなくなっていたら (退出・Bot の追放) extractor と同じ 403
    let (access, bot_create_events) = state
        .discord
        .refresh_permissions(&guild_id, &user_id)
        .await?
        .ok_or_else(|| {
            ApiError::Forbidden(
                "you are not a member of this guild, or the bot has not joined it".into(),
            )
        })?;
    Ok(web::Json(build_my_permissions(
        &user_id,
        access.permissions,
        bot_create_events,
        guilds::get_config(&state.pool, &guild_id)
            .await?
            .can_edit_events(access.permissions.can_manage_server(), &access.roles),
    )))
}

/// ギルドの編集許可に選べるロール。所属メンバーのみ取得できる。
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses((status = 200, body = Vec<GuildRole>), (status = 401, body = ErrorBody), (status = 403, body = ErrorBody))
)]
#[get("/{guild_id}/roles")]
pub async fn roles(member: GuildMember) -> web::Json<Vec<GuildRole>> {
    web::Json(member.access.guild.editor_roles.clone())
}

/// 通知先に選べるチャンネル (テキスト / アナウンス) の一覧と、Bot がそこに投稿できるか (#181)。
/// メンバーなら誰でも呼べる (予定ごとの通知先 (#175) は予定を作れる人が選ぶため) が、
/// 返すのは**呼び出した本人が Discord で見られるチャンネルだけ** (Bot に見えるからといって、
/// 本人に権限のないスタッフ専用チャンネルの名前を列挙させない)。
/// Bot 側のキャッシュ (最大 1 分) により、チャンネルの追加や権限の変更は少し遅れて反映される
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = Vec<GuildChannel>),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / Bot 未参加", body = ErrorBody),
        (status = 502, description = "Discord に問い合わせられなかった", body = ErrorBody),
    )
)]
#[get("/{guild_id}/channels")]
pub async fn channels(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<GuildChannel>>, ApiError> {
    Ok(web::Json(fetch_channels(&state, &member).await?))
}

/// 呼び出した本人に見えるチャンネルの一覧を取る。Bot が退出済み (キャッシュ上は参加していても
/// Discord から見えない) ならメンバー確認 (extractor) と同じ 403 にする
async fn fetch_channels(
    state: &AppState,
    member: &GuildMember,
) -> Result<Vec<GuildChannel>, ApiError> {
    state
        .discord
        .guild_channels(member.guild_id(), &member.access)
        .await
        .map_err(|err| match err {
            DiscordError::GuildGone => {
                ApiError::Forbidden("the bot has not joined this guild".into())
            }
            other => other.into(),
        })
}

/// ギルド設定 (未設定なら既定値)。通知先チャンネルの ID はサーバー管理権限を持つ人にだけ返す
/// (それ以外は `notification_channel_configured` だけ分かる)
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = GuildConfig),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/{guild_id}/config")]
pub async fn get_config(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<GuildConfig>, ApiError> {
    let config = guilds::get_config(&state.pool, member.guild_id()).await?;
    // 一般メンバーには通知先の ID を見せない (`/init` でスタッフ専用チャンネルが設定されていると、
    // チャンネル一覧では見えないチャンネルの ID がここから分かってしまう)
    Ok(web::Json(if member.permissions().can_manage_server() {
        config
    } else {
        config.without_channel_id()
    }))
}

/// サーバー設定の更新内容。`restricted` 以外は省略すると変更しない
#[derive(Debug, Deserialize, ToSchema)]
pub struct GuildConfigInput {
    pub restricted: bool,
    /// 編集を許可するロール (最大 25 件)。省略は維持、空配列は全解除
    #[serde(default)]
    pub editor_role_ids: Option<Vec<String>>,
    /// 予定の開始時刻に通知するか (#181)
    #[serde(default)]
    pub notify_at_start: Option<bool>,
    /// 新しい予定の既定の事前通知 (#181)。最大 10 件、`num` は 1〜100
    #[serde(default)]
    pub default_notifications: Option<Vec<Notification>>,
    /// 通知先チャンネルの ID (#181)。Bot が投稿できるテキスト / アナウンスチャンネルであること
    /// (`GET /guilds/{guild_id}/channels` の `can_post`)。今の値と同じなら確認しない
    #[serde(default)]
    #[schema(example = "782502586817314820")]
    pub notification_channel_id: Option<String>,
}

impl GuildConfigInput {
    /// 形式の検証 (Discord への問い合わせが要る投稿可否は [`put_config`] で見る)
    pub fn validate(&self) -> Result<(), ApiError> {
        if let Some(ids) = &self.editor_role_ids {
            let mut seen = std::collections::HashSet::new();
            if ids.len() > 25 || ids.iter().any(|id| !is_snowflake(id) || !seen.insert(id)) {
                return Err(ApiError::BadRequest(
                    "editor_role_ids must contain at most 25 unique snowflakes".into(),
                ));
            }
        }
        if let Some(list) = &self.default_notifications {
            Notification::validate_list(list, "default_notifications", NOTIFICATIONS_MAX)?;
        }
        if let Some(channel_id) = &self.notification_channel_id
            && !is_snowflake(channel_id)
        {
            return Err(ApiError::BadRequest(
                "notification_channel_id must be a snowflake".into(),
            ));
        }
        Ok(())
    }
}

/// ギルド設定の更新。サーバー管理権限 (`can_manage_server`) が必要。
/// 通知先チャンネルは `/init` と同じ行 (`event_settings`) を書くので、どちらからでも変更できる
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = GuildConfigInput,
    responses(
        (status = 200, body = GuildConfig),
        (status = 400, description = "通知の件数・値域、チャンネル ID の形式、Bot が投稿できないチャンネル", body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / 管理権限なし", body = ErrorBody),
        (status = 502, description = "チャンネルの確認で Discord に問い合わせられなかった", body = ErrorBody),
    )
)]
#[put("/{guild_id}/config")]
pub async fn put_config(
    member: GuildMember,
    body: web::Json<GuildConfigInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<GuildConfig>, ApiError> {
    if !member.permissions().can_manage_server() {
        return Err(ApiError::Forbidden(
            "manage permission is required to change guild settings".into(),
        ));
    }
    body.validate()?;
    if let Some(ids) = &body.editor_role_ids
        && ids.iter().any(|id| {
            !member
                .access
                .guild
                .editor_roles
                .iter()
                .any(|role| &role.id == id)
        })
    {
        return Err(ApiError::BadRequest(
            "editor_role_ids must be selectable roles of this guild".into(),
        ));
    }
    let guild_id = member.guild_id();
    let current = guilds::get_config(&state.pool, guild_id).await?;

    // 通知先は変えるときだけ Bot の投稿可否を確かめる。`/init` で設定したスレッドなど一覧に無い
    // チャンネルが入っていても、他の設定の保存が止まらないようにする。
    // 一覧は本人に見えるチャンネルに絞ったもの (見えないチャンネルは「このギルドのチャンネルではない」と同じ扱い)
    let new_channel = body
        .notification_channel_id
        .as_deref()
        .filter(|id| current.notification_channel_id.as_deref() != Some(*id));
    if let Some(channel_id) = new_channel {
        let list = fetch_channels(&state, &member).await?;
        let channel = list.iter().find(|c| c.id == channel_id).ok_or_else(|| {
            ApiError::BadRequest(
                "notification_channel_id must be a text or announcement channel of this guild"
                    .into(),
            )
        })?;
        if !channel.can_post {
            return Err(ApiError::BadRequest(format!(
                "the bot cannot post to this channel (missing permissions: {})",
                channel
                    .missing_permissions
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }

    let mut tx = state.pool.begin().await?;
    guilds::upsert_config(
        &mut tx,
        guild_id,
        &GuildConfigUpdate {
            restricted: body.restricted,
            editor_role_ids: body.editor_role_ids.clone(),
            notify_at_start: body.notify_at_start,
            default_notifications: body.default_notifications.clone(),
        },
    )
    .await?;
    if let Some(channel_id) = new_channel {
        let previous = guilds::set_notification_channel(&mut tx, guild_id, channel_id).await?;
        tracing::info!(
            guild_id,
            channel_id,
            previous = previous.as_deref().unwrap_or("-"),
            user_id = %member.user.discord_user_id,
            "notification channel set from the web"
        );
    }
    let config = guilds::get_config(&mut *tx, guild_id).await?;
    tx.commit().await?;
    Ok(web::Json(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notifications::NotificationUnit;

    fn input() -> GuildConfigInput {
        GuildConfigInput {
            restricted: false,
            editor_role_ids: None,
            notify_at_start: Some(true),
            default_notifications: Some(vec![Notification {
                num: 30,
                unit: NotificationUnit::Minutes,
            }]),
            notification_channel_id: Some("782502586817314820".to_owned()),
        }
    }

    #[test]
    fn editor_role_input_rejects_invalid_ids_duplicates_and_overflow() {
        let mut body = input();
        body.editor_role_ids = Some((1..=25).map(|id| id.to_string()).collect());
        assert!(body.validate().is_ok());
        for ids in [
            vec!["123".into(), "123".into()],
            vec!["abc".into()],
            vec!["".into()],
            vec!["1/2".into()],
            (1..=26).map(|id| id.to_string()).collect(),
        ] {
            body.editor_role_ids = Some(ids);
            assert!(body.validate().is_err());
        }
        body.editor_role_ids = Some(vec![]);
        assert!(body.validate().is_ok());
    }

    #[test]
    fn accepts_a_valid_input() {
        assert!(input().validate().is_ok());
        // 省略した項目は検証しない
        let minimal: GuildConfigInput = serde_json::from_str(r#"{"restricted": true}"#).unwrap();
        assert!(minimal.validate().is_ok());
        assert_eq!(minimal.notify_at_start, None);
        assert_eq!(minimal.default_notifications, None);
        assert_eq!(minimal.notification_channel_id, None);
    }

    #[test]
    fn rejects_too_many_default_notifications() {
        let mut i = input();
        i.default_notifications = Some(vec![
            Notification {
                num: 1,
                unit: NotificationUnit::Hours,
            };
            NOTIFICATIONS_MAX + 1
        ]);
        assert!(i.validate().is_err());
    }

    #[test]
    fn rejects_default_notifications_outside_the_num_range() {
        for num in [0, 101] {
            let mut i = input();
            i.default_notifications = Some(vec![Notification {
                num,
                unit: NotificationUnit::Minutes,
            }]);
            assert!(i.validate().is_err(), "{num}");
        }
    }

    #[test]
    fn rejects_unknown_notification_units() {
        let json =
            r#"{"restricted": false, "default_notifications": [{"num": 1, "unit": "years"}]}"#;
        assert!(serde_json::from_str::<GuildConfigInput>(json).is_err());
    }

    #[test]
    fn rejects_non_snowflake_channel_ids() {
        for id in ["", "general", "1/2", "-1", "123456789012345678901"] {
            let mut i = input();
            i.notification_channel_id = Some(id.to_owned());
            assert!(i.validate().is_err(), "{id}");
        }
    }
}
