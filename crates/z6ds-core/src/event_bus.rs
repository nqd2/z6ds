//! M00 — in-process publish/subscribe event bus.

use std::sync::{Arc, RwLock};

use crate::contracts::AppEvent;

type Handler = Box<dyn Fn(&AppEvent) + Send + Sync>;

/// Thread-safe event bus for `AppEvent` delivery.
#[derive(Clone, Default)]
pub struct EventBus {
    handlers: Arc<RwLock<Vec<Handler>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deliver `event` to all subscribers.
    pub fn publish(&self, event: AppEvent) {
        let handlers = self
            .handlers
            .read()
            .expect("event bus handlers lock poisoned");
        for handler in handlers.iter() {
            handler(&event);
        }
    }

    /// Subscribe to every event.
    pub fn subscribe<F>(&self, handler: F)
    where
        F: Fn(&AppEvent) + Send + Sync + 'static,
    {
        self.handlers
            .write()
            .expect("event bus handlers lock poisoned")
            .push(Box::new(handler));
    }

    /// Subscribe only when `event.event_type` matches `event_type`.
    pub fn subscribe_type<F>(&self, event_type: &str, handler: F)
    where
        F: Fn(&AppEvent) + Send + Sync + 'static,
    {
        let filter = event_type.to_string();
        self.subscribe(move |event| {
            if event.event_type == filter {
                handler(event);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::contracts::{event_types, AppEvent};

    #[test]
    fn publish_delivers_to_subscriber() {
        let bus = EventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_c = Arc::clone(&seen);

        bus.subscribe(move |ev| {
            seen_c.lock().unwrap().push(ev.event_type.clone());
        });

        bus.publish(AppEvent::new(
            event_types::FILE_SAVED,
            "M03",
            json!({ "path": "main.c" }),
        ));

        assert_eq!(*seen.lock().unwrap(), vec![event_types::FILE_SAVED.to_string()]);
    }

    #[test]
    fn subscribe_type_filters_by_event_type() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0u32));
        let count_c = Arc::clone(&count);

        bus.subscribe_type(event_types::BUILD_COMPLETED, move |_| {
            *count_c.lock().unwrap() += 1;
        });

        bus.publish(AppEvent::new(
            event_types::FILE_SAVED,
            "M03",
            json!({}),
        ));
        bus.publish(AppEvent::new(
            event_types::BUILD_COMPLETED,
            "M06",
            json!({ "success": true }),
        ));

        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn app_event_includes_schema_version_and_correlation_id() {
        let ev = AppEvent::new(event_types::PROJECT_OPENED, "M02", json!({}))
            .with_correlation_id("req-1");
        assert_eq!(ev.schema_version, 1);
        assert_eq!(ev.correlation_id.as_deref(), Some("req-1"));
    }
}
