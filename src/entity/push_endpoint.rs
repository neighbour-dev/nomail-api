use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "push_endpoints")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
    pub device_id: i64,
    pub provider: String,
    pub environment: String,
    pub encrypted_token: String,
    pub token_hash: String,
    pub enabled: bool,
    pub registered_at: DateTimeUtc,
    pub invalidated_at: Option<DateTimeUtc>,
    pub last_error: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
