//! M11 HC-SR04 — basic trig/echo visual from PD14/PD15 GPIO.

use z6ds_core::contracts::{GpioStateMap, PeripheralVisualState, SCHEMA_VERSION_PERIPHERAL_VISUAL};

pub struct HcSr04Plugin {
    instance_id: String,
    trig_pin: String,
    echo_pin: String,
    distance_cm: f64,
}

impl HcSr04Plugin {
    pub fn new(instance_id: &str, pins: &[String]) -> Self {
        let trig_pin = pins.first().cloned().unwrap_or_else(|| "PD14".into());
        let echo_pin = pins.get(1).cloned().unwrap_or_else(|| "PD15".into());
        Self {
            instance_id: instance_id.to_string(),
            trig_pin,
            echo_pin,
            distance_cm: 25.0,
        }
    }

    pub fn visual_from_gpio(&self, map: &GpioStateMap) -> Option<PeripheralVisualState> {
        let _trig = map.pins.get(&self.trig_pin).copied();
        let _echo = map.pins.get(&self.echo_pin).copied();
        Some(PeripheralVisualState {
            schema_version: SCHEMA_VERSION_PERIPHERAL_VISUAL,
            instance_id: self.instance_id.clone(),
            module_type: "hc-sr04".into(),
            state: serde_json::json!({
                "distanceCm": self.distance_cm,
                "trig": self.trig_pin,
                "echo": self.echo_pin,
            }),
            virtual_time_ns: map.virtual_time_ns,
        })
    }
}
