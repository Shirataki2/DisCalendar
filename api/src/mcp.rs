//! MCP 専用の読み取り境界。Web の AuthUser には OAuth を受け付けさせない。
use std::{future::Future, pin::Pin, time::Duration};

use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Datelike as _, FixedOffset, NaiveDateTime, Utc};
use hmac::{Hmac, KeyInit as _, Mac as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{
    error::ApiError,
    models::events::{self, Event, EventRow},
    state::AppState,
};

pub struct McpConfig {
    client: reqwest::Client,
    introspection_url: String,
    issuer: String,
    resource: String,
    secret: String,
}

impl McpConfig {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        if std::env::var("MCP_ENABLED").as_deref() != Ok("true") {
            return Ok(None);
        }
        let origin = std::env::var("MCP_AUTH_ORIGIN")?;
        let secret = std::env::var("MCP_INTROSPECTION_SECRET")?;
        Ok(Some(Self::new(&origin, secret)?))
    }

    pub fn new(origin: &str, secret: String) -> anyhow::Result<Self> {
        let url = reqwest::Url::parse(origin)?;
        anyhow::ensure!(
            url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none()
                && url.username().is_empty()
                && url.password().is_none(),
            "MCP_AUTH_ORIGIN must be an origin"
        );
        anyhow::ensure!(
            url.scheme() == "https"
                || (url.scheme() == "http"
                    && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
            "MCP requires HTTPS or loopback HTTP"
        );
        anyhow::ensure!(
            secret.len() >= 32,
            "MCP_INTROSPECTION_SECRET must be at least 32 bytes"
        );
        let origin = url.origin().ascii_serialization();
        Ok(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()?,
            introspection_url: format!("{origin}/mcp/introspect"),
            issuer: format!("{origin}/api/auth"),
            resource: format!("{origin}/mcp"),
            secret,
        })
    }
}

#[derive(Deserialize)]
struct Introspection {
    active: bool,
    sub: String,
    client_id: String,
    connection_id: String,
    iss: String,
    aud: serde_json::Value,
    exp: i64,
    scope: String,
    guild_ids: Vec<String>,
}

pub struct McpUser {
    sub: String,
    connection_id: String,
    discord_user_id: String,
    scopes: Vec<String>,
    guild_ids: Vec<String>,
}

impl FromRequest for McpUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ApiError>>>>;
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let config = req
                .app_data::<web::Data<Option<McpConfig>>>()
                .and_then(|c| c.as_ref().as_ref())
                .ok_or_else(|| ApiError::NotFound("MCP is disabled".into()))?;
            let token = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .filter(|v| !v.is_empty() && v.len() <= 8192)
                .ok_or(ApiError::Unauthorized)?;
            // リダイレクト・障害・不正な応答は全て拒否し、トークンや応答本文はログに出さない。
            let response = config
                .client
                .post(&config.introspection_url)
                .bearer_auth(&config.secret)
                .json(&serde_json::json!({"token": token}))
                .send()
                .await
                .map_err(|_| ApiError::Unavailable("MCP authentication unavailable".into()))?;
            if !response.status().is_success() {
                return Err(ApiError::Unavailable(
                    "MCP authentication unavailable".into(),
                ));
            }
            let claims: Introspection =
                response.json().await.map_err(|_| ApiError::Unauthorized)?;
            let audience_matches = claims.aud == config.resource
                || claims
                    .aud
                    .as_array()
                    .is_some_and(|values| values.iter().any(|v| v == &config.resource));
            if !claims.active
                || claims.sub.is_empty()
                || claims.connection_id.is_empty()
                || claims.client_id.is_empty()
                || claims.iss != config.issuer
                || !audience_matches
                || claims.exp <= Utc::now().timestamp()
                || claims.guild_ids.len() > 100
                || !claims
                    .guild_ids
                    .iter()
                    .all(|id| crate::discord::is_snowflake(id))
            {
                return Err(ApiError::Unauthorized);
            }
            let state = req
                .app_data::<web::Data<AppState>>()
                .ok_or(ApiError::Unauthorized)?;
            // ユーザー指定の Discord ID は使わず、検証済み sub の連携アカウントから解決する。
            let discord_user_id: Option<String> = sqlx::query_scalar(
                "SELECT \"accountId\" FROM account WHERE \"userId\" = $1 AND \"providerId\" = 'discord'")
                .bind(&claims.sub).fetch_optional(&state.pool).await?;
            Ok(Self {
                sub: claims.sub,
                connection_id: claims.connection_id,
                discord_user_id: discord_user_id.ok_or(ApiError::Unauthorized)?,
                scopes: claims.scope.split_whitespace().map(str::to_owned).collect(),
                guild_ids: claims.guild_ids,
            })
        })
    }
}

impl McpUser {
    fn require_scope(&self, scope: &str) -> Result<(), ApiError> {
        if self.scopes.iter().any(|s| s == scope) {
            Ok(())
        } else {
            Err(ApiError::Forbidden("insufficient scope".into()))
        }
    }
    async fn require_guild(&self, state: &AppState, guild_id: &str) -> Result<(), ApiError> {
        self.require_scope("events:read")?;
        if !self.guild_ids.iter().any(|id| id == guild_id)
            || state
                .discord
                .member_access(guild_id, &self.discord_user_id)
                .await?
                .is_none()
        {
            return Err(ApiError::Forbidden("guild access denied".into()));
        }
        Ok(())
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/mcp")
            .wrap(actix_web::middleware::DefaultHeaders::new().add(("Cache-Control", "no-store")))
            .app_data(web::JsonConfig::default().limit(8192))
            .route("/guilds", web::post().to(list_guilds))
            .route("/events/list", web::post().to(list_events))
            .route("/events/get", web::post().to(get_event)),
    );
}

async fn list_guilds(user: McpUser, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    user.require_scope("guilds:read")?;
    let mut guilds = Vec::new();
    for id in &user.guild_ids {
        if let Some(access) = state
            .discord
            .member_access(id, &user.discord_user_id)
            .await?
        {
            guilds.push(serde_json::json!({"guild_id": id, "name": access.guild.name}));
        }
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({"guilds": guilds})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetInput {
    guild_id: String,
    event_id: i32,
}

async fn get_event(
    user: McpUser,
    input: web::Json<GetInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    user.require_guild(&state, &input.guild_id).await?;
    let row = events::find_by_id(&state.pool, &input.guild_id, input.event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"event": output_event(row)?})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListInput {
    guild_id: String,
    start: String,
    end: String,
    limit: Option<u32>,
    cursor: Option<String>,
}

fn parse_time(value: &str) -> Result<NaiveDateTime, ApiError> {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| ApiError::BadRequest("offset-required ISO 8601 expected".into()))?;
    let time = parsed
        .naive_utc()
        .checked_add_signed(chrono::Duration::hours(9))
        .filter(|t| (1..=9999).contains(&t.year()))
        .ok_or_else(|| ApiError::BadRequest("date out of range".into()))?;
    Ok(time)
}

#[derive(Serialize, Deserialize, PartialEq)]
struct Cursor {
    sub: String,
    connection_id: String,
    guild_id: String,
    start: NaiveDateTime,
    end: NaiveDateTime,
    limit: u32,
    after: Option<(NaiveDateTime, i32)>,
}

fn cursor_mac(secret: &str) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(b"discalendar:mcp:cursor:v1:");
    mac
}

fn encode_cursor(cursor: &Cursor, secret: &str) -> Result<String, ApiError> {
    let payload = serde_json::to_vec(cursor).map_err(anyhow::Error::from)?;
    let mut mac = cursor_mac(secret);
    mac.update(&payload);
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&payload),
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    ))
}

fn decode_cursor(raw: &str, expected: &Cursor, secret: &str) -> Result<Cursor, ApiError> {
    let invalid = || ApiError::BadRequest("invalid cursor or changed search conditions".into());
    let (payload, signature) = raw
        .split_once('.')
        .filter(|_| raw.len() <= 4096)
        .ok_or_else(invalid)?;
    let payload = URL_SAFE_NO_PAD.decode(payload).map_err(|_| invalid())?;
    let signature = URL_SAFE_NO_PAD.decode(signature).map_err(|_| invalid())?;
    let mut mac = cursor_mac(secret);
    mac.update(&payload);
    mac.verify_slice(&signature).map_err(|_| invalid())?;
    let mut decoded: Cursor = serde_json::from_slice(&payload).map_err(|_| invalid())?;
    let after = decoded.after.take().ok_or_else(invalid)?;
    if &decoded != expected {
        return Err(invalid());
    }
    decoded.after = Some(after);
    Ok(decoded)
}

async fn list_events(
    user: McpUser,
    input: web::Json<ListInput>,
    state: web::Data<AppState>,
    config: web::Data<Option<McpConfig>>,
) -> Result<HttpResponse, ApiError> {
    user.require_guild(&state, &input.guild_id).await?;
    let start = parse_time(&input.start)?;
    let end = parse_time(&input.end)?;
    let limit = input.limit.unwrap_or(50);
    if end <= start || end - start > chrono::Duration::days(93) || !(1..=100).contains(&limit) {
        return Err(ApiError::BadRequest(
            "range must be positive and at most 93 days; limit must be 1..100".into(),
        ));
    }
    let config = config.as_ref().as_ref().ok_or(ApiError::Unauthorized)?;
    let mut cursor = Cursor {
        sub: user.sub,
        connection_id: user.connection_id,
        guild_id: input.guild_id.clone(),
        start,
        end,
        limit,
        after: None,
    };
    if let Some(raw) = &input.cursor {
        cursor = decode_cursor(raw, &cursor, &config.secret)?;
    }
    let mut rows = list_rows(
        &state.pool,
        &cursor.guild_id,
        start,
        end,
        cursor.after,
        limit,
    )
    .await?;
    let next = if rows.len() > limit as usize {
        rows.pop();
        cursor.after = rows.last().map(|r| (r.start_at, r.id));
        Some(encode_cursor(&cursor, &config.secret)?)
    } else {
        None
    };
    let events = rows
        .into_iter()
        .map(output_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"events": events, "next_cursor": next})))
}

pub async fn list_rows(
    pool: &sqlx::PgPool,
    guild_id: &str,
    start: NaiveDateTime,
    end: NaiveDateTime,
    after: Option<(NaiveDateTime, i32)>,
    limit: u32,
) -> sqlx::Result<Vec<EventRow>> {
    // 半開区間の重なり。終日の終了日は包含日、時刻が同じ予定は開始時刻の一点として扱う。
    sqlx::query_as::<_, EventRow>(
        r#"
        SELECT e.*, l.scheduled_event_id AS discord_scheduled_event_id
        FROM events e LEFT JOIN event_discord_links l ON l.event_id = e.id
        WHERE e.guild_id = $1
          AND (CASE WHEN e.is_all_day THEN date_trunc('day', e.start_at) ELSE e.start_at END) < $3
          AND (CASE WHEN e.is_all_day THEN date_trunc('day', e.end_at) + INTERVAL '1 day' > $2
                    WHEN e.start_at = e.end_at THEN e.start_at >= $2 ELSE e.end_at > $2 END)
          AND ($4::timestamp IS NULL OR (e.start_at, e.id) > ($4, $5))
        ORDER BY e.start_at, e.id LIMIT $6
    "#,
    )
    .bind(guild_id)
    .bind(start)
    .bind(end)
    .bind(after.map(|a| a.0))
    .bind(after.map(|a| a.1))
    .bind(i64::from(limit) + 1)
    .fetch_all(pool)
    .await
}

pub fn output_event(row: EventRow) -> Result<serde_json::Value, ApiError> {
    // 生の保存内容全体をハッシュ化する。updated_at が NULL / 同値でも内容の変更を検出する。
    let version = format!(
        "sha256-v1:{}",
        URL_SAFE_NO_PAD.encode(Sha256::digest(
            serde_json::to_vec(&row).map_err(anyhow::Error::from)?
        ))
    );
    let event = Event::from(row);
    let mut value = serde_json::to_value(&event).map_err(anyhow::Error::from)?;
    let offset = FixedOffset::east_opt(9 * 3600).expect("JST offset");
    for (key, time) in [
        ("start_at", Some(event.start_at)),
        ("end_at", Some(event.end_at)),
        ("created_at", Some(event.created_at)),
        ("updated_at", event.updated_at),
    ] {
        value[key] = match time {
            Some(t) if event.is_all_day && matches!(key, "start_at" | "end_at") => {
                t.date().to_string().into()
            }
            Some(t) => t
                .and_local_timezone(offset)
                .single()
                .ok_or_else(|| ApiError::BadRequest("date out of range".into()))?
                .to_rfc3339()
                .into(),
            None => serde_json::Value::Null,
        };
    }
    value["version"] = version.into();
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpServer, test};
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use utoipa_actix_web::AppExt as _;

    #[actix_web::test]
    async fn cursor_and_offsets() {
        let start = parse_time("2026-09-13T00:00:00+09:00").unwrap();
        assert_eq!(start, parse_time("2026-09-12T15:00:00Z").unwrap());
        for bad in ["2026-09-13", "2026-09-13T00:00:00", "invalid"] {
            assert!(parse_time(bad).is_err());
        }
        let mut expected = Cursor {
            sub: "u1".into(),
            connection_id: "c1".into(),
            guild_id: "111".into(),
            start,
            end: start + chrono::Duration::days(1),
            limit: 50,
            after: None,
        };
        expected.after = Some((start, 1));
        let token = encode_cursor(&expected, "secret").unwrap();
        expected.after = None;
        assert!(decode_cursor(&token, &expected, "secret").is_ok());
        assert!(decode_cursor(&token, &expected, "wrong").is_err());
        assert!(decode_cursor(&format!("{token}x"), &expected, "secret").is_err());
        for field in ["sub", "connection_id", "guild_id", "start", "end", "limit"] {
            let mut changed = serde_json::to_value(&expected).unwrap();
            changed[field] = match field {
                "limit" => json!(1),
                "start" | "end" => json!("2026-09-15T00:00:00"),
                _ => json!("other"),
            };
            assert!(
                decode_cursor(&token, &serde_json::from_value(changed).unwrap(), "secret").is_err()
            );
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn http_auth_and_reads(pool: sqlx::PgPool) {
        sqlx::raw_sql(
            r#"
            CREATE TABLE "account" ("userId" TEXT, "providerId" TEXT, "accountId" TEXT);
            INSERT INTO account VALUES ('u1', 'discord', '333'), ('u2', 'discord', '444');
            INSERT INTO events (guild_id, name, start_at, end_at, is_all_day) VALUES
              ('111', '終日', '2026-09-13', '2026-09-13', true),
              ('111', '境界で終了', '2026-09-13 01:00', '2026-09-13 12:00', false),
              ('111', '一点', '2026-09-13 12:00', '2026-09-13 12:00', false),
              ('111', '同じ開始時刻', '2026-09-13 12:00', '2026-09-13 13:00', false),
              ('111', '範囲外', '2026-09-14', '2026-09-14 01:00', false),
              ('222', '別サーバー', '2026-09-13', '2026-09-14', false);
        "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let valid = json!({"active":true,"sub":"u1","client_id":"client","connection_id":"c1",
            "iss":format!("{origin}/api/auth"),"aud":format!("{origin}/mcp"),
            "exp":Utc::now().timestamp()+900,"scope":"guilds:read events:read","guild_ids":["111","222","333"]});
        let claims = Arc::new(Mutex::new(valid.clone()));
        let mock_claims = claims.clone();
        let server = HttpServer::new(move || {
            let claims = mock_claims.clone();
            App::new()
                .route(
                    "/mcp/introspect",
                    web::post().to(
                        move |req: HttpRequest, body: web::Json<serde_json::Value>| {
                            let claims = claims.clone();
                            async move {
                                assert_eq!(
                                    req.headers().get("authorization").unwrap(),
                                    "Bearer 01234567890123456789012345678901"
                                );
                                if body["token"] != "oauth-test" {
                                    return HttpResponse::Ok().json(json!({"active":false}));
                                }
                                let value = claims.lock().unwrap().clone();
                                if value.is_null() {
                                    return HttpResponse::ServiceUnavailable().finish();
                                }
                                HttpResponse::Ok().json(value)
                            }
                        },
                    ),
                )
                .route(
                    "/guilds/{guild_id}",
                    web::get().to(|id: web::Path<String>| async move {
                        if id.as_str() != "111" && id.as_str() != "333" {
                            return HttpResponse::NotFound().finish();
                        }
                        HttpResponse::Ok().json(
                            json!({"id":id.as_str(),"name":"サーバー","owner_id":"333","roles":[]}),
                        )
                    }),
                )
                .route(
                    "/guilds/{guild_id}/members/{user_id}",
                    web::get().to(|path: web::Path<(String, String)>| async move {
                        if path.0 != "111" || path.1 != "333" {
                            return HttpResponse::NotFound().finish();
                        }
                        HttpResponse::Ok().json(json!({"roles":[],"user":{"username":"member"}}))
                    }),
                )
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        tokio::spawn(server);
        let state = web::Data::new(AppState {
            pool: pool.clone(),
            sql_console_pool: pool.clone(),
            sql_known_words: tokio::sync::Mutex::new(None),
            discord: crate::discord::DiscordClient::new("test", &origin).unwrap(),
            site_base_url: origin.clone(),
            event_update_locks: Default::default(),
            auth: crate::auth::AuthConfig {
                secret: "web-secret".into(),
                cookie_names: vec![],
            },
            activity_days: moka::future::Cache::new(1),
            admin: Default::default(),
            started_at: Utc::now(),
        });
        let app = test::init_service(
            App::new()
                .into_utoipa_app()
                .configure(crate::routes::configure)
                .into_app()
                .app_data(state)
                .app_data(web::Data::new(Some(
                    McpConfig::new(&origin, "01234567890123456789012345678901".into()).unwrap(),
                )))
                .configure(configure),
        )
        .await;
        let request = |path: &str, body: serde_json::Value| {
            test::TestRequest::post()
                .uri(path)
                .insert_header(("Authorization", "Bearer oauth-test"))
                .set_json(body)
                .to_request()
        };
        let response = test::call_service(&app, request("/mcp/guilds", json!({}))).await;
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            test::read_body_json::<serde_json::Value, _>(response).await["guilds"],
            json!([{"guild_id":"111","name":"サーバー"}])
        );
        for path in ["/admin/me", "/guilds/111/config", "/events/111"] {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(path)
                    .insert_header(("Authorization", "Bearer oauth-test"))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), 401, "{path}");
        }
        for (body, status) in [
            (json!({"guild_id":"111","event_id":6}), 404),
            (json!({"guild_id":"999","event_id":1}), 403),
            (json!({"guild_id":"222","event_id":6}), 403),
            (json!({"guild_id":"333","event_id":1}), 403),
            (json!({"guild_id":"111","event_id":1,"user_id":"u2"}), 400),
        ] {
            assert_eq!(
                test::call_service(&app, request("/mcp/events/get", body))
                    .await
                    .status(),
                status
            );
        }
        let single: serde_json::Value = test::call_and_read_body_json(
            &app,
            request("/mcp/events/get", json!({"guild_id":"111","event_id":1})),
        )
        .await;
        assert_eq!(single["event"]["start_at"], "2026-09-13");
        assert_eq!(single["event"]["end_at"], "2026-09-13");
        assert!(single["event"]["updated_at"].is_null());
        sqlx::query("UPDATE events SET name = '変更' WHERE id=1")
            .execute(&pool)
            .await
            .unwrap();
        let changed: serde_json::Value = test::call_and_read_body_json(
            &app,
            request("/mcp/events/get", json!({"guild_id":"111","event_id":1})),
        )
        .await;
        assert_ne!(single["event"]["version"], changed["event"]["version"]);
        let base = json!({"guild_id":"111","start":"2026-09-13T12:00:00+09:00","end":"2026-09-14T00:00:00+09:00","limit":2});
        let first: serde_json::Value =
            test::call_and_read_body_json(&app, request("/mcp/events/list", base.clone())).await;
        assert_eq!(
            first["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(first["events"][1]["start_at"], "2026-09-13T12:00:00+09:00");
        let mut next = base.clone();
        next["cursor"] = first["next_cursor"].clone();
        let second: serde_json::Value =
            test::call_and_read_body_json(&app, request("/mcp/events/list", next.clone())).await;
        assert_eq!(second["events"][0]["id"], 4);
        assert!(second["next_cursor"].is_null());
        next["limit"] = json!(3);
        assert_eq!(
            test::call_service(&app, request("/mcp/events/list", next))
                .await
                .status(),
            400
        );
        for (key, value) in [
            ("limit", json!(0)),
            ("limit", json!(101)),
            ("start", json!("2026-09-13T12:00:00")),
            ("end", json!("2027-01-01T00:00:00Z")),
            ("end", base["start"].clone()),
            ("cursor", json!("tampered")),
        ] {
            let mut bad = base.clone();
            bad[key] = value;
            assert_eq!(
                test::call_service(&app, request("/mcp/events/list", bad))
                    .await
                    .status(),
                400
            );
        }
        let empty: serde_json::Value = test::call_and_read_body_json(&app,request("/mcp/events/list",json!({"guild_id":"111","start":"2026-10-01T00:00:00Z","end":"2026-10-02T00:00:00Z"}))).await;
        assert_eq!(empty["events"], json!([]));
        assert!(empty["next_cursor"].is_null());
        sqlx::query("INSERT INTO events (guild_id, name, start_at, end_at) SELECT '111', '上限', '2026-11-01'::timestamp, '2026-11-02'::timestamp FROM generate_series(1, 105)").execute(&pool).await.unwrap();
        for (limit, count) in [(None, 50), (Some(100), 100)] {
            let mut query = json!({"guild_id":"111","start":"2026-11-01T00:00:00+09:00","end":"2026-11-02T00:00:00+09:00"});
            if let Some(limit) = limit {
                query["limit"] = json!(limit);
            }
            let page: serde_json::Value =
                test::call_and_read_body_json(&app, request("/mcp/events/list", query)).await;
            assert_eq!(page["events"].as_array().unwrap().len(), count);
            assert!(page["next_cursor"].is_string());
        }
        for (key, value, status) in [
            ("scope", json!("guilds:read"), 403),
            ("exp", json!(1), 401),
            ("iss", json!("wrong"), 401),
            ("aud", json!(["wrong"]), 401),
            ("active", json!(false), 401),
            ("sub", json!("missing"), 401),
            ("sub", json!("u2"), 403),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            *claims.lock().unwrap() = invalid;
            assert_eq!(
                test::call_service(
                    &app,
                    request("/mcp/events/get", json!({"guild_id":"111","event_id":1}))
                )
                .await
                .status(),
                status,
                "{key}"
            );
        }
        // 同じトークンの次の操作も必ず検証口を通る。
        *claims.lock().unwrap() = json!({"active":false});
        assert_eq!(
            test::call_service(&app, request("/mcp/guilds", json!({})))
                .await
                .status(),
            401
        );
        *claims.lock().unwrap() = serde_json::Value::Null;
        assert_eq!(
            test::call_service(&app, request("/mcp/guilds", json!({})))
                .await
                .status(),
            503
        );
        handle.stop(true).await;
    }
}
