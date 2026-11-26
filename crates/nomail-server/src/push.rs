//! Native SQLite and HTTPS adapters for the push outbox.
use chrono::Utc;
use nomail_app::{
    message::Message,
    push::{self, Finish, Job, Outbox, RelayConfig, RelayRequest, Reply, StoreError, Transport},
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, Statement,
    TransactionTrait, Value,
};

#[path = "../../push-sql/queries.rs"]
pub(crate) mod sql;

pub(crate) fn statement(sql: &str, values: impl IntoIterator<Item = Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Sqlite, sql, values)
}

pub(crate) async fn enqueue(tx: &DatabaseTransaction, m: &Message) -> Result<(), DbErr> {
    let now = m.received_at.timestamp();
    tx.execute(statement(
        sql::ENQUEUE,
        [
            m.id.into(),
            push::notification_title(&m.common).into(),
            m.common.summary.as_str().into(),
            now.into(),
            (now + push::SUBMIT_WINDOW).into(),
        ],
    ))
    .await?;
    Ok(())
}

pub struct Store<'a>(pub &'a DatabaseConnection);

impl Outbox for Store<'_> {
    async fn claim(&self, now: i64, lease_until: i64) -> Result<Option<Job>, StoreError> {
        self.0
            .execute(statement(sql::EXPIRE, [now.into()]))
            .await
            .map_err(|_| StoreError)?;
        let row = self
            .0
            .query_one(statement(sql::CLAIM, [now.into(), lease_until.into()]))
            .await
            .map_err(|_| StoreError)?;
        row.map(|row| {
            let delivery_id: String = row.try_get("", "delivery_id").map_err(|_| StoreError)?;
            let message_id: String = row.try_get("", "message_id").map_err(|_| StoreError)?;
            let channel: String = row.try_get("", "channel").map_err(|_| StoreError)?;
            Ok(Job {
                delivery_id: delivery_id.parse().map_err(|_| StoreError)?,
                message_id: message_id.parse().map_err(|_| StoreError)?,
                channel: channel.parse()?,
                token: row.try_get("", "token").map_err(|_| StoreError)?,
                title: row.try_get("", "title").map_err(|_| StoreError)?,
                body: row.try_get("", "body").map_err(|_| StoreError)?,
                accepted: row.try_get("", "accepted").map_err(|_| StoreError)?,
                attempt: row.try_get("", "attempt").map_err(|_| StoreError)?,
                lease: row.try_get("", "lease").map_err(|_| StoreError)?,
            })
        })
        .transpose()
    }
    async fn finish(&self, job: &Job, finish: Finish) -> Result<(), StoreError> {
        let id = [
            job.delivery_id.simple().to_string().into(),
            job.lease.into(),
        ];
        match finish {
            Finish::Remove => {
                self.0
                    .execute(statement(sql::REMOVE, id))
                    .await
                    .map_err(|_| StoreError)?;
            }
            Finish::InvalidTarget => {
                let tx = self.0.begin().await.map_err(|_| StoreError)?;
                tx.execute(statement(sql::INVALIDATE, id.clone()))
                    .await
                    .map_err(|_| StoreError)?;
                tx.execute(statement(sql::REMOVE, id))
                    .await
                    .map_err(|_| StoreError)?;
                tx.commit().await.map_err(|_| StoreError)?;
            }
            Finish::Reschedule {
                accepted,
                next_at,
                accepted_deadline,
            } => {
                self.0
                    .execute(statement(
                        sql::RESCHEDULE,
                        id.into_iter().chain([
                            accepted.into(),
                            next_at.into(),
                            accepted_deadline.into(),
                        ]),
                    ))
                    .await
                    .map_err(|_| StoreError)?;
            }
        }
        Ok(())
    }
}

pub struct HttpClient {
    config: RelayConfig,
    client: reqwest::Client,
}
pub struct Enabled;
impl HttpClient {
    pub fn new(config: RelayConfig) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(push::TIMEOUT_SECONDS))
            .build()?;
        Ok(Self { config, client })
    }
}
impl Transport for HttpClient {
    async fn request(&self, request: RelayRequest<'_>) -> Option<Reply> {
        let url = self.config.url(&request);
        let builder = match request {
            RelayRequest::Submit(body) => self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&body).ok()?),
            RelayRequest::Status(_) => self.client.get(url),
        };
        let mut response = builder.bearer_auth(self.config.token()).send().await.ok()?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if body.len() + chunk.len() > push::MAX_REPLY_BYTES {
                return None;
            }
            body.extend_from_slice(&chunk);
        }
        Some(Reply {
            status,
            retry_after,
            body,
        })
    }
}

pub async fn run(db: DatabaseConnection, client: HttpClient) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match push::drain(&Store(&db), &client, || Utc::now().timestamp(), 16).await {
            Ok(report) => {
                if report.rejected > 0 {
                    tracing::warn!(
                        count = report.rejected,
                        "push deliveries failed permanently"
                    );
                }
                if report.authorization_blocked > 0 {
                    tracing::warn!(
                        count = report.authorization_blocked,
                        "push relay rejected instance credentials or entitlement"
                    );
                }
            }
            Err(_) => tracing::warn!("push outbox processing failed"),
        }
    }
}
