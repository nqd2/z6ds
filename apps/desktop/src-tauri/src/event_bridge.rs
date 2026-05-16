//! M00 bridge — forward in-process `EventBus` events to the Tauri webview.

use tauri::{AppHandle, Emitter};
use z6ds_core::EventBus;

/// Subscribe on `bus` and emit each `AppEvent` to the frontend as `app-event`.
pub fn attach_event_bridge(app: AppHandle, bus: &EventBus) {
    bus.subscribe(move |event| {
        if let Err(err) = app.emit("app-event", event) {
            eprintln!("event bridge: failed to emit app-event: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z6ds_core::contracts::{event_types, AppEvent};

    #[test]
    fn app_event_serializes_type_field() {
        let event = AppEvent::new(event_types::BUILD_LOG, "M06", json!({ "text": "ok" }));
        let v = serde_json::to_value(&event).expect("serialize");
        assert_eq!(v.get("type").and_then(|t| t.as_str()), Some("build.log"));
        assert_eq!(v.get("schemaVersion").and_then(|t| t.as_u64()), Some(1));
    }
}
