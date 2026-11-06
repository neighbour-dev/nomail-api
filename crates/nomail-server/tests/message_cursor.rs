use actix_web::{App, http::StatusCode, test, web};
use chrono::{DateTime, Duration, Utc};
use nomail_server::{AppState, app, auth, config::Config, db, entity::message, realtime::Hub};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};
use uuid::Uuid;

#[actix_web::test]
async fn cursor_queries_preserve_scope_filters_order_and_deleted_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: dir.path().join("cursor.db").to_str().unwrap().into(),
        jwt_secret: "local-cursor-test-secret".into(),
        jwt_issuer: "test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    };
    let database = db::connect(&config).await.unwrap();
    auth::ensure_initial_user(&database, &config).await.unwrap();
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
    });
    let app = test::init_service(App::new().app_data(state).configure(app::configure)).await;
    macro_rules! api {
        ($method:ident,$path:expr,$body:expr,$token:expr) => {{
            let token: Option<&str> = $token;
            let mut request = test::TestRequest::$method().uri($path);
            if let Some(token) = token {
                request = request.insert_header(("Authorization", format!("Bearer {token}")));
            }
            let response = test::call_service(&app, request.set_json($body).to_request()).await;
            let status = response.status();
            let body: Value = test::read_body_json(response).await;
            (status, body)
        }};
    }
    let (status, _) = api!(get, "/api/messages/cursor", json!({}), None);
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, login) = api!(
        post,
        "/api/auth/login",
        json!({"username":"admin","password":"password123","deviceId":"cursor","platform":"ios","deviceType":"phone"}),
        None
    );
    let token = login["data"]["accessToken"].as_str().unwrap();
    let (_, config) = api!(
        post,
        "/api/webhook/configs",
        json!({"name":"Cursor","platform":{"provider":"generic","credentials":{"type":"none"}}}),
        Some(token)
    );
    let config_id = config["data"]["id"].as_i64().unwrap();
    let base = format!("/api/messages/cursor?webhookConfigId={config_id}");
    let (_, empty) = api!(get, &base, json!({}), Some(token));
    assert_eq!(
        empty["data"],
        json!({"items":[],"hasMore":false,"nextCursor":null,"startCursor":null,"endCursor":null})
    );
    let (_, created) = api!(
        post,
        config["data"]["url"].as_str().unwrap(),
        json!({"title":"Cursor","content":"hello\n world","level":"high"}),
        None
    );
    let template = message::Entity::find_by_id(
        Uuid::parse_str(created["data"]["messageId"].as_str().unwrap()).unwrap(),
    )
    .one(&database)
    .await
    .unwrap()
    .unwrap();
    message::Entity::delete_by_id(template.id)
        .exec(&database)
        .await
        .unwrap();
    let time: DateTime<Utc> = "2040-01-01T00:00:00.123456789Z".parse().unwrap();
    let ids: Vec<_> = (1..=9).map(Uuid::from_u128).collect();
    for (index, id) in ids.iter().enumerate() {
        let mut row: message::ActiveModel = template.clone().into();
        row.id = Set(*id);
        row.created_at = Set(if index == 0 {
            time - Duration::seconds(1)
        } else if index == 8 {
            time + Duration::seconds(1)
        } else {
            time
        });
        row.insert(&database).await.unwrap();
    }
    // A foreign config/message and a deleted config/message must remain invisible,
    // including when a caller supplies that config's ID as an additional filter.
    use nomail_server::entity::webhook;
    let own_config = webhook::Entity::find_by_id(config_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    for (index, deleted) in [false, true].into_iter().enumerate() {
        let mut config: webhook::ActiveModel = own_config.clone().into();
        let foreign_id = config_id + 10 + index as i64;
        config.id = Set(foreign_id);
        config.key = Set(format!("cursor-hidden-{index}"));
        config.user_id = Set(if deleted {
            own_config.user_id
        } else {
            own_config.user_id + 100
        });
        config.deleted_at = Set(deleted.then_some(time));
        config.insert(&database).await.unwrap();
        let mut row: message::ActiveModel = template.clone().into();
        row.id = Set(Uuid::from_u128(100 + index as u128));
        row.webhook_config_id = Set(foreign_id);
        row.insert(&database).await.unwrap();
        let (_, hidden) = api!(
            get,
            &format!("/api/messages/cursor?webhookConfigId={foreign_id}"),
            json!({}),
            Some(token)
        );
        assert_eq!(hidden["data"]["items"], json!([]));
    }
    let (_, all) = api!(get, "/api/messages/cursor", json!({}), Some(token));
    assert_eq!(all["data"]["items"].as_array().unwrap().len(), 9);
    assert!(all["data"].get("total").is_none());
    assert!(all["data"].get("page").is_none());
    let mut path = format!("{base}&limit=3");
    let mut history = Vec::new();
    let mut pages = Vec::new();
    loop {
        let (status, response) = api!(get, &path, json!({}), Some(token));
        assert_eq!(status, StatusCode::OK);
        let page = response["data"].clone();
        for item in page["items"].as_array().unwrap() {
            assert_eq!(item["summary"], "hello world");
            history.push(item["id"].as_str().unwrap().to_owned());
        }
        pages.push(page.clone());
        if page["hasMore"] == false {
            assert!(page["nextCursor"].is_null());
            break;
        }
        assert_eq!(page["nextCursor"], page["endCursor"]);
        path = format!(
            "{base}&limit=3&before={}",
            page["nextCursor"].as_str().unwrap()
        );
    }
    assert_eq!(
        history,
        ids.iter()
            .rev()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    // Reuse a historical page boundary to fetch all newer items in ascending batches.
    let anchor = pages[2]["startCursor"].as_str().unwrap(); // ID 3
    path = format!("{base}&limit=2&after={anchor}");
    let mut added = Vec::new();
    loop {
        let (_, response) = api!(get, &path, json!({}), Some(token));
        let page = &response["data"];
        added.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_owned()),
        );
        if page["hasMore"] == false {
            break;
        }
        path = format!(
            "{base}&limit=2&after={}",
            page["nextCursor"].as_str().unwrap()
        );
    }
    assert_eq!(
        added,
        ids[3..].iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    let latest = pages[0]["startCursor"].as_str().unwrap();
    let (_, empty) = api!(
        get,
        &format!("{base}&after={latest}"),
        json!({}),
        Some(token)
    );
    assert_eq!(empty["data"]["items"], json!([]));
    assert!(empty["data"]["startCursor"].is_null());
    // Deleting the exact anchor does not invalidate the independently encoded position.
    let (_, _) = api!(
        delete,
        &format!("/api/messages/{}", ids[6]),
        json!({}),
        Some(token)
    );
    let boundary = pages[0]["endCursor"].as_str().unwrap();
    let (_, older) = api!(
        get,
        &format!("{base}&before={boundary}&limit=100"),
        json!({}),
        Some(token)
    );
    assert_eq!(older["data"]["items"].as_array().unwrap().len(), 6);
    let (_, newer) = api!(
        get,
        &format!("{base}&after={boundary}&limit=100"),
        json!({}),
        Some(token)
    );
    assert_eq!(newer["data"]["items"].as_array().unwrap().len(), 2);
    for suffix in [
        "status=read",
        "level=low",
        "pinned=true",
        "query=missing",
        "from=2041-01-01T00:00:00Z",
        "to=2039-01-01T00:00:00Z",
    ] {
        let (_, response) = api!(get, &format!("{base}&{suffix}"), json!({}), Some(token));
        assert_eq!(response["data"]["items"], json!([]), "{suffix}");
    }
    let (_, filtered) = api!(
        get,
        &format!(
            "{base}&query=world&status=unread&level=HIGH&pinned=false&from=2040-01-01T00:00:00Z&to=2040-01-01T00:00:01Z"
        ),
        json!({}),
        Some(token)
    );
    assert_eq!(filtered["data"]["items"].as_array().unwrap().len(), 6);
    for suffix in [
        "before=x&after=y",
        "before=",
        "after=bad",
        "limit=0",
        "limit=101",
        "limit=no",
        "from=bad",
        "status=no",
        "level=no",
        "pinned=no",
        "webhookConfigId=0",
        "from=2041-01-01T00:00:00Z&to=2040-01-01T00:00:00Z",
    ] {
        let (status, _) = api!(
            get,
            &format!("/api/messages/cursor?{suffix}"),
            json!({}),
            Some(token)
        );
        assert_eq!(status, StatusCode::BAD_REQUEST, "{suffix}");
    }
    // Offset pagination retains its existing response contract.
    let (_, page) = api!(
        get,
        "/api/messages?page=1&pageSize=2",
        json!({}),
        Some(token)
    );
    assert_eq!(page["data"]["pageSize"], 2);
    assert_eq!(page["data"]["total"], 8);
}
