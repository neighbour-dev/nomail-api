use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "devices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
    pub user_id: i64,
    pub device_id: String,
    pub name: Option<String>,
    pub platform: String,
    pub device_type: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub os_version: Option<String>,
    pub app_version: Option<String>,
    pub app_build: Option<String>,
    pub locale: Option<String>,
    pub time_zone: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_seen_at: Option<DateTimeUtc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
