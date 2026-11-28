//! サーバー用 API トークン (#405) の管理 API と外部公開フック API の統合テスト。

use actix_web::{
    App, HttpMessage, HttpResponse, HttpServer,
    dev::Service,
    http::{StatusCode, header::AUTHORIZATION},
    test, web,
};
use discalendar_api::{
    auth::{AuthConfig, AuthUser},
    discord::DiscordClient,
    models::{
        api_tokens::{self, AUTH_FAILURE_LIMIT_PER_MINUTE, RATE_LIMIT_PER_MINUTE},
        event_history,
    },
    openapi::ApiDoc,
    routes,
    state::{AdminConfig, AppState},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::OpenApi as _;
use utoipa_actix_web::AppExt;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";
const MANAGER_ID: &str = "333333333333333333";
const MEMBER_ID: &str = "444444444444444444";

async fn start_mock_discord() -> (String, actix_web::dev::ServerHandle) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(|| {
        App::new()
            .route(
                "/guilds/{guild_id}",
                web::get().to(|guild_id: web::Path<String>| async move {
                    let id = guild_id.into_inner();
                    if id == GUILD || id == OTHER_GUILD {
                        HttpResponse::Ok().json(json!({
                            "id": id,
                            "name": "テストサーバー",
                            "icon": null,
                            "owner_id": "999999999999999999",
                            "roles": [
                                {"id": id, "permissions": "0", "name": "@everyone", "color": 0, "position": 0, "managed": false},
                                {"id": "888888888888888888", "permissions": "32", "name": "Manager", "color": 0, "position": 1, "managed": false}
                            ]
                        }))
                    } else {
                        HttpResponse::NotFound().finish()
                    }
                }),
            )
            .route(
                "/guilds/{guild_id}/members/{user_id}",
                web::get().to(|path: web::Path<(String, String)>| async move {
                    let (_guild_id, user_id) = path.into_inner();
                    match user_id.as_str() {
                        MANAGER_ID => HttpResponse::Ok().json(json!({"roles": ["888888888888888888"]})),
                        MEMBER_ID => HttpResponse::Ok().json(json!({"roles": []})),
                        _ => HttpResponse::NotFound().finish(),
                    }
                }),
            )
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    (format!("http://{address}"), handle)
}

fn build_state(pool: PgPool, discord_origin: &str) -> web::Data<AppState> {
    web::Data::new(AppState {
        pool: pool.clone(),
        sql_console_pool: pool,
        sql_known_words: Default::default(),
        discord: DiscordClient::new("test-bot-token", discord_origin).unwrap(),
        site_base_url: "https://discalendar.app".into(),
        event_update_locks: Default::default(),
        auth: AuthConfig {
            secret: "test-secret".into(),
            cookie_names: vec!["test".into()],
        },
        activity_days: moka::future::Cache::new(10),
        external_feeds: moka::future::Cache::new(10),
        external_fetch_slots: tokio::sync::Semaphore::new(4),
        api_token_rate_limits: moka::future::Cache::new(1000),
        admin: AdminConfig::default(),
        started_at: chrono::Utc::now(),
    })
}

async fn seed_guilds(pool: &PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id, name, locale) VALUES ($1, 'メイン', 'ja'), ($2, '別サーバー', 'ja')")
        .bind(GUILD)
        .bind(OTHER_GUILD)
        .execute(pool)
        .await
        .unwrap();
}

fn event_payload(name: &str) -> Value {
    json!({
        "name": name,
        "description": "外部連携からの予定",
        "color": "#5865F2",
        "is_all_day": false,
        "start_at": "2026-12-01T21:00:00",
        "end_at": "2026-12-01T22:00:00",
        "notifications": [{ "num": 30, "unit": "minutes" }]
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn manage_api_tokens_requires_manage_permission_and_never_exposes_secrets(pool: PgPool) {
    seed_guilds(&pool).await;
    let (origin, handle) = start_mock_discord().await;
    let state = build_state(pool.clone(), &origin);

    // 1. 未認証 (401) と一般メンバー (403) は拒否
    for (actor, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(MEMBER_ID), StatusCode::FORBIDDEN),
    ] {
        let app = test::init_service(
            App::new()
                .into_utoipa_app()
                .app_data(state.clone())
                .configure(routes::configure)
                .into_app()
                .wrap_fn(move |req, srv| {
                    if let Some(id) = actor {
                        req.extensions_mut().insert(AuthUser {
                            id: format!("u{id}"),
                            name: "test".into(),
                            discord_user_id: id.into(),
                        });
                    }
                    srv.call(req)
                }),
        )
        .await;
        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/guilds/{GUILD}/api-tokens"))
                .set_json(json!({"name": "CI", "scope": "create", "expires_in_days": 30}))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), expected);
    }

    // 2. 管理者で発行・一覧・ローテーション・失効・削除
    let app = test::init_service(
        App::new()
            .into_utoipa_app()
            .app_data(state.clone())
            .configure(routes::configure)
            .into_app()
            .wrap_fn(|req, srv| {
                req.extensions_mut().insert(AuthUser {
                    id: format!("u{MANAGER_ID}"),
                    name: "manager".into(),
                    discord_user_id: MANAGER_ID.into(),
                });
                srv.call(req)
            }),
    )
    .await;

    let res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/guilds/{GUILD}/api-tokens"))
            .set_json(json!({"name": "GitHub Actions", "scope": "write", "expires_in_days": 90}))
            .to_request(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(res.headers().get("Cache-Control").unwrap(), "no-store");
    let issued: Value = test::read_body_json(res).await;
    let token_id = issued["id"].as_str().unwrap().to_owned();
    let raw_token = issued["token"].as_str().unwrap().to_owned();
    assert!(raw_token.starts_with("dct_"));
    assert_eq!(raw_token.len(), 68);

    // DB には平文が保存されずハッシュのみ保存される
    let stored_hash: String =
        sqlx::query_scalar("SELECT token_hash FROM guild_api_tokens WHERE id = $1")
            .bind(token_id.parse::<i64>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(stored_hash, raw_token);
    assert_eq!(stored_hash, api_tokens::hash_hex(raw_token.as_bytes()));

    // 一覧 API は平文もハッシュも返さない
    let list_res = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/guilds/{GUILD}/api-tokens"))
            .to_request(),
    )
    .await;
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_body = String::from_utf8(test::read_body(list_res).await.to_vec()).unwrap();
    assert!(!list_body.contains(&raw_token));
    assert!(!list_body.contains(&stored_hash));
    assert!(list_body.contains(issued["token_prefix"].as_str().unwrap()));

    // ローテーションすると新しいトークンが発行され、古いトークンは無効になる
    let rotate_res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/guilds/{GUILD}/api-tokens/{token_id}/rotate"))
            .to_request(),
    )
    .await;
    assert_eq!(rotate_res.status(), StatusCode::OK);
    let rotated: Value = test::read_body_json(rotate_res).await;
    let new_token = rotated["token"].as_str().unwrap().to_owned();
    assert_ne!(new_token, raw_token);
    assert!(
        api_tokens::authenticate(&pool, GUILD, &raw_token)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        api_tokens::authenticate(&pool, GUILD, &new_token)
            .await
            .unwrap()
            .is_some()
    );

    // 失効すると認証に失敗し、再ローテーションは 400
    let revoke_res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/guilds/{GUILD}/api-tokens/{token_id}/revoke"))
            .to_request(),
    )
    .await;
    assert_eq!(revoke_res.status(), StatusCode::NO_CONTENT);
    assert!(
        api_tokens::authenticate(&pool, GUILD, &new_token)
            .await
            .unwrap()
            .is_none()
    );
    let rotate_revoked = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/guilds/{GUILD}/api-tokens/{token_id}/rotate"))
            .to_request(),
    )
    .await;
    assert_eq!(rotate_revoked.status(), StatusCode::BAD_REQUEST);

    // 削除
    let delete_res = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/guilds/{GUILD}/api-tokens/{token_id}"))
            .to_request(),
    )
    .await;
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    handle.stop(true).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn hooks_crud_idempotency_external_id_history_and_rate_limits(pool: PgPool) {
    seed_guilds(&pool).await;
    sqlx::query("INSERT INTO guild_webhooks (guild_id, url, kind, secret, created_by) VALUES ($1, 'https://example.com/hook', 'json', 'sec', $2)")
        .bind(GUILD)
        .bind(MANAGER_ID)
        .execute(&pool)
        .await
        .unwrap();

    let (origin, handle) = start_mock_discord().await;
    let state = build_state(pool.clone(), &origin);
    let app = test::init_service(
        App::new()
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .app_data(state.clone())
            .configure(routes::configure)
            .openapi_service(|api| {
                utoipa_swagger_ui::SwaggerUi::new("/docs/{_:.*}").url("/openapi.json", api)
            })
            .into_app(),
    )
    .await;

    // OpenAPI に公開フックと管理トークンが含まれる
    let spec_res = test::call_service(
        &app,
        test::TestRequest::get().uri("/openapi.json").to_request(),
    )
    .await;
    assert_eq!(spec_res.status(), StatusCode::OK);
    let spec: Value = test::read_body_json(spec_res).await;
    assert!(spec["paths"]["/hooks/guilds/{guild_id}/events"]["post"].is_object());
    assert!(spec["paths"]["/guilds/{guild_id}/api-tokens"]["get"].is_object());
    assert!(spec["components"]["securitySchemes"]["api_token"].is_object());

    let create_only = api_tokens::create(
        &pool,
        GUILD,
        "作成専用",
        api_tokens::ApiTokenScope::Create,
        None,
        MANAGER_ID,
    )
    .await
    .unwrap();
    let writer = api_tokens::create(
        &pool,
        GUILD,
        "自動同期Bot",
        api_tokens::ApiTokenScope::Write,
        None,
        MANAGER_ID,
    )
    .await
    .unwrap();
    let expired_token = api_tokens::create(
        &pool,
        GUILD,
        "期限切れ",
        api_tokens::ApiTokenScope::Write,
        Some(chrono::Utc::now() - chrono::Duration::minutes(5)),
        MANAGER_ID,
    )
    .await
    .unwrap();

    // 他ギルド・期限切れ・不正トークンは 401
    for (guild, bearer) in [
        (OTHER_GUILD, writer.token.as_str()),
        (GUILD, expired_token.token.as_str()),
        (GUILD, "dct_invalid"),
    ] {
        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/hooks/guilds/{guild}/events"))
                .insert_header((AUTHORIZATION, format!("Bearer {bearer}")))
                .set_json(event_payload("拒否"))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    // バリデーション (タイトル上限・色・通知件数) は通常 API と同じ 400
    let invalid_res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/hooks/guilds/{GUILD}/events"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .set_json(event_payload(&"あ".repeat(33)))
            .to_request(),
    )
    .await;
    assert_eq!(invalid_res.status(), StatusCode::BAD_REQUEST);

    // create_only トークンで作成 + 冪等キー
    let mut create_body = event_payload("配信予定");
    create_body["external_id"] = json!("yt:live-1");
    let res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/hooks/guilds/{GUILD}/events"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .insert_header(("Idempotency-Key", "idem-create-1"))
            .set_json(&create_body)
            .to_request(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: Value = test::read_body_json(res).await;
    let event_id = created["id"].as_i64().unwrap() as i32;
    assert_eq!(created["external_id"], "yt:live-1");
    assert!(created["created_by"].is_null());

    // 同じ Idempotency-Key + 同じ本文なら重複作成せずリプレイを返す
    let replay_res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/hooks/guilds/{GUILD}/events"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .insert_header(("Idempotency-Key", "idem-create-1"))
            .set_json(&create_body)
            .to_request(),
    )
    .await;
    assert_eq!(replay_res.status(), StatusCode::CREATED);
    assert_eq!(
        replay_res.headers().get("Idempotency-Replayed").unwrap(),
        "true"
    );
    let replayed: Value = test::read_body_json(replay_res).await;
    assert_eq!(replayed["id"], created["id"]);

    // 同じ Idempotency-Key + 異なる本文なら 409
    let conflict_res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/hooks/guilds/{GUILD}/events"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .insert_header(("Idempotency-Key", "idem-create-1"))
            .set_json(event_payload("別タイトル"))
            .to_request(),
    )
    .await;
    assert_eq!(conflict_res.status(), StatusCode::CONFLICT);

    // 同じ external_id で POST すると 409
    let dup_ext_res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/hooks/guilds/{GUILD}/events"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .set_json(&create_body)
            .to_request(),
    )
    .await;
    assert_eq!(dup_ext_res.status(), StatusCode::CONFLICT);

    // create_only トークンで更新・削除は 403
    let forbidden_put = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/hooks/guilds/{GUILD}/events/{event_id}"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .set_json(event_payload("更新不可"))
            .to_request(),
    )
    .await;
    assert_eq!(forbidden_put.status(), StatusCode::FORBIDDEN);

    // write トークンで external_id 更新 (200) と新規作成 (Upsert -> 201)
    let upsert_update = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/hooks/guilds/{GUILD}/events/external/yt:live-1"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", writer.token)))
            .set_json(event_payload("配信予定（変更後）"))
            .to_request(),
    )
    .await;
    assert_eq!(upsert_update.status(), StatusCode::OK);
    let updated_json: Value = test::read_body_json(upsert_update).await;
    assert_eq!(updated_json["id"], event_id);
    assert_eq!(updated_json["name"], "配信予定（変更後）");

    let upsert_create = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/hooks/guilds/{GUILD}/events/external/yt:live-2"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", writer.token)))
            .set_json(event_payload("新規アップサート"))
            .to_request(),
    )
    .await;
    assert_eq!(upsert_create.status(), StatusCode::CREATED);
    let upserted: Value = test::read_body_json(upsert_create).await;
    let second_id = upserted["id"].as_i64().unwrap() as i32;

    // 変更履歴に source='api_token', actor_label=<トークン名> が残る
    let history = event_history::list(&pool, GUILD, event_id).await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].source, "api_token");
    assert_eq!(history[0].action, "update");
    assert!(history[0].actor_discord_user_id.is_none());
    assert_eq!(history[0].actor_label.as_deref(), Some("自動同期Bot"));
    assert_eq!(history[1].source, "api_token");
    assert_eq!(history[1].action, "create");
    assert_eq!(history[1].actor_label.as_deref(), Some("作成専用"));

    // Webhook outbox が予約されている
    let webhook_actors: Vec<String> =
        sqlx::query_scalar("SELECT actor_id FROM guild_webhook_outbox ORDER BY id ASC")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        webhook_actors,
        vec![
            format!("api_token:{}", create_only.id),
            format!("api_token:{}", writer.id),
            format!("api_token:{}", writer.id),
        ]
    );

    // external_id で削除後、同じ external_id で再度作成できる
    let del_ext = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/hooks/guilds/{GUILD}/events/external/yt:live-2"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", writer.token)))
            .to_request(),
    )
    .await;
    assert_eq!(del_ext.status(), StatusCode::NO_CONTENT);
    let upsert_recreate = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/hooks/guilds/{GUILD}/events/external/yt:live-2"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", writer.token)))
            .set_json(event_payload("再作成予定"))
            .to_request(),
    )
    .await;
    assert_eq!(upsert_recreate.status(), StatusCode::CREATED);
    let recreated: Value = test::read_body_json(upsert_recreate).await;
    assert_ne!(recreated["id"].as_i64().unwrap() as i32, second_id);

    // 24 時間経過した冪等キーは 409 になり、保持タスクで response_body が消える
    sqlx::query("UPDATE guild_api_token_operations SET created_at = now() - interval '25 hours'")
        .execute(&pool)
        .await
        .unwrap();
    let expired_idem = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/hooks/guilds/{GUILD}/events"))
            .insert_header((AUTHORIZATION, format!("Bearer {}", create_only.token)))
            .insert_header(("Idempotency-Key", "idem-create-1"))
            .set_json(&create_body)
            .to_request(),
    )
    .await;
    assert_eq!(expired_idem.status(), StatusCode::CONFLICT);
    assert!(api_tokens::purge_expired_operations(&pool).await.unwrap() >= 1);

    // レート制限 (トークン単位 60 回/分) とログ・last_used_at の記録
    let tokens = api_tokens::list_with_logs(&pool, GUILD).await.unwrap();
    let writer_info = tokens.iter().find(|t| t.id == writer.id).unwrap();
    assert!(writer_info.last_used_at.is_some());
    assert!(!writer_info.logs.is_empty());

    // 残りの枠を使い切って 429 になることを確認
    let mut hit_429 = false;
    for _ in 0..=RATE_LIMIT_PER_MINUTE {
        let res = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/hooks/guilds/{GUILD}/events/{event_id}"))
                .insert_header((AUTHORIZATION, format!("Bearer {}", writer.token)))
                .set_json(event_payload("頻度制限確認"))
                .to_request(),
        )
        .await;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            hit_429 = true;
            break;
        }
    }
    assert!(hit_429);

    // 認証失敗の頻度制限 (30 回/分)
    let mut auth_hit_429 = false;
    for _ in 0..=AUTH_FAILURE_LIMIT_PER_MINUTE {
        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/hooks/guilds/{GUILD}/events"))
                .insert_header((AUTHORIZATION, "Bearer dct_wrong"))
                .insert_header(("cf-connecting-ip", "203.0.113.10"))
                .set_json(event_payload("認証失敗"))
                .to_request(),
        )
        .await;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            auth_hit_429 = true;
            break;
        }
    }
    assert!(auth_hit_429);

    handle.stop(true).await;
}
