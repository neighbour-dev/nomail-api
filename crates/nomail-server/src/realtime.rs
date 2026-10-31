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
