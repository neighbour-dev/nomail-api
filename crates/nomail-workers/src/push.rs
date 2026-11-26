//! D1 and Fetch adapters for the push outbox.
use crate::{db, time};
use chrono::{DateTime, Utc};
use futures_util::{
    StreamExt,
    future::{Either, select},
};
use nomail_app::{
    auth::{AuthContext, AuthError, AuthResult},
    message::Message,
    push::{
        self, DeviceStore, Finish, Job, Outbox, Registration, RelayConfig, RelayRequest, Reply,
        StoreError, Transport,
    },
};
use serde_json::json;
use worker::{
    AbortController, D1PreparedStatement, Delay, Env, Fetch, Headers, Method, Request, RequestInit,
    RequestRedirect,
};

#[path = "../../push-sql/queries.rs"]
mod sql;

pub fn config(env: &Env) -> Result<Option<RelayConfig>, &'static str> {
    RelayConfig::from_token(
        env.secret(push::TOKEN_SETTING)
            .map(|v| v.to_string())
            .unwrap_or_default(),
    )
}

pub(crate) fn enqueue(
    db: &db::Store,
    m: &Message,
) -> Result<D1PreparedStatement, db::StorageError> {
    let now = m.received_at.timestamp();
    db.statement(
        sql::ENQUEUE,
        vec![
            json!(m.id),
            json!(push::TITLE),
            json!(push::BODY),
            json!(now),
            json!(now + push::SUBMIT_WINDOW),
        ],
    )
}

impl DeviceStore for crate::auth::repository::Store<'_> {
    async fn set_target(
        &self,
        actor: &AuthContext,
        target: Option<&Registration>,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
        let mut values = vec![
            json!(actor.device_id),
            json!(actor.user_id),
            json!(actor.session_id),
            json!(time::format(actor.logged_in_at.timestamp_millis())),
            json!(now.timestamp()),
        ];
        let err = |_| AuthError::Internal("push registration storage failed".into());
        if let Some(target) = target {
            values.extend([json!(target.token), json!(target.channel.as_str())]);
            if self
                .0
                .rows(sql::SET_TARGET, values)
                .await
                .map_err(err)?
                .is_empty()
            {
                return Err(AuthError::Unauthorized);
            }
        } else {
            self.0.run(sql::DROP_TARGET, values).await.map_err(err)?;
        }
        Ok(())
    }
}

impl Outbox for db::Store {
    async fn claim(&self, now: i64, lease_until: i64) -> Result<Option<Job>, StoreError> {
        self.run(sql::EXPIRE, vec![json!(now)])
            .await
            .map_err(|_| StoreError)?;
        self.rows(sql::CLAIM, vec![json!(now), json!(lease_until)])
            .await
            .map_err(|_| StoreError)?
            .into_iter()
            .next()
            .map(|mut row| {
                row["accepted"] = json!(row["accepted"].as_i64() == Some(1));
                serde_json::from_value(row).map_err(|_| StoreError)
            })
            .transpose()
    }
    async fn finish(&self, job: &Job, finish: Finish) -> Result<(), StoreError> {
        let mut id = vec![
            json!(job.delivery_id.simple().to_string()),
            json!(job.lease),
        ];
        let db_err = |_| StoreError;
        match finish {
            Finish::Remove => {
                self.run(sql::REMOVE, id).await.map_err(db_err)?;
            }
            Finish::InvalidTarget => {
                let invalidate = self
                    .statement(sql::INVALIDATE, id.clone())
                    .map_err(db_err)?;
                let remove = self.statement(sql::REMOVE, id).map_err(db_err)?;
                self.0
                    .batch(vec![invalidate, remove])
                    .await
                    .map_err(|_| StoreError)?;
            }
            Finish::Reschedule {
                accepted,
                next_at,
                accepted_deadline,
            } => {
                id.extend([json!(accepted), json!(next_at), json!(accepted_deadline)]);
                self.run(sql::RESCHEDULE, id).await.map_err(db_err)?;
            }
        }
        Ok(())
    }
}

struct HttpClient(RelayConfig);
impl Transport for HttpClient {
    async fn request(&self, request: RelayRequest<'_>) -> Option<Reply> {
        let url = self.0.url(&request);
        let headers = Headers::new();
        headers
            .set("Authorization", &format!("Bearer {}", self.0.token()))
            .ok()?;
        let mut init = RequestInit::new();
        init.with_redirect(RequestRedirect::Manual);
        match request {
            RelayRequest::Submit(body) => {
                headers.set("Content-Type", "application/json").ok()?;
                init.with_method(Method::Post)
                    .with_body(Some(serde_json::to_string(&body).ok()?.into()));
            }
            RelayRequest::Status(_) => {
                init.with_method(Method::Get);
            }
        }
        init.with_headers(headers);
        let request = Request::new_with_init(url.as_ref(), &init).ok()?;
        let controller = AbortController::default();
        let signal = controller.signal();
        let perform = async {
            let mut response = Fetch::Request(request)
                .send_with_signal(&signal)
                .await
                .ok()?;
            let status = response.status_code();
            let retry_after = response.headers().get("Retry-After").ok().flatten();
            let mut body = Vec::new();
            if let Ok(mut stream) = response.stream() {
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.ok()?;
                    if body.len() + chunk.len() > push::MAX_REPLY_BYTES {
                        return None;
                    }
                    body.extend_from_slice(&chunk);
                }
            }
            Some(Reply {
                status,
                retry_after,
                body,
            })
        };
        let timeout = Delay::from(std::time::Duration::from_secs(push::TIMEOUT_SECONDS));
        futures_util::pin_mut!(perform, timeout);
        match select(perform, timeout).await {
            Either::Left((Some(reply), _)) => Some(reply),
            _ => {
                controller.abort();
                None
            }
        }
    }
}

pub async fn run(env: Env, config: RelayConfig, limit: usize) {
    let Ok(db) = env.d1("DB").map(db::Store) else {
        worker::console_error!("push outbox database is unavailable");
        return;
    };
    match push::drain(&db, &HttpClient(config), || time::now_ms() / 1000, limit).await {
        Ok(report) => {
            if report.rejected > 0 {
                worker::console_warn!("{} push deliveries failed permanently", report.rejected);
            }
            if report.authorization_blocked > 0 {
                worker::console_warn!("push relay rejected instance credentials or entitlement");
            }
        }
        Err(_) => worker::console_error!("push outbox processing failed"),
    }
}
