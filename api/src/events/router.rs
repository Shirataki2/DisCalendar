use crate::error::ApiError;
use crate::events::model::{Events, EventsMessage, EventCreate};
use crate::http;
use actix_web::{
    get, post, put, delete,
    web, HttpMessage, HttpResponse,
    HttpRequest, cookie::Cookie};
use actix_session::Session;
use chrono::Duration;

fn check_session(session: &Session, token: &str) -> Result<bool, ApiError> {
    if let Some(session_token) = session.get::<String>("token")? {
        if session_token == token {
            Ok(true)
        } else {
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

async fn check_token(
    session: &Session,
    id: &str,
    req: &HttpRequest
) -> Result<(), ApiError> {
    let token: Cookie = req.cookie("access_token").ok_or(
        ApiError::new(401, "Unauthorized".to_string())
    )?;
    let token = token.value();
    debug!("Session: {:?}", session.get::<String>("token"));
    if check_session(&session, &token)? {
        debug!("Session Access");
    } else {
        debug!("Register New Session");
        http::check_member(&id, &req).await?;
        session.set("token", token)?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct EventQuery {
    start_at: chrono::NaiveDateTime,
    date_type: String,
}

#[derive(Deserialize)]
struct EventPath {
    guild_id: String,
    event_id: i32,
}

#[get("/events/{guild_id}")]
async fn get_events(
    session: Session,
    guild_id: web::Path<String>,
    query: web::Query<EventQuery>,
    req: HttpRequest
) -> Result<HttpResponse, ApiError> {
    let id = guild_id.into_inner();
    let query = query.into_inner();
    let start_at = query.start_at + Duration::days(-6);
    let date_type = query.date_type;
    let end_at = match date_type.as_str() {
        "month" => start_at + Duration::days(43),
        "week"  => start_at + Duration::days(19),
        "4day"  => start_at + Duration::days(16),
        "day"   => start_at + Duration::days(13),
        _       => start_at + Duration::days(43)
    };
    check_token(&session, &id, &req).await?;
    Ok(HttpResponse::Ok().json(
        Events::find_by_guild_id_with_duration(
            id,
            start_at,
            end_at
        )?
    ))
}

#[post("/events/{guild_id}")]
async fn create_event(
    session: Session,
    guild_id: web::Path<String>,
    event: web::Json<EventCreate>,
    req: HttpRequest
) -> Result<HttpResponse, ApiError> {
    let id = guild_id.into_inner();
    let event = event.into_inner();
    if id != event.guild_id {
        return Err(ApiError::new(401, "Unauthorized".to_string()))
    }
    check_token(&session, &id, &req).await?;
    Ok(HttpResponse::Ok().json(
        Events::create(event)?
    ))
}

#[put("/events/{guild_id}/{event_id}")]
async fn update_event(
    session: Session,
    path: web::Path<EventPath>,
    event: web::Json<EventsMessage>,
    req: HttpRequest
) -> Result<HttpResponse, ApiError> {
    let path = path.into_inner();
    let gid = path.guild_id;
    let eid = path.event_id;
    let event = event.into_inner();
    check_token(&session, &gid, &req).await?;
    Ok(HttpResponse::Ok().json(
        Events::update(eid, event)?
    ))
}

#[delete("/events/{guild_id}/{event_id}")]
async fn delete_event(
    session: Session,
    path: web::Path<EventPath>,
    req: HttpRequest
) -> Result<HttpResponse, ApiError> {
    let path = path.into_inner();
    let gid = path.guild_id;
    let eid = path.event_id;
    check_token(&session, &gid, &req).await?;
    Ok(HttpResponse::Ok().json(
        Events::delete(eid)?
    ))
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_events);
    cfg.service(create_event);
    cfg.service(update_event);
    cfg.service(delete_event);
}
