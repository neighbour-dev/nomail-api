use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "notify_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
    pub webhook_config_id: i64,
    pub provider: String,
    pub event_type: String,
    pub event_version: Option<String>,
    pub deduplication_scope: Option<String>,
    pub deduplication_key: Option<String>,
    pub raw_headers: String,
    pub title: String,
    pub summary: String,
    pub level: String,
    pub status: String,
    pub url: Option<String>,
    pub raw_payload: String,
    pub content_type: String,
    pub pinned: bool,
    pub expire_at: Option<DateTimeUtc>,
    pub read_at: Option<DateTimeUtc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl TryFrom<&Model> for nomail_app::message::Message {
    type Error = serde_json::Error;
    fn try_from(m: &Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: m.id,
            received_at: m.created_at,
            updated_at: m.updated_at,
            deleted_at: m.deleted_at,
            webhook_config_id: m.webhook_config_id,
            common: nomail_app::message::MessageContent {
                title: m.title.clone(),
                summary: m.summary.clone(),
                level: serde_json::from_value(serde_json::json!(m.level))?,
                url: m.url.clone(),
            },
            status: m.status.clone(),
            pinned: m.pinned,
            provider: serde_json::from_value(serde_json::json!(m.provider))?,
            metadata: nomail_app::webhook::EventMetadata {
                event_type: m.event_type.clone(),
                version: m.event_version.clone(),
                deduplication_key: m
                    .deduplication_scope
                    .clone()
                    .zip(m.deduplication_key.clone())
                    .map(|(scope, value)| nomail_app::webhook::DeduplicationKey { scope, value }),
            },
            raw_headers: serde_json::from_str(&m.raw_headers)?,
            raw_payload: m.raw_payload.clone(),
            content_type: m.content_type.clone(),
            expires_at: m.expire_at,
            read_at: m.read_at,
        })
    }
}
