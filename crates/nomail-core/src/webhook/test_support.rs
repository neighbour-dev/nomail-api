//! Runtime-independent helpers shared by platform tests; no platform fixtures here.
use super::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub(super) const NOW: i64 = 1_793_354_400;

pub(super) fn headers(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

pub(super) fn sign(secret: &[u8], bytes: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).unwrap();
    mac.update(bytes);
    mac.finalize().into_bytes().to_vec()
}

pub(super) fn receive_with(
    provider: Provider,
    authentication: Authentication<'_>,
    headers: &dyn HeaderLookup,
    body: &[u8],
) -> Result<ReceiveOutcome, WebhookError> {
    receive(
        &WebhookConfig::new(provider, authentication, NOW),
        &WebhookRequest { headers, body },
    )
}

pub(super) fn event(outcome: ReceiveOutcome) -> PlatformEvent {
    match outcome {
        ReceiveOutcome::Event(event) => *event,
        other => panic!("expected event, got {other:?}"),
    }
}

pub(super) fn response(outcome: ReceiveOutcome) -> ProtocolResponse {
    match outcome {
        ReceiveOutcome::Respond(response) => response,
        other => panic!("expected control response, got {other:?}"),
    }
}
