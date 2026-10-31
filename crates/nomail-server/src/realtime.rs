pub use nomail_app::http::sse::{SseEvent as Event, encode};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::mpsc;

type Subscribers = HashMap<i64, HashMap<u64, mpsc::Sender<Event>>>;

/// Native SSE connection execution; routing/authentication are already complete.
pub(crate) fn stream(
    hub: &Hub,
    request: nomail_app::http::Subscription,
) -> actix_web::HttpResponse {
    use actix_web::{HttpResponse, web};
    use chrono::Utc;
    use nomail_app::http::sse::{HEARTBEAT, HEARTBEAT_SECONDS};

    let mut subscription = hub.subscribe(request.user_id);
    let stream = async_stream::stream! {
        yield Ok::<_,actix_web::Error>(web::Bytes::from(encode(&Event::ready(Utc::now()))));
        let period = std::time::Duration::from_secs(HEARTBEAT_SECONDS);
        let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        let expiry = tokio::time::sleep((request.expires_at - Utc::now()).to_std().unwrap_or_default());
        tokio::pin!(expiry);
        loop {
            tokio::select! {
                _ = &mut expiry => {
                    yield Ok(web::Bytes::from(encode(&Event::auth_expired())));
                    break;
                }
                _ = heartbeat.tick() => yield Ok(web::Bytes::from_static(HEARTBEAT)),
                event = subscription.receiver.recv() => match event {
                    Some(event) => yield Ok(web::Bytes::from(encode(&event))),
                    None => break,
                },
            }
        }
        drop(subscription);
    };
    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

#[derive(Clone, Default)]
pub struct Hub {
    inner: Arc<Mutex<Subscribers>>,
    next: Arc<AtomicU64>,
}

pub struct Subscription {
    pub receiver: mpsc::Receiver<Event>,
    user_id: i64,
    id: u64,
    hub: Weak<Mutex<Subscribers>>,
}

impl Hub {
    pub fn subscribe(&self, user_id: i64) -> Subscription {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel(32);
        self.inner
            .lock()
            .expect("hub lock")
            .entry(user_id)
            .or_default()
            .insert(id, sender);
        Subscription {
            receiver,
            user_id,
            id,
            hub: Arc::downgrade(&self.inner),
        }
    }
    pub fn publish(&self, user_id: i64, event: Event) {
        if let Some(subscribers) = self.inner.lock().expect("hub lock").get(&user_id) {
            for subscriber in subscribers.values() {
                let _ = subscriber.try_send(event.clone());
            }
        }
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(hub) = self.hub.upgrade() {
            let mut hub = hub.lock().expect("hub lock");
            if let Some(subscribers) = hub.get_mut(&self.user_id) {
                subscribers.remove(&self.id);
                if subscribers.is_empty() {
                    hub.remove(&self.user_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn isolation_backpressure_and_cleanup() {
        let hub = Hub::default();
        let mut one = hub.subscribe(1);
        let mut two = hub.subscribe(2);
        let event = Event::ready(chrono::Utc::now());
        for _ in 0..40 {
            hub.publish(1, event.clone());
        }
        assert_eq!(one.receiver.len(), 32);
        assert!(two.receiver.try_recv().is_err());
        assert_eq!(one.receiver.recv().await.unwrap().event_name(), "ready");
        assert!(encode(&event).starts_with("event: ready\ndata: "));
        drop(one);
        assert!(!hub.inner.lock().unwrap().contains_key(&1));
    }
}
