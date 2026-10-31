//! SQLite configuration encoding and scoped atomic writes only.
use crate::{db, entity::webhook};
use chrono::{DateTime, Utc};
use nomail_core::webhook::config::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};

pub(super) struct Store<'a>(pub &'a DatabaseConnection);
fn storage(error: DbErr) -> ConfigError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        ConfigError::Conflict
    } else {
        ConfigError::Internal(error.to_string())
    }
}
fn response(c: webhook::Model) -> ConfigResult<ConfigResponse> {
    Ok(ConfigResponse::new(
        c.id,
        c.name,
        c.key,
        c.provider
            .parse()
            .map_err(|e: nomail_core::webhook::WebhookError| {
                ConfigError::Internal(e.to_string())
            })?,
        c.source.unwrap_or_default(),
        c.enabled,
        c.description.unwrap_or_default(),
    ))
}
fn owned(user_id: i64) -> sea_orm::Select<webhook::Entity> {
    webhook::Entity::find()
        .filter(webhook::Column::UserId.eq(user_id))
        .filter(webhook::Column::DeletedAt.is_null())
}
async fn get(
    db: &impl ConnectionTrait,
    user_id: i64,
    id: i64,
) -> ConfigResult<Option<ConfigResponse>> {
    owned(user_id)
        .filter(webhook::Column::Id.eq(id))
        .one(db)
        .await
        .map_err(storage)?
        .map(response)
        .transpose()
}
impl WebhookConfigStore for Store<'_> {
    async fn list_owned(
        &self,
        user_id: i64,
        page: &ConfigPage,
    ) -> ConfigResult<(Vec<ConfigResponse>, u64)> {
        let tx = self.0.begin().await.map_err(storage)?;
        let total = owned(user_id).count(&tx).await.map_err(storage)?;
        let items = owned(user_id)
            .order_by_desc(webhook::Column::CreatedAt)
            .offset(page.offset)
            .limit(page.size)
            .all(&tx)
            .await
            .map_err(storage)?
            .into_iter()
            .map(response)
            .collect::<ConfigResult<Vec<_>>>()?;
        tx.commit().await.map_err(storage)?;
        Ok((items, total))
    }
    async fn get_owned(&self, user_id: i64, id: i64) -> ConfigResult<Option<ConfigResponse>> {
        get(self.0, user_id, id).await
    }
    async fn insert(&self, c: &NewConfig) -> ConfigResult<ConfigResponse> {
        response(
            webhook::ActiveModel {
                user_id: Set(c.user_id),
                name: Set(c.name.clone()),
                key: Set(c.key.clone()),
                provider: Set(c.provider.as_str().into()),
                source: Set(Some(c.source.clone())),
                encrypted_secret: Set(Some(c.encrypted_secret.clone())),
                enabled: Set(c.enabled),
                description: Set(Some(c.description.clone())),
                created_at: Set(c.at),
                updated_at: Set(c.at),
                deleted_at: Set(None),
                ..Default::default()
            }
            .insert(self.0)
            .await
            .map_err(storage)?,
        )
    }
    async fn mutate_owned(
        &self,
        user_id: i64,
        id: i64,
        command: &ConfigMutation,
        at: DateTime<Utc>,
    ) -> ConfigResult<Option<ConfigResponse>> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let before = if matches!(command, ConfigMutation::Delete) {
            get(&tx, user_id, id).await?
        } else {
            None
        };
        let mut update =
            webhook::Entity::update_many().col_expr(webhook::Column::UpdatedAt, Expr::value(at));
        match command {
            ConfigMutation::Delete => {
                update = update.col_expr(webhook::Column::DeletedAt, Expr::value(at))
            }
            ConfigMutation::Enabled(enabled) => {
                update = update.col_expr(webhook::Column::Enabled, Expr::value(*enabled))
            }
            ConfigMutation::Patch(patch) => {
                if let Some(v) = &patch.name {
                    update = update.col_expr(webhook::Column::Name, Expr::value(v.clone()));
                }
                if let Some(v) = patch.provider {
                    update = update.col_expr(webhook::Column::Provider, Expr::value(v.as_str()));
                }
                if let Some(v) = &patch.source {
                    update = update.col_expr(webhook::Column::Source, Expr::value(v.clone()));
                }
                if let Some(v) = &patch.encrypted_secret {
                    update =
                        update.col_expr(webhook::Column::EncryptedSecret, Expr::value(v.clone()));
                }
                if let Some(v) = &patch.description {
                    update = update.col_expr(webhook::Column::Description, Expr::value(v.clone()));
                }
            }
        }
        let affected = update
            .filter(webhook::Column::Id.eq(id))
            .filter(webhook::Column::UserId.eq(user_id))
            .filter(webhook::Column::DeletedAt.is_null())
            .exec(&tx)
            .await
            .map_err(storage)?
            .rows_affected;
        let result = if affected == 0 {
            None
        } else if matches!(command, ConfigMutation::Delete) {
            before
        } else {
            get(&tx, user_id, id).await?
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
