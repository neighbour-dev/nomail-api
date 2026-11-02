//! Native service composition. No endpoint binding lives here.
use crate::{AppState, auth, message, runtime::Runtime, webhook};
use nomail_app::http::{
    Environment, EnvironmentServices, HealthCheck, HttpResult, RequestHead, Services,
};

pub(crate) struct NativeEnvironment<'a>(pub &'a AppState);
impl<'a> Environment for NativeEnvironment<'a> {
    type AuthStore = auth::repository::Store<'a>;
    type MessageStore = message::repository::Store<'a>;
    type ConfigStore = webhook::configs::repository::Store<'a>;
    type IngestionStore = message::repository::Store<'a>;
    type Runtime = Runtime;

    fn services(&self) -> HttpResult<EnvironmentServices<Self>> {
        let state = self.0;
        Ok(Services::new(
            auth::repository::Store(&state.db),
            message::repository::Store(&state.db),
            webhook::configs::repository::Store(&state.db),
            message::repository::Store(&state.db),
            Runtime {
                encryption_key: state.config.encryption_key,
            },
            auth::auth_config(&state.config),
        ))
    }
    async fn health(&self, _: &RequestHead) -> HttpResult<HealthCheck> {
        Ok(HealthCheck::Database(self.0.db.ping().await.is_ok()))
    }
}
