use serde_json::Value;
use worker::{D1Database, D1PreparedStatement, D1Result, wasm_bindgen::JsValue};

/// Storage failures never carry HTTP status codes or response types.
#[derive(Debug)]
pub enum StorageError {
    Conflict,
    Internal(&'static str),
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict => f.write_str("resource already exists"),
            Self::Internal(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for StorageError {}
impl From<worker::Error> for StorageError {
    fn from(error: worker::Error) -> Self {
        if error.to_string().contains("UNIQUE constraint failed") {
            Self::Conflict
        } else {
            Self::Internal("database operation failed")
        }
    }
}
type Result<T> = std::result::Result<T, StorageError>;

pub struct Store(pub D1Database);
impl Store {
    pub fn statement(&self, sql: &str, values: Vec<Value>) -> Result<D1PreparedStatement> {
        let bindings: Vec<JsValue> = values
            .into_iter()
            .map(|v| {
                Ok(match v {
                    Value::Null => JsValue::NULL,
                    Value::String(s) => JsValue::from_str(&s),
                    Value::Bool(b) => JsValue::from_f64(if b { 1.0 } else { 0.0 }),
                    Value::Number(n) => {
                        let number = n
                            .as_f64()
                            .ok_or(StorageError::Internal("invalid numeric parameter"))?;
                        if number.abs() > 9_007_199_254_740_991.0 {
                            return Err(StorageError::Internal(
                                "numeric parameter exceeds D1 safe integer range",
                            ));
                        }
                        JsValue::from_f64(number)
                    }
                    _ => {
                        return Err(StorageError::Internal("non-scalar database parameter"));
                    }
                })
            })
            .collect::<Result<_>>()?;
        Ok(self.0.prepare(sql).bind(&bindings)?)
    }
    pub async fn rows(&self, sql: &str, values: Vec<Value>) -> Result<Vec<Value>> {
        Ok(self.statement(sql, values)?.all().await?.results()?)
    }
    pub async fn run(&self, sql: &str, values: Vec<Value>) -> Result<D1Result> {
        Ok(self.statement(sql, values)?.run().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_storage_classifies_unique_constraint_failures() {
        assert!(matches!(
            StorageError::from(worker::Error::RustError(
                "UNIQUE constraint failed: sensitive value".into()
            )),
            StorageError::Conflict
        ));
        let error = StorageError::from(worker::Error::RustError(
            "sensitive database failure".into(),
        ));
        assert!(matches!(
            error,
            StorageError::Internal("database operation failed")
        ));
        assert!(!format!("{error:?}").contains("sensitive"));
    }
}
