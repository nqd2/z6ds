//! M11 — peripheral plugin host.

mod button;
mod hc_sr04;
mod led;

use std::collections::HashMap;

use serde_json::json;
use z6ds_core::contracts::{
    event_types, AppEvent, GpioStateMap, PinDriveRequest, SCHEMA_VERSION_SIMULATOR,
};
use z6ds_core::netlist::NetlistDocument;
use z6ds_core::EventBus;

pub use button::ButtonPlugin;
pub use hc_sr04::HcSr04Plugin;
pub use led::LedPlugin;

pub struct PeripheralHost {
    leds: HashMap<String, LedPlugin>,
    buttons: HashMap<String, ButtonPlugin>,
    hcsr04: HashMap<String, HcSr04Plugin>,
}

impl PeripheralHost {
    pub fn new() -> Self {
        Self {
            leds: HashMap::new(),
            buttons: HashMap::new(),
            hcsr04: HashMap::new(),
        }
    }

    pub fn sync_netlist(&mut self, doc: &NetlistDocument) {
        self.leds.clear();
        self.buttons.clear();
        self.hcsr04.clear();
        for m in &doc.modules {
            match m.module_type.as_str() {
                "led" => {
                    self.leds
                        .insert(m.instance_id.clone(), LedPlugin::new(&m.instance_id, &m.pins));
                }
                "button" => {
                    self.buttons.insert(
                        m.instance_id.clone(),
                        ButtonPlugin::new(&m.instance_id, &m.pins),
                    );
                }
                "hc-sr04" => {
                    self.hcsr04.insert(
                        m.instance_id.clone(),
                        HcSr04Plugin::new(&m.instance_id, &m.pins),
                    );
                }
                _ => {}
            }
        }
    }

    pub fn on_gpio(&self, bus: &EventBus, map: &GpioStateMap) {
        for led in self.leds.values() {
            if let Some(state) = led.visual_from_gpio(map) {
                bus.publish(AppEvent::new(
                    event_types::PERIPHERAL_VISUAL_CHANGED,
                    "M11",
                    serde_json::to_value(&state).unwrap_or(json!({})),
                ));
            }
        }
        for sensor in self.hcsr04.values() {
            if let Some(state) = sensor.visual_from_gpio(map) {
                bus.publish(AppEvent::new(
                    event_types::PERIPHERAL_VISUAL_CHANGED,
                    "M11",
                    serde_json::to_value(&state).unwrap_or(json!({})),
                ));
            }
        }
    }

    pub fn on_button_press(
        &self,
        session_id: &str,
        target_pin: &str,
    ) -> Option<PinDriveRequest> {
        for btn in self.buttons.values() {
            if btn.matches_pin(target_pin) {
                return Some(PinDriveRequest {
                    schema_version: SCHEMA_VERSION_SIMULATOR,
                    session_id: session_id.into(),
                    pin_id: btn.mcu_pin().to_string(),
                    level: 0,
                    drive: "external".into(),
                    source_instance_id: Some(btn.instance_id().to_string()),
                });
            }
        }
        if target_pin == "PA0" || target_pin == "USER" {
            return Some(PinDriveRequest {
                schema_version: SCHEMA_VERSION_SIMULATOR,
                session_id: session_id.into(),
                pin_id: "PA0".into(),
                level: 0,
                drive: "external".into(),
                source_instance_id: Some("USER".into()),
            });
        }
        None
    }

    pub fn on_button_release(&self, session_id: &str, target_pin: &str) -> Option<PinDriveRequest> {
        let mut req = self.on_button_press(session_id, target_pin)?;
        req.level = 1;
        Some(req)
    }
}
