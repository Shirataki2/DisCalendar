//! 通知言語の変更を実 API の認可・DB 更新・投票同期で検証する。
use actix_web::{
    App, HttpMessage, HttpResponse, HttpServer, dev::Service, http::StatusCode, test, web,
};
use discalendar_api::{
    auth::{AuthConfig, AuthUser},
    discord::DiscordClient,
    models::guilds,
    routes,
    state::{AdminConfig, AppState},
};
use sqlx::PgPool;
use utoipa_actix_web::AppExt;

#[sqlx::test(migrations = "./migrations")]
async fn server_locale_requires_membership_and_management_permissions(pool: PgPool) {
    sqlx::query("INSERT INTO guilds(guild_id,name) VALUES ('111','日本語の名前')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO push_subscriptions(user_id,endpoint,p256dh,auth,device_name) VALUES ('u3','https://fcm.googleapis.com/test','test','test','device')").execute(&pool).await.unwrap();
    let poll: i32 = sqlx::query_scalar("INSERT INTO schedule_polls(guild_id,title,created_by,created_at) VALUES ('111','日本語の予定','3',now()) RETURNING id").fetch_one(&pool).await.unwrap();
    let revision: i64 =
        sqlx::query_scalar("SELECT discord_revision FROM schedule_polls WHERE id=$1")
            .bind(poll)
            .fetch_one(&pool)
            .await
            .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(|| App::new()
        .route("/guilds/111", web::get().to(|| async { HttpResponse::Ok().json(serde_json::json!({"id":"111","name":"server","icon":null,"owner_id":"9","roles":[{"id":"111","permissions":"0","name":"everyone","color":0,"position":0,"managed":false},{"id":"8","permissions":"32","name":"manager","color":0,"position":1,"managed":false}]})) }))
        .route("/guilds/111/members/{id}", web::get().to(|id: web::Path<String>| async move { match id.as_str() { "2" => HttpResponse::Ok().json(serde_json::json!({"roles":[]})), "3" => HttpResponse::Ok().json(serde_json::json!({"roles":["8"]})), _ => HttpResponse::NotFound().finish() } })))
        .listen(listener).unwrap().run();
    let handle = server.handle();
    tokio::spawn(server);
    let state = web::Data::new(AppState {
        pool: pool.clone(),
        sql_console_pool: pool.clone(),
        sql_known_words: Default::default(),
        discord: DiscordClient::new("test", &format!("http://{address}")).unwrap(),
        site_base_url: "https://example.com".into(),
        event_update_locks: Default::default(),
        auth: AuthConfig {
            secret: "test-secret".into(),
            cookie_names: vec!["test".into()],
        },
        activity_days: moka::future::Cache::new(10),
        external_feeds: moka::future::Cache::new(10),
        external_fetch_slots: tokio::sync::Semaphore::new(4),
        admin: AdminConfig::default(),
        started_at: chrono::Utc::now(),
    });
    // セッション検証自体は auth の既存テストに任せ、認証後の本人情報をリクエストに注入する。
    for (actor, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("2"), StatusCode::FORBIDDEN),
        (Some("4"), StatusCode::FORBIDDEN),
        (Some("3"), StatusCode::OK),
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
        let response = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/users/@me/push-subscriptions")
                .set_json(
                    serde_json::json!({"endpoint":"https://fcm.googleapis.com/test","locale":"en"}),
                )
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            if actor.is_none() {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::NO_CONTENT
            }
        );
        let device_locale: String =
            sqlx::query_scalar("SELECT locale FROM push_subscriptions WHERE user_id='u3'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            device_locale,
            if actor == Some("3") { "en" } else { "ja" },
            "端末言語の API は本人だけが更新できる"
        );
        let request = test::TestRequest::put()
            .uri("/guilds/111/config")
            .set_json(serde_json::json!({"restricted":false,"locale":"en"}))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), expected, "actor={actor:?}");
        let stored = guilds::get_config(&pool, "111").await.unwrap();
        assert_eq!(
            stored.locale,
            if expected == StatusCode::OK {
                "en"
            } else {
                "ja"
            }
        );
        if expected == StatusCode::OK {
            let json: serde_json::Value = test::read_body_json(response).await;
            assert_eq!(json["locale"], "en");
            let response = test::call_service(
                &app,
                test::TestRequest::put()
                    .uri("/guilds/111/config")
                    .set_json(serde_json::json!({"restricted":true}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                guilds::get_config(&pool, "111").await.unwrap().locale,
                "en",
                "省略した言語は維持する"
            );
            let response = test::call_service(
                &app,
                test::TestRequest::put()
                    .uri("/guilds/111/config")
                    .set_json(serde_json::json!({"restricted":true,"locale":"fr"}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }
    let updated: i64 =
        sqlx::query_scalar("SELECT discord_revision FROM schedule_polls WHERE id=$1")
            .bind(poll)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        updated,
        revision + 1,
        "言語変更だけでも既存投票投稿が再同期される"
    );
    let title: String = sqlx::query_scalar("SELECT title FROM schedule_polls WHERE id=$1")
        .bind(poll)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, "日本語の予定", "利用者の入力は翻訳しない");
    sqlx::raw_sql(include_str!(
        "../rollback/20261004000000_notification_locales.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE guilds SET locale='ja' WHERE guild_id='111'")
        .execute(&pool)
        .await
        .unwrap();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_name='push_subscriptions' AND column_name='locale')").fetch_one(&pool).await.unwrap();
    assert!(!exists, "戻し方は追加カラムとトリガーだけを取り除く");
    handle.stop(true).await;
}
