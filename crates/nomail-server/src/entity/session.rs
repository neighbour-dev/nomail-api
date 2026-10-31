use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
    pub device_id: i64,
    pub refresh_token_hash: String,
    pub expires_at: DateTimeUtc,
    pub logged_in_at: DateTimeUtc,
    pub revoked_at: Option<DateTimeUtc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
