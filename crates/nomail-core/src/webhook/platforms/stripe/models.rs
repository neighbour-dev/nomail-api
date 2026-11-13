//! Native event envelopes. Resource fields are generated separately from the
//! pinned official specification; projections never fetch Stripe API objects.
use super::{
    generated::{StripeEventObjectTag, StripeResource, StripeV2CoreEventObjectTag},
    support::StripeValue,
};
use crate::webhook::platforms::native::NativeValue;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum StripeWebhookPayload {
    Snapshot(Box<StripeSnapshotEvent>),
    Thin(Box<StripeThinNotification>),
}

impl NativeValue for StripeWebhookPayload {
    fn read(value: &Value) -> Option<Self> {
        match value.get("object")?.as_str()? {
            "event" => StripeSnapshotEvent::read(value).map(|v| Self::Snapshot(Box::new(v))),
            "v2.core.event" => StripeThinNotification::read(value).map(|v| Self::Thin(Box::new(v))),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for StripeWebhookPayload {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::read(&value).ok_or_else(|| serde::de::Error::custom("unknown Stripe event envelope"))
    }
}

impl StripeWebhookPayload {
    pub fn is_thin(&self) -> bool {
        matches!(self, Self::Thin(_))
    }
}

stripe_model!(StripeSnapshotEvent {
    id: String = "id",
    object: StripeEventObjectTag = "object",
    event_type: String = "type",
    api_version: String = "api_version",
    created: i64 = "created",
    livemode: bool = "livemode",
    data: StripeEventData = "data",
    account: String = "account",
    context: String = "context",
    pending_webhooks: i64 = "pending_webhooks",
    request: StripeEventRequest = "request",
});

stripe_model!(StripeThinNotification {
    id: String = "id",
    object: StripeV2CoreEventObjectTag = "object",
    event_type: String = "type",
    created: String = "created",
    livemode: bool = "livemode",
    context: String = "context",
    reason: StripeEventReason = "reason",
    related_object: StripeRelatedObject = "related_object",
    snapshot_event: String = "snapshot_event",
    // When supplied, keep these open hashes distinct from snapshot data.object.
    // No fields are fabricated or retrieved via the Events v2 API.
    data: BTreeMap<String, Value> = "data",
    changes: BTreeMap<String, Value> = "changes",
});

stripe_model!(StripeEventData {
    object: StripeResource = "object",
    // This is a sparse, recursive patch of OLD values, not a full resource.
    // Metadata deletions and historical array semantics must remain verbatim.
    previous_attributes: BTreeMap<String, Value> = "previous_attributes",
});
stripe_model!(StripeEventRequest {
    id: String = "id",
    idempotency_key: String = "idempotency_key",
});
stripe_model!(StripeEventReason {
    reason_type: String = "type",
    request: StripeEventRequest = "request",
});
stripe_model!(StripeRelatedObject {
    id: String = "id",
    object_type: String = "type",
    url: String = "url",
});
