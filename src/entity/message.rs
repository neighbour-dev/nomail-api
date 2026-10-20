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
    pub external_id: Option<String>,
    pub source: Option<String>,
    pub title: String,
    pub content: Option<String>,
    pub level: String,
    pub status: String,
    pub url: Option<String>,
    pub raw_payload: Option<String>,
    pub content_type: Option<String>,
    pub pinned: Option<bool>,
    pub expire_at: Option<DateTimeUtc>,
    pub read_at: Option<DateTimeUtc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
