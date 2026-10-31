use super::*;
use crate::{auth, message, webhook};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Delete,
}
impl Method {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
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
    Raw,
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
    type Query: QueryDto;
    type Response: Serialize + DeserializeOwned;
    const OPERATION: Operation;
}

// This is the ONLY list of paths, methods, authentication, body/response kinds
// and associated DTO types. Enum, bindings metadata and OpenAPI all use it.
macro_rules! endpoints {
    ($($name:ident => ($id:literal,$method:ident,$path:literal,$auth:ident,$body:ident,$response:ident,$query:literal,$param:ident,$request:ty,$query_type:ty,$result:ty,$tag:literal,$summary:literal)),* $(,)?) => {
        #[derive(Clone,Copy,Debug,PartialEq,Eq)]
        #[repr(usize)]
        pub enum Operation {$($name),*}
        impl Operation {
            pub const ALL:&'static[Self]=&[$(Self::$name),*];
            pub fn route(self)->&'static Route {&ROUTES[self as usize]}
        }
        pub mod endpoints {
            use super::*;
            $(pub struct $name;
            impl Endpoint for $name {
                type Request=$request;type Query=$query_type;type Response=$result;
                const OPERATION:Operation=Operation::$name;
            })*
        }
        pub static ROUTES:&[Route]=&[$(Route {
            operation:Operation::$name,operation_id:$id,method:Method::$method,path:$path,
            auth:AuthRequirement::$auth,body:BodyKind::$body,response:ResponseKind::$response,
            has_query:$query,path_kind:PathKind::$param,tag:$tag,summary:$summary,
            #[cfg(feature="openapi")] request_schema:openapi::request_schema::<$request>,
            #[cfg(feature="openapi")] query_schema:openapi::request_schema::<$query_type>,
            #[cfg(feature="openapi")] response_schema:openapi::response_schema::<ApiResponse<$result>>,
        }),*];
    }
}
endpoints! {
    Health => ("health",Get,"/api/health",Public,None,Json,false,None,(),(),HealthResponse,"Health","健康检查"),
    Login => ("login",Post,"/api/auth/login",Public,Json,Json,false,None,auth::LoginRequest,(),auth::TokenResponse,"Auth","登录"),
    Refresh => ("refreshToken",Post,"/api/auth/refresh-token",Public,Json,Json,false,None,auth::RefreshRequest,(),auth::TokenResponse,"Auth","轮换刷新令牌"),
    Logout => ("logout",Post,"/api/auth/logout",Public,Json,Json,false,None,auth::RefreshRequest,(),(),"Auth","退出登录"),
    GetAccount => ("getAccount",Get,"/api/auth/account",Bearer,None,Json,false,None,(),(),auth::AccountResponse,"Auth","获取账号"),
    UpdateAccount => ("updateAccount",Patch,"/api/auth/account",Bearer,Json,Json,false,None,auth::AccountUpdateRequest,(),auth::AccountResponse,"Auth","修改账号"),
    ListDevices => ("listDevices",Get,"/api/auth/devices",Bearer,None,Json,false,None,(),(),Vec<auth::DeviceResponse>,"Auth","设备列表"),
    KickDevice => ("kickDevice",Delete,"/api/auth/devices/{deviceId}/session",Bearer,None,Json,false,DeviceId,(),(),(),"Auth","撤销设备会话"),
    DeleteDevice => ("deleteDevice",Delete,"/api/auth/devices/{deviceId}",Bearer,None,Json,false,DeviceId,(),(),(),"Auth","删除设备"),
    ListConfigs => ("listWebhookConfigs",Get,"/api/webhook/configs",Bearer,None,Json,true,None,(),PaginationQuery,Page<webhook::config::ConfigResponse>,"Webhook Configs","接收配置列表"),
    CreateConfig => ("createWebhookConfig",Post,"/api/webhook/configs",Bearer,Json,Json,false,None,webhook::config::ConfigRequest,(),webhook::config::ConfigResponse,"Webhook Configs","创建接收配置"),
    GetConfig => ("getWebhookConfig",Get,"/api/webhook/configs/{id}",Bearer,None,Json,false,ConfigId,(),(),webhook::config::ConfigResponse,"Webhook Configs","配置详情"),
    UpdateConfig => ("updateWebhookConfig",Patch,"/api/webhook/configs/{id}",Bearer,Json,Json,false,ConfigId,webhook::config::ConfigRequest,(),webhook::config::ConfigResponse,"Webhook Configs","修改接收配置"),
    DeleteConfig => ("deleteWebhookConfig",Delete,"/api/webhook/configs/{id}",Bearer,None,Json,false,ConfigId,(),(),(),"Webhook Configs","删除接收配置"),
    EnableConfig => ("enableWebhookConfig",Patch,"/api/webhook/configs/{id}/enabled",Bearer,Json,Json,false,ConfigId,webhook::config::EnabledRequest,(),webhook::config::ConfigResponse,"Webhook Configs","启停接收配置"),
    ReceiveWebhook => ("receiveWebhook",Post,"/api/webhook/{key}",Webhook,Raw,Webhook,false,WebhookKey,Vec<u8>,(),webhook::ReceiveResponse,"Webhook","接收平台消息或控制请求"),
    ListMessages => ("listMessages",Get,"/api/messages",Bearer,None,Json,true,None,(),message::ListQuery,Page<message::MessageListItem>,"Messages","消息列表"),
    UnreadCount => ("unreadCount",Get,"/api/messages/unread-count",Bearer,None,Json,false,None,(),(),CountResponse,"Messages","未读数量"),
    StreamMessages => ("streamMessages",Get,"/api/messages/stream",Bearer,None,Sse,false,None,(),(),(),"Messages","在线 SSE 事件流"),
    BatchMessages => ("batchMessages",Post,"/api/messages/batch",Bearer,Json,Json,false,None,message::BatchRequest,(),AffectedResponse,"Messages","批量修改消息"),
    GetMessage => ("getMessage",Get,"/api/messages/{id}",Bearer,None,Json,false,MessageId,(),(),message::MessageDetail,"Messages","消息详情"),
    UpdateMessage => ("updateMessage",Patch,"/api/messages/{id}",Bearer,Json,Json,false,MessageId,message::UpdateRequest,(),message::MessageDetail,"Messages","修改消息状态"),
    DeleteMessage => ("deleteMessage",Delete,"/api/messages/{id}",Bearer,None,Json,false,MessageId,(),(),(),"Messages","删除消息"),
    RawMessage => ("rawMessage",Get,"/api/messages/{id}/raw",Bearer,None,Raw,false,MessageId,(),(),(),"Messages","读取可读原始内容"),
}

pub struct RouteMatch {
    pub route: &'static Route,
    pub parameter: Option<String>,
}
impl RouteMatch {
    pub fn parameter(&self) -> Result<&str, ApiFailure> {
        let value = self.parameter.as_deref().ok_or_else(ApiFailure::internal)?;
        match self.route.path_kind {
            PathKind::DeviceId => {
                crate::auth::check_length(value, "deviceId", 128, true).map_err(ApiFailure::bad)?
            }
            PathKind::ConfigId => {
                value
                    .parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| ApiFailure::bad("invalid webhook config id"))?;
            }
            PathKind::MessageId => {
                uuid::Uuid::parse_str(value).map_err(|_| ApiFailure::bad("invalid message id"))?;
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

impl Operation {
    pub fn required_body_fields(self) -> &'static [&'static str] {
        match self {
            Self::CreateConfig => webhook::config::ConfigRequest::REQUIRED_CREATE_FIELDS,
            _ => &[],
        }
    }
}
