use crate::error::{ApiError, Result};
use serde_json::Value;
use worker::{D1Database, D1PreparedStatement, D1Result, wasm_bindgen::JsValue};

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
                            .ok_or_else(|| ApiError::bad("invalid numeric parameter"))?;
                        if number.abs() > 9_007_199_254_740_991.0 {
                            return Err(ApiError::bad(
                                "numeric parameter exceeds D1 safe integer range",
                            ));
                        }
                        JsValue::from_f64(number)
                    }
                    _ => {
                        return Err(worker::Error::RustError(
                            "non-scalar database parameter".into(),
                        )
                        .into());
                    }
                })
            })
            .collect::<Result<_>>()?;
        Ok(self.0.prepare(sql).bind(&bindings)?)
    }
    pub async fn rows(&self, sql: &str, values: Vec<Value>) -> Result<Vec<Value>> {
        Ok(self.statement(sql, values)?.all().await?.results()?)
    }
    pub async fn one(&self, sql: &str, values: Vec<Value>) -> Result<Value> {
        self.rows(sql, values)
            .await?
            .into_iter()
            .next()
            .ok_or_else(ApiError::not_found)
    }
    pub async fn run(&self, sql: &str, values: Vec<Value>) -> Result<D1Result> {
        Ok(self.statement(sql, values)?.run().await?)
    }
}
