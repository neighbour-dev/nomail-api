//! D1 configuration encoding and scoped atomic writes only.
use crate::{db, time};
use chrono::{DateTime, Utc};
use nomail_app::webhook::config::*;
use serde_json::{Value, json};

pub(crate) struct Store<'a>(pub &'a db::Store);
const OWNED: &str = "id=? AND user_id=? AND deleted_at IS NULL";
// Internal storage projection; the application decrypts and redacts before HTTP.
const COLUMNS: &str = "id,name,key,provider,encrypted_platform,enabled,description";
fn storage(error: db::StorageError) -> ConfigError {
    match error {
        db::StorageError::Conflict => ConfigError::Conflict,
        error => ConfigError::Internal(error.to_string()),
    }
}
fn d1(error: worker::Error) -> ConfigError {
    storage(error.into())
}
fn invalid(message: &str) -> ConfigError {
    ConfigError::Internal(message.into())
}
fn response(row: Value) -> ConfigResult<ConfigRecord> {
    let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
    Ok(ConfigRecord::new(
        row["id"]
            .as_i64()
            .ok_or_else(|| invalid("invalid config id"))?,
        text("name"),
        text("key"),
        text("provider")
            .parse()
            .map_err(|_| invalid("invalid stored provider"))?,
        text("encrypted_platform"),
        row["enabled"].as_i64() == Some(1),
        text("description"),
    ))
}
impl WebhookConfigStore for Store<'_> {
    async fn list_owned(
        &self,
        user_id: i64,
        page: &ConfigPage,
    ) -> ConfigResult<(Vec<ConfigRecord>, u64)> {
        let results = self.0.0.batch(vec![
            self.0.statement("SELECT COUNT(*) AS total FROM webhook_configs WHERE user_id=? AND deleted_at IS NULL", vec![json!(user_id)]).map_err(storage)?,
            self.0.statement(&format!("SELECT {COLUMNS} FROM webhook_configs WHERE user_id=? AND deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?"), vec![json!(user_id), json!(page.size()), json!(page.offset().to_string())]).map_err(storage)?,
        ]).await.map_err(d1)?;
        let total = results[0]
            .results::<Value>()
            .map_err(d1)?
            .first()
            .and_then(|v| v["total"].as_u64())
            .ok_or_else(|| invalid("invalid config count"))?;
        let items = results[1]
            .results::<Value>()
            .map_err(d1)?
            .into_iter()
            .map(response)
            .collect::<ConfigResult<Vec<_>>>()?;
        Ok((items, total))
    }
    async fn get_owned(&self, user_id: i64, id: i64) -> ConfigResult<Option<ConfigRecord>> {
        self.0
            .rows(
                &format!("SELECT {COLUMNS} FROM webhook_configs WHERE {OWNED}"),
                vec![json!(id.to_string()), json!(user_id)],
            )
            .await
            .map_err(storage)?
            .into_iter()
            .next()
            .map(response)
            .transpose()
    }
    async fn insert(&self, c: &NewConfig) -> ConfigResult<ConfigRecord> {
        let at = time::format(c.at.timestamp_millis());
        let rows = self.0.rows(&format!("INSERT INTO webhook_configs (user_id,name,key,provider,encrypted_platform,description,enabled,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?) RETURNING {COLUMNS}"),
            vec![json!(c.user_id), json!(c.name), json!(c.key), json!(c.provider), json!(c.encrypted_platform), json!(c.description), json!(c.enabled), json!(at), json!(at)]).await.map_err(storage)?;
        response(
            rows.into_iter()
                .next()
                .ok_or_else(|| invalid("inserted config disappeared"))?,
        )
    }
    async fn mutate_owned(
        &self,
        user_id: i64,
        id: i64,
        command: &ConfigMutation,
        at: DateTime<Utc>,
    ) -> ConfigResult<Option<ConfigRecord>> {
        let mut sets = vec!["updated_at=?"];
        let mut values = vec![json!(time::format(at.timestamp_millis()))];
        match command {
            ConfigMutation::Delete => {
                sets.push("deleted_at=?");
                values.push(json!(time::format(at.timestamp_millis())));
            }
            ConfigMutation::Enabled(enabled) => {
                sets.push("enabled=?");
                values.push(json!(enabled));
            }
            ConfigMutation::Patch(patch) => {
                for (column, value) in [
                    ("name=?", &patch.name),
                    ("description=?", &patch.description),
                ] {
                    if let Some(value) = value {
                        sets.push(column);
                        values.push(json!(value));
                    }
                }
                if let Some(platform) = &patch.platform {
                    sets.extend(["provider=?", "encrypted_platform=?"]);
                    values.extend([json!(platform.provider), json!(platform.ciphertext)]);
                }
            }
        }
        values.extend([json!(id.to_string()), json!(user_id)]);
        // RETURNING reads the exact row written by this scoped statement. A
        // soft-deleted row is still returned by its own UPDATE.
        self.0
            .rows(
                &format!(
                    "UPDATE webhook_configs SET {} WHERE {OWNED} RETURNING {COLUMNS}",
                    sets.join(",")
                ),
                values,
            )
            .await
            .map_err(storage)?
            .into_iter()
            .next()
            .map(response)
            .transpose()
    }
}
