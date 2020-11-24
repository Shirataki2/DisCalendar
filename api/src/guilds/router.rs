use crate::error::ApiError;
use crate::guilds::model::Guilds;
use crate::http;
use actix_web::{get, web, HttpResponse, HttpRequest};

#[get("/guilds/{guild_id}")]
async fn get_guild(guild_id: web::Path<String>) -> Result<HttpResponse, ApiError> {
    Ok(
        HttpResponse::Ok().json(
            http::get_guild(&guild_id.into_inner()).await?
        )
    )
}

#[get("/guilds/check/{guild_id}")]
async fn check_joined(guild_id: web::Path<String>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    Ok(
        HttpResponse::Ok().json(
            http::check_member(&guild_id.into_inner(), &req).await?
        )
    )
}

#[derive(Deserialize)]
pub struct GuildIds {
    guild_ids: String
}

#[get("/guilds/joined")]
async fn get_joined_guild(q: web::Query<GuildIds>) -> Result<HttpResponse, ApiError> {
    let guild_ids: Vec<String> = q.into_inner().guild_ids
        .split(",")
        .map(|s| s.to_string())
        .collect();
    Ok(HttpResponse::Ok().json(
        Guilds::joined(guild_ids)?
    ))
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_joined_guild);
    cfg.service(check_joined);
    cfg.service(get_guild);
}
