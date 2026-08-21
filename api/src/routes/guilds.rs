use actix_web::{get, put, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::GuildMember;
use crate::{
    auth::AuthUser,
    error::{ApiError, ErrorBody},
    models::guilds::{self, Guild, GuildConfig},
    state::AppState,
};

/// 一度に問い合わせられるギルド数の上限 (Discord のユーザーあたり参加上限は 200)
const JOINED_MAX_IDS: usize = 200;

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
    let ids: Vec<String> = query
        .guild_ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if ids.len() > JOINED_MAX_IDS {
        return Err(ApiError::BadRequest(format!(
            "at most {JOINED_MAX_IDS} guild_ids are allowed"
        )));
    }
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
pub async fn my_permissions(member: GuildMember) -> web::Json<MyPermissions> {
    let p = member.permissions();
    web::Json(MyPermissions {
        user_id: member.user.discord_user_id.clone(),
        permissions: p.bits().to_string(),
        administrator: p.administrator(),
        manage_guild: p.manage_guild(),
        manage_messages: p.manage_messages(),
        manage_roles: p.manage_roles(),
        can_manage_server: p.can_manage_server(),
    })
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
