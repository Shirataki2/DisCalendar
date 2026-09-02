use actix_web::{get, post, put, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{GuildMember, events::parse_guild_ids};
use crate::{
    auth::AuthUser,
    discord::Permissions,
    error::{ApiError, ErrorBody},
    models::guilds::{self, Guild, GuildConfig},
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
    /// 上記 4 つのいずれか。restricted モードでの編集可否とサーバー設定の変更可否に使う
    pub can_manage_server: bool,
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
fn build_my_permissions(user_id: &str, p: Permissions, bot_create_events: bool) -> MyPermissions {
    MyPermissions {
        user_id: user_id.to_owned(),
        permissions: p.bits().to_string(),
        administrator: p.administrator(),
        manage_guild: p.manage_guild(),
        manage_messages: p.manage_messages(),
        manage_roles: p.manage_roles(),
        can_manage_server: p.can_manage_server(),
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
    )))
}

/// ギルド設定 (未設定なら既定値)
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
    Ok(web::Json(
        guilds::get_config(&state.pool, member.guild_id()).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct GuildConfigInput {
    pub restricted: bool,
}

/// ギルド設定の更新。サーバー管理権限 (`can_manage_server`) が必要
#[utoipa::path(
    tag = "guilds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = GuildConfigInput,
    responses(
        (status = 200, body = GuildConfig),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / 管理権限なし", body = ErrorBody),
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
    Ok(web::Json(
        guilds::upsert_config(&state.pool, member.guild_id(), body.restricted).await?,
    ))
}
