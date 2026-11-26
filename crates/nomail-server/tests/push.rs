use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
use chrono::Utc;
use nomail_app::push::{self, Finish, Outbox, RelayConfig, Transport};
use nomail_server::{AppState, app, config::Config, db, push as native, realtime::Hub};
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use std::{net::TcpListener, sync::Mutex};

mod common;

const REGISTRATION: &str = "/api/device/push/token";
const PREFERENCES: &str = "/api/device/push/preferences";
const DELIVERIES: &str = "/v1/caller/deliveries";

#[actix_web::test]
async fn push_schema_is_identical_on_both_runtimes() {
    assert_eq!(
        include_str!("../migrations/005_push.sql"),
        include_str!("../../nomail-workers/migrations/0005_push.sql")
    );
}

#[actix_web::test]
async fn commits_once_and_retries_same_command_after_losing_relay_ack() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: directory.path().join("push.db").to_str().unwrap().into(),
        jwt_secret: "push-test-jwt".into(),
        jwt_issuer: "test".into(),
        encryption_key: [7; 32],
        port: 3000,
    };
    let db = db::connect(&config).await.unwrap();
    common::initialize(&db, &config).await.unwrap();
    let api = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState {
                db: db.clone(),
                config: config.clone(),
                realtime: Hub::default(),
            }))
            .app_data(native::Enabled)
            .configure(app::configure),
    )
    .await;
    let login: Value = test::call_and_read_body_json(&api, test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({"username":"admin","password":"password123","deviceId":"iphone","platform":"ios","deviceType":"phone"}))
        .to_request()).await;
    let auth = (
        "Authorization",
        format!("Bearer {}", login["data"]["accessToken"].as_str().unwrap()),
    );
    let registration = |channel: &str, token: &str| {
        test::TestRequest::put()
            .uri(REGISTRATION)
            .insert_header(auth.clone())
            .set_json(json!({"channel":channel,"deviceToken":token}))
            .to_request()
    };
    assert_eq!(
        test::call_service(&api, registration("apns", "abc"))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        test::call_service(&api, registration("apns", "AB12"))
            .await
            .status(),
        StatusCode::OK
    );
    let second_login: Value = test::call_and_read_body_json(&api, test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({"username":"admin","password":"password123","deviceId":"second-phone","platform":"ios","deviceType":"phone"}))
        .to_request()).await;
    let second_auth = (
        "Authorization",
        format!(
            "Bearer {}",
            second_login["data"]["accessToken"].as_str().unwrap()
        ),
    );
    assert_eq!(
        test::call_service(
            &api,
            test::TestRequest::put()
                .uri(REGISTRATION)
                .insert_header(second_auth.clone())
                .set_json(json!({"channel":"apns","deviceToken":"cd34"}))
                .to_request()
        )
        .await
        .status(),
        StatusCode::OK
    );
    let source: Value = test::call_and_read_body_json(&api, test::TestRequest::post()
        .uri("/api/webhook/configs").insert_header(auth.clone())
        .set_json(json!({"name":"source","platform":{"provider":"generic","credentials":{"type":"none"}}}))
        .to_request()).await;
    let webhook = source["data"]["url"].as_str().unwrap();
    let receive =
        |id: &str| {
            test::TestRequest::post().uri(webhook)
        .set_json(json!({"id":id,"title":"private webhook title","content":"private webhook body"}))
        .to_request()
        };
    let preferences = |normal| {
        test::TestRequest::put()
            .uri(PREFERENCES)
            .insert_header(auth.clone())
            .set_json(json!({"enabled":true,"low":true,"normal":normal,"high":true,"urgent":true}))
            .to_request()
    };
    let outbox = native::Store(&db);
    assert_eq!(
        test::call_service(&api, preferences(false)).await.status(),
        StatusCode::OK
    );
    let muted: Value = test::call_and_read_body_json(&api, receive("muted-event")).await;
    assert_eq!(muted["data"]["duplicate"], false);
    let now = Utc::now().timestamp();
    assert!(outbox.claim(now, now + 60).await.unwrap().is_none());
    assert_eq!(
        test::call_service(&api, preferences(true)).await.status(),
        StatusCode::OK
    );

    db.execute_unprepared("CREATE TRIGGER fail_push BEFORE INSERT ON push_jobs BEGIN SELECT RAISE(ABORT, 'failed test enqueue'); END;")
        .await.unwrap();
    assert_eq!(
        test::call_service(&api, receive("event-1")).await.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    db.execute_unprepared("DROP TRIGGER fail_push")
        .await
        .unwrap();
    let first: Value = test::call_and_read_body_json(&api, receive("event-1")).await;
    let second_settings: Value = test::call_and_read_body_json(
        &api,
        test::TestRequest::get()
            .uri(PREFERENCES)
            .insert_header(second_auth)
            .to_request(),
    )
    .await;
    assert_eq!(
        second_settings["data"],
        json!({"enabled":false,"low":true,"normal":true,"high":true,"urgent":true})
    );
    let duplicate: Value = test::call_and_read_body_json(&api, receive("event-1")).await;
    let message_id = first["data"]["messageId"].clone();
    assert_eq!(first["data"]["duplicate"], false);
    assert_eq!(duplicate["data"]["duplicate"], true);
    assert_eq!(duplicate["data"]["messageId"], message_id);
    assert_eq!(
        test::call_service(&api, preferences(false)).await.status(),
        StatusCode::OK
    );
    let now = Utc::now().timestamp();
    assert!(outbox.claim(now, now + 60).await.unwrap().is_none());
    assert_eq!(
        test::call_service(&api, preferences(true)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        test::call_service(&api, registration("apns", "ab12"))
            .await
            .status(),
        StatusCode::OK
    );

    // Reopen the database after the receiver has dropped, before dispatching.
    drop(api);
    db.close().await.unwrap();
    let db = db::connect(&config).await.unwrap();
    let outbox = native::Store(&db);
    let now = Utc::now().timestamp();
    let job = outbox.claim(now, now + 60).await.unwrap().unwrap();
    assert!(outbox.claim(now, now + 60).await.unwrap().is_none());
    let delivery_id = job.delivery_id.to_string();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = RelayConfig::for_local_test(listener.local_addr().unwrap(), "a".repeat(64));
    let requests = web::Data::new(Mutex::new(Vec::<Value>::new()));
    let observed = requests.clone();
    let server = HttpServer::new(move || App::new().app_data(observed.clone()).route(
        DELIVERIES, web::post().to(|req: HttpRequest, command: web::Json<Value>, requests: web::Data<Mutex<Vec<Value>>>| async move {
            assert_eq!(req.headers().get("Authorization").unwrap().to_str().unwrap(), format!("Bearer {}", "a".repeat(64)));
            let mut requests = requests.lock().unwrap();
            requests.push(command.into_inner());
            if requests.len() == 1 {
                // The relay has kept the command, but the caller gets a proxy error.
                HttpResponse::BadGateway().insert_header(("Retry-After", "8")).finish()
            } else {
                HttpResponse::Ok().json(json!({"code":0,"data":{"deliveryId":requests[0]["deliveryId"],"status":"queued"}}))
            }
        }),
    ).route("/v1/caller/deliveries/{id}", web::get().to(|req: HttpRequest| async move {
        assert_eq!(req.headers().get("Authorization").unwrap().to_str().unwrap(), format!("Bearer {}", "a".repeat(64)));
        HttpResponse::Ok().json(json!({"code":0,"data":{
            "deliveryId":req.match_info().get("id").unwrap(),"status":"failed",
            "lastError":{"code":"invalid_target"}
        }}))
    }))).workers(1).listen(listener).unwrap().run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    let client = native::HttpClient::new(config).unwrap();
    push::drain(&outbox, &client, || now + 60, 2).await.unwrap();
    push::drain(&outbox, &client, || now + 67, 2).await.unwrap();
    assert_eq!(requests.lock().unwrap().len(), 1);
    push::drain(&outbox, &client, || now + 90, 2).await.unwrap();
    let poll = outbox.claim(now + 121, now + 181).await.unwrap().unwrap();
    assert!(poll.accepted);
    assert_eq!(poll.delivery_id, job.delivery_id);
    outbox.finish(&job, Finish::InvalidTarget).await.unwrap(); // stale lease
    {
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0], requests[1]);
        assert_eq!(
            requests[0],
            json!({
                "deliveryId":delivery_id, "messageId":message_id,
                "notification":{"title":"NoMail: private webhook title","body":"private webhook body"},
                "target":{"channel":"apns","token":"ab12"}
            })
        );
    }
    let target_sql = || {
        sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT token FROM push_targets WHERE device_id=1",
        )
    };
    assert!(db.query_one(target_sql()).await.unwrap().is_some());
    // A token refresh racing an old in-flight check must not be invalidated.
    db.execute_unprepared("UPDATE push_targets SET token='ab34', version='new' WHERE device_id=1")
        .await
        .unwrap();
    assert_eq!(client.request(poll.request()).await.unwrap().status, 200);
    outbox.finish(&poll, Finish::InvalidTarget).await.unwrap();
    let target: String = db
        .query_one(target_sql())
        .await
        .unwrap()
        .unwrap()
        .try_get("", "token")
        .unwrap();
    assert_eq!(target, "ab34");
    handle.stop(false).await;
}
