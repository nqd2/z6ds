//! M11 LED plugin — map GPIO level to visual on/off.

use z6ds_core::contracts::{GpioStateMap, PeripheralVisualState, SCHEMA_VERSION_PERIPHERAL_VISUAL};

pub struct LedPlugin {
    instance_id: String,
    gpio_pin: String,
}

impl LedPlugin {
    pub fn new(instance_id: &str, pins: &[String]) -> Self {
        let gpio_pin = pins
            .first()
            .cloned()
            .unwrap_or_else(|| "PG13".to_string());
        Self {
            instance_id: instance_id.to_string(),
            gpio_pin,
        }
    }

    pub fn visual_from_gpio(&self, map: &GpioStateMap) -> Option<PeripheralVisualState> {
        let level = map.pins.get(&self.gpio_pin).copied().unwrap_or(0);
        Some(PeripheralVisualState {
            schema_version: SCHEMA_VERSION_PERIPHERAL_VISUAL,
            instance_id: self.instance_id.clone(),
            module_type: "led".into(),
            state: serde_json::json!({ "on": level != 0 }),
            virtual_time_ns: map.virtual_time_ns,
        })
    }
}
