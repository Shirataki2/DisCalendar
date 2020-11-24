use crate::error::ApiError;
use crate::client::{get_client, get_bot, get_api_url};
use actix_web::{HttpMessage, HttpRequest, cookie::Cookie};
use serenity::model::{
    guild::{GuildInfo, PartialGuild, PartialMember},
    user::CurrentUser
};

pub async fn get_current_user_guilds(token: &str) -> Result<Vec<GuildInfo>, ApiError> {
    let client = get_client(token.to_string())?;
    let url = get_api_url("/users/@me/guilds");
    let resp = client.get(&url)
                     .send()
                     .await?
                     .json::<Vec<GuildInfo>>()
                     .await?;
    Ok(resp)
}


pub async fn get_current_user(token: &str) -> Result<CurrentUser, ApiError> {
    let client = get_client(token.to_string())?;
    let url = get_api_url("/users/@me");
    let resp = client.get(&url)
                     .send()
                     .await?
                     .json::<CurrentUser>()
                     .await?;
    Ok(resp)
}

pub async fn get_guild(guild_id: &str) -> Result<PartialGuild, ApiError> {
    let bot = get_bot()?;
    let url = {
        let url = format!(
            "/guilds/{guild_id}",
            guild_id=guild_id
        );
        get_api_url(&url)
    };
    let resp = bot.get(&url)
                  .send()
                  .await?
                  .json::<PartialGuild>()
                  .await?;
    Ok(resp)
}

pub async fn get_guild_member(guild_id: &str, user_id: &str) -> Result<PartialMember, ApiError> {
    let bot = get_bot()?;
    let url = {
        let url = format!(
            "/guilds/{guild_id}/members/{user_id}",
            guild_id=guild_id, user_id=user_id
        );
        get_api_url(&url)
    };
    let resp = bot.get(&url)
                  .send()
                  .await?
                  .json::<PartialMember>()
                  .await?;
    Ok(resp)
}

pub async fn check_member(guild_id: &str, req: &HttpRequest) -> Result<PartialGuild, ApiError> {
    let token: Cookie = req.cookie("access_token").ok_or(
        ApiError::new(401, "Unauthorized".to_string())
    )?;
    let token = token.value();
    let user_guilds: Vec<GuildInfo> = get_current_user_guilds(&token).await?;
    let user: CurrentUser = get_current_user(&token).await?;
    get_guild_member(&guild_id, &user.id.0.to_string()).await?;
    let guild = get_guild(&guild_id).await?;
    match user_guilds.iter().find(|&guild| guild.id.0.to_string() == guild_id) {
        Some(_) => Ok(guild),
        None    => Err(ApiError::new(401, "Unauthorized".to_string()))
    }
}
