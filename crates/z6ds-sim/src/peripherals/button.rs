//! M11 button plugin — USER / netlist button → PA0 drive.

pub struct ButtonPlugin {
    instance_id: String,
    mcu_pin: String,
}

impl ButtonPlugin {
    pub fn new(instance_id: &str, pins: &[String]) -> Self {
        let mcu_pin = pins.first().cloned().unwrap_or_else(|| "PA0".to_string());
        Self {
            instance_id: instance_id.to_string(),
            mcu_pin,
        }
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn mcu_pin(&self) -> &str {
        &self.mcu_pin
    }

    pub fn matches_pin(&self, target: &str) -> bool {
        target == self.mcu_pin || target == "USER" || target == self.instance_id
    }
}
