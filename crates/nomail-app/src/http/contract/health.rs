use super::routes::endpoint;
use crate::http::{Environment, HealthCheck, HttpResult, RequestHead};
use serde::{Deserialize, Serialize};

endpoint! {
    Health {
        method: Get,
        path: "/api/health",
        auth: Public,
        operation_id: "health",
        tag: "Health",
        summary: "健康检查",
        request: (),
        query: (),
        response: HealthResponse,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl Health {
    pub(in crate::http) async fn handle(
        environment: &impl Environment,
        head: &RequestHead,
    ) -> HttpResult<HealthResponse> {
        let HealthCheck::Database(healthy) = environment.health(head).await?;
        Ok(HealthResponse::database(healthy))
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Error,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub db: HealthStatus,
}
impl HealthResponse {
    pub fn database(healthy: bool) -> Self {
        let status = if healthy {
            HealthStatus::Ok
        } else {
            HealthStatus::Error
        };
        Self { status, db: status }
    }
}
