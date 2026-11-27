use super::endpoint;
use crate::http::ApiFailure;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::Deserialize;

endpoint! {
    RegisterPush {
        method: Put,
        path: "/api/device/push/token",
        auth: Bearer,
        operation_id: "registerPushToken",
        tag: "Device",
        summary: "上报或更新当前登录设备的推送 Token",
        request: RegisterPushRequest,
        query: (),
        response: (),
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl RegisterPush {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        if !call.environment.push_enabled() {
            return Err(ApiFailure::new(503, "push is not configured").into());
        }
        let input = call.input::<Self>()?;
        call.services
            .devices
            .register_push(
                call.actor()?,
                input.channel.into(),
                input.device_token,
                call.now(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct RegisterPushRequest {
    /// Channel used by the relay; not inferred from the token or login platform.
    pub channel: PushChannel,
    /// APNs hexadecimal device token or case-sensitive FCM registration token.
    pub device_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum PushChannel {
    Apns,
    Fcm,
}

impl From<PushChannel> for crate::push::Channel {
    fn from(channel: PushChannel) -> Self {
        match channel {
            PushChannel::Apns => Self::Apns,
            PushChannel::Fcm => Self::Fcm,
        }
    }
}
