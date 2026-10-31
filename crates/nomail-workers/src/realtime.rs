//! Best-effort SSE fanout. Durable Objects coordinate requests from all isolates.
//! Like Server, this is not a persistent event log and offers no replay.
use futures_channel::mpsc;
use futures_util::{
    StreamExt,
    future::{Either, select},
    stream,
};
use nomail_app::http::{
    ApiFailure,
    sse::{self, HEARTBEAT, HEARTBEAT_SECONDS, MessageCreated, SseEvent},
};
use std::{cell::RefCell, time::Duration};
use worker::*;

#[durable_object]
pub struct Realtime {
    subscribers: RefCell<Vec<mpsc::Sender<Vec<u8>>>>,
}

impl DurableObject for Realtime {
    fn new(_state: State, _env: Env) -> Self {
        Self {
            subscribers: RefCell::new(Vec::new()),
        }
    }
    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.method() == Method::Post && req.path() == "/publish" {
            let data: MessageCreated = req.json().await?;
            let bytes = sse::encode(&SseEvent::MessageCreated(data)).into_bytes();
            self.subscribers.borrow_mut().retain_mut(|sender| {
                if sender.is_closed() {
                    return false;
                }
                let _ = sender.try_send(bytes.clone());
                true
            });
            return Response::ok("ok");
        }
        if req.method() != Method::Get || req.path() != "/stream" {
            return Response::error("not found", 404);
        }
        let expiry = req
            .url()?
            .query_pairs()
            .find(|(key, _)| key == "expires")
            .and_then(|(_, v)| v.parse::<i64>().ok())
            .unwrap_or(0);
        if expiry <= crate::time::now_ms() / 1000 {
            return Response::error("unauthorized", 401);
        }
        let mut subscribers = self.subscribers.borrow_mut();
        subscribers.retain(|sender| !sender.is_closed());
        // Bound both subscriber count and per-client queue memory.
        if subscribers.len() >= 32 {
            return Response::error("too many connections", 429);
        }
        let (mut sender, receiver) = mpsc::channel(32);
        let now = chrono::DateTime::from_timestamp_millis(crate::time::now_ms())
            .expect("runtime timestamp in range");
        let _ = sender.try_send(sse::encode(&SseEvent::ready(now)).into_bytes());
        subscribers.push(sender);
        drop(subscribers);
        let stream = stream::unfold(
            (receiver, false),
            move |(mut receiver, finished)| async move {
                if finished {
                    return None;
                }
                let remaining = expiry * 1000 - crate::time::now_ms();
                if remaining <= 0 {
                    return Some((
                        Ok::<_, Error>(sse::encode(&SseEvent::auth_expired()).into_bytes()),
                        (receiver, true),
                    ));
                }
                let next = receiver.next();
                let timeout = Delay::from(Duration::from_millis(
                    remaining.min((HEARTBEAT_SECONDS * 1000) as i64) as u64,
                ));
                futures_util::pin_mut!(next, timeout);
                let bytes = match select(next, timeout).await {
                    Either::Left((Some(bytes), _)) => bytes,
                    Either::Left((None, _)) => return None,
                    Either::Right(_) => HEARTBEAT.to_vec(),
                };
                Some((Ok(bytes), (receiver, false)))
            },
        );
        let mut response = Response::from_stream(stream)?;
        response
            .headers_mut()
            .set("Content-Type", "text/event-stream")?;
        response.headers_mut().set("Cache-Control", "no-cache")?;
        Ok(response)
    }
}

pub async fn publish(env: &Env, uid: i64, data: MessageCreated) -> Result<()> {
    let namespace = env.durable_object("REALTIME")?;
    let stub = namespace.id_from_name(&uid.to_string())?.get_stub()?;
    let req = Request::new_with_init(
        "https://realtime.internal/publish",
        RequestInit::new()
            .with_method(Method::Post)
            .with_body(Some(serde_json::to_string(&data)?.into())),
    )?;
    let response = stub.fetch_with_request(req).await?;
    if response.status_code() != 200 {
        return Err(Error::RustError("realtime fanout failed".into()));
    }
    Ok(())
}

pub async fn subscribe(env: &Env, uid: i64, expires: i64) -> Result<Response> {
    let namespace = env.durable_object("REALTIME")?;
    let response = namespace
        .id_from_name(&uid.to_string())?
        .get_stub()?
        .fetch_with_str(&format!(
            "https://realtime.internal/stream?expires={expires}"
        ))
        .await?;
    if response.status_code() == 200 {
        return Ok(response);
    }
    let failure = match response.status_code() {
        401 => ApiFailure::unauthorized(),
        429 => ApiFailure::new(429, "too many connections"),
        _ => ApiFailure::internal(),
    };
    let status = failure.status;
    Ok(Response::from_json(&failure.response())?.with_status(status))
}
