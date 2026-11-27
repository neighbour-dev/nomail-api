use super::super::{ApiFailure, Environment, HttpOutcome, HttpResult, dispatch::EndpointCall};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}
impl Method {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthRequirement {
    Public,
    Bearer,
    Webhook,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    None,
    Json,
    Raw,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseKind {
    Json,
    Sse,
    Webhook,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    None,
    DeviceId,
    ConfigId,
    MessageId,
    WebhookKey,
}

pub struct Route {
    pub operation: Operation,
    pub operation_id: &'static str,
    pub method: Method,
    pub path: &'static str,
    pub auth: AuthRequirement,
    pub body: BodyKind,
    pub response: ResponseKind,
    pub has_query: bool,
    pub path_kind: PathKind,
    pub tag: &'static str,
    pub summary: &'static str,
    #[cfg(feature = "openapi")]
    pub request_schema: fn() -> serde_json::Value,
    #[cfg(feature = "openapi")]
    pub query_schema: fn() -> serde_json::Value,
    #[cfg(feature = "openapi")]
    pub response_schema: fn() -> serde_json::Value,
}
pub trait Endpoint {
    type Request: DeserializeOwned;
    type Query: DeserializeOwned;
    type Response: Serialize + DeserializeOwned;
    const OPERATION: Operation;
}

// Each endpoint owns its route metadata and DTOs. OpenAPI reads the same route as dispatch.
macro_rules! endpoint {
    (
        $name:ident {
            method: $method:ident,
            path: $path:literal,
            auth: $auth:ident,
            operation_id: $id:literal,
            tag: $tag:literal,
            summary: $summary:literal,
            request: $request:ty,
            query: $query_type:ty,
            response: $result:ty,
            body_kind: $body:ident,
            response_kind: $response:ident,
            has_query: $query:literal,
            path_kind: $param:ident $(,)?
        }
    ) => {
        pub struct $name;

        impl $crate::http::Endpoint for $name {
            type Request = $request;
            type Query = $query_type;
            type Response = $result;
            const OPERATION: $crate::http::Operation = $crate::http::Operation::$name;
        }

        impl $name {
            pub(in crate::http) const ROUTE: $crate::http::Route = $crate::http::Route {
                operation: $crate::http::Operation::$name,
                operation_id: $id,
                method: $crate::http::Method::$method,
                path: $path,
                auth: $crate::http::AuthRequirement::$auth,
                body: $crate::http::BodyKind::$body,
                response: $crate::http::ResponseKind::$response,
                has_query: $query,
                path_kind: $crate::http::PathKind::$param,
                tag: $tag,
                summary: $summary,
                #[cfg(feature = "openapi")]
                request_schema: $crate::http::openapi::request_schema::<$request>,
                #[cfg(feature = "openapi")]
                query_schema: $crate::http::openapi::request_schema::<$query_type>,
                #[cfg(feature = "openapi")]
                response_schema: $crate::http::openapi::response_schema::<
                    $crate::http::EndpointResponse<Self>,
                >,
            };

            endpoint!(@respond $response, $result);
        }
    };
    (@respond Json, $result:ty) => {
        pub(in crate::http) fn respond(
            data: $result,
        ) -> $crate::http::HttpResult<$crate::http::HttpOutcome> {
            $crate::http::transport::json::<Self>(data)
        }
    };
    (@respond Sse, $result:ty) => {
        endpoint!(@passthrough);
    };
    (@respond Webhook, $result:ty) => {
        endpoint!(@passthrough);
    };
    (@passthrough) => {
        pub(in crate::http) fn respond(
            outcome: $crate::http::HttpOutcome,
        ) -> $crate::http::HttpResult<$crate::http::HttpOutcome> {
            Ok(outcome)
        }
    };
}
pub(super) use endpoint;

// Explicit inventory; generates the enum, static routes and compile-time handler bindings.
macro_rules! endpoints {
    (Health in $health:ident; $($name:ident in $($module:ident)::+),* $(,)?) => {
        #[derive(Clone,Copy,Debug,PartialEq,Eq)]
        #[repr(usize)]
        pub enum Operation { Health, $($name),* }
        impl Operation {
            pub const ALL:&'static[Self]=&[Self::Health, $(Self::$name),*];
            pub fn route(self)->&'static Route {&ROUTES[self as usize]}

            pub(in crate::http) async fn handle(self, call: EndpointCall<'_, impl Environment>) -> HttpResult<HttpOutcome> {
                match self {
                    Self::Health => unreachable!("health is dispatched before service configuration"),
                    $(Self::$name => endpoints::$name::respond(endpoints::$name::handle(call).await?),)*
                }
            }
        }
        pub mod endpoints {
            pub use super::super::$health::Health;
            $(pub use super::super::$($module)::+::$name;)*
        }
        pub static ROUTES:&[Route]=&[endpoints::Health::ROUTE, $(endpoints::$name::ROUTE),*];
    }
}
endpoints! {
    Health in health;
    SetupStatus in auth::setup_status,
    Setup in auth::setup,
    Login in auth::login,
    Refresh in auth::refresh,
    Logout in auth::logout,
    GetAccount in auth::get_account,
    UpdateAccount in auth::update_account,
    ListDevices in device::list_devices,
    RegisterPush in device::register_push,
    RemovePush in device::remove_push,
    GetPushPreferences in device::get_push_preferences,
    SetPushPreferences in device::set_push_preferences,
    KickDevice in device::kick_device,
    DeleteDevice in device::delete_device,
    ListConfigs in webhook::list_configs,
    CreateConfig in webhook::create_config,
    GetConfig in webhook::get_config,
    UpdateConfig in webhook::update_config,
    DeleteConfig in webhook::delete_config,
    EnableConfig in webhook::enable_config,
    ReceiveWebhook in webhook::receive_webhook,
    ListMessages in message::list_messages,
    CursorMessages in message::cursor_messages,
    UnreadCount in message::unread_count,
    StreamMessages in message::stream_messages,
    BatchMessages in message::batch_messages,
    GetMessage in message::get_message,
    UpdateMessage in message::update_message,
    DeleteMessage in message::delete_message,
    RawMessage in message::raw_message,
}

pub struct RouteMatch {
    pub route: &'static Route,
    pub parameter: Option<String>,
}
impl RouteMatch {
    fn raw_parameter(&self) -> Result<&str, ApiFailure> {
        self.parameter.as_deref().ok_or_else(ApiFailure::internal)
    }
    pub fn config_id(&self) -> Result<crate::webhook::config::ConfigId, ApiFailure> {
        if self.route.path_kind != PathKind::ConfigId {
            return Err(ApiFailure::internal());
        }
        let value = self
            .raw_parameter()?
            .parse::<i64>()
            .map_err(|_| ApiFailure::bad("invalid webhook config id"))?;
        crate::webhook::config::ConfigId::new(value).map_err(ApiFailure::from)
    }
    pub fn message_id(&self) -> Result<uuid::Uuid, ApiFailure> {
        if self.route.path_kind != PathKind::MessageId {
            return Err(ApiFailure::internal());
        }
        uuid::Uuid::parse_str(self.raw_parameter()?)
            .map_err(|_| ApiFailure::bad("invalid message id"))
    }
    /// Text parameters and compatibility access. Dispatch uses typed accessors
    /// for IDs so validated values never need to be parsed again by use cases.
    pub fn parameter(&self) -> Result<&str, ApiFailure> {
        let value = self.raw_parameter()?;
        match self.route.path_kind {
            PathKind::DeviceId => crate::device::validate_id(value).map_err(ApiFailure::bad)?,
            PathKind::ConfigId => {
                self.config_id()?;
            }
            PathKind::MessageId => {
                self.message_id()?;
            }
            _ => (),
        }
        Ok(value)
    }
}
fn parameter_segment(segment: &str) -> bool {
    segment.starts_with('{') && segment.ends_with('}')
}

pub fn resolve(method: &str, path: &str) -> Result<RouteMatch, ApiFailure> {
    let path = path.trim_end_matches('/');
    if !path.starts_with('/') || path[1..].split('/').any(str::is_empty) {
        return Err(ApiFailure::not_found());
    }
    let segments: Vec<&str> = path[1..].split('/').collect();
    let mut best_score = None;
    let mut best = None;
    for route in ROUTES {
        let template: Vec<&str> = route.path[1..].split('/').collect();
        if template.len() != segments.len()
            || !template
                .iter()
                .zip(&segments)
                .all(|(t, s)| parameter_segment(t) || t == s)
        {
            continue;
        }
        let score = template.iter().filter(|s| !parameter_segment(s)).count();
        if best_score.is_none_or(|old| score > old) {
            best_score = Some(score);
            best = None;
        }
        if best_score == Some(score) && route.method.as_str() == method {
            let parameter = template
                .iter()
                .zip(&segments)
                .find(|(t, _)| parameter_segment(t))
                .map(|(_, s)| decode_parameter(s))
                .transpose()?;
            best = Some(RouteMatch { route, parameter });
        }
    }
    // Preserve the existing public not-found contract for unsupported methods.
    best.ok_or_else(ApiFailure::not_found)
}
fn decode_parameter(value: &str) -> Result<String, ApiFailure> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return Err(ApiFailure::bad("invalid URL parameter"));
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    percent_decode_str(value)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| ApiFailure::bad("invalid URL parameter"))
}
pub fn webhook_url(key: &str) -> String {
    Operation::ReceiveWebhook.route().path.replace(
        "{key}",
        &utf8_percent_encode(key, NON_ALPHANUMERIC).to_string(),
    )
}
