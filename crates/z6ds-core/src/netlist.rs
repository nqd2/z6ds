//! M10 — hardware netlist document, validation, board defaults.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::contracts::{event_types, AppEvent, BoardConfig, PinConfig};
use crate::EventBus;

pub const SCHEMA_VERSION_NETLIST: u32 = 1;
pub const SCHEMA_VERSION_VALIDATION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetlistDocument {
    pub schema_version: u32,
    pub board: String,
    pub modules: Vec<NetlistModule>,
    pub wires: Vec<NetlistWire>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetlistModule {
    pub schema_version: u32,
    pub instance_id: String,
    pub module_type: String,
    pub pins: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<NetlistPosition>,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetlistPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct NetlistPinRef {
    pub node_id: String,
    pub pin_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetlistWire {
    pub schema_version: u32,
    pub wire_id: String,
    pub from: NetlistPinRef,
    pub to: NetlistPinRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub schema_version: u32,
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinBindingMap {
    pub schema_version: u32,
    pub bindings: HashMap<String, Vec<NetlistPinRef>>,
}

impl NetlistDocument {
    pub fn empty(board: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_NETLIST,
            board: board.into(),
            modules: Vec::new(),
            wires: Vec::new(),
            metadata: json!({}),
        }
    }
}

/// In-memory netlist with EventBus notifications.
#[derive(Clone)]
pub struct NetlistStore {
    document: NetlistDocument,
    bus: EventBus,
}

impl NetlistStore {
    pub fn new(bus: EventBus) -> Self {
        Self {
            document: NetlistDocument::empty("STM32F429I-DISC1"),
            bus,
        }
    }

    pub fn document(&self) -> &NetlistDocument {
        &self.document
    }

    pub fn set_document(&mut self, doc: NetlistDocument) {
        self.document = doc;
        self.publish_changed();
    }

    pub fn apply_board_defaults(&mut self, board: &BoardConfig) -> &NetlistDocument {
        self.document = build_defaults_from_board(board);
        self.publish_changed();
        &self.document
    }

    pub fn add_module(&mut self, module: NetlistModule) -> Result<(), ValidationIssue> {
        if self
            .document
            .modules
            .iter()
            .any(|m| m.instance_id == module.instance_id)
        {
            return Err(ValidationIssue {
                code: "duplicate_instance".into(),
                message: format!("module {} already exists", module.instance_id),
                wire_id: None,
                instance_id: Some(module.instance_id.clone()),
            });
        }
        self.document.modules.push(module);
        self.publish_changed();
        Ok(())
    }

    pub fn remove_module(&mut self, instance_id: &str) -> bool {
        let before = self.document.modules.len();
        self.document
            .modules
            .retain(|m| m.instance_id != instance_id);
        if self.document.modules.len() == before {
            return false;
        }
        self.document
            .wires
            .retain(|w| w.from.node_id != instance_id && w.to.node_id != instance_id);
        self.publish_changed();
        true
    }

    pub fn add_wire(&mut self, wire: NetlistWire) -> Result<(), ValidationIssue> {
        if let Some(issue) = self.validate_wire_topology(&wire) {
            return Err(issue);
        }
        if self.document.wires.iter().any(|w| w.wire_id == wire.wire_id) {
            return Err(ValidationIssue {
                code: "duplicate_wire_id".into(),
                message: format!("wire {} already exists", wire.wire_id),
                wire_id: Some(wire.wire_id.clone()),
                instance_id: None,
            });
        }
        if self
            .document
            .wires
            .iter()
            .any(|w| same_connection(w, &wire))
        {
            return Err(ValidationIssue {
                code: "duplicate_wire".into(),
                message: "identical connection already exists".into(),
                wire_id: None,
                instance_id: None,
            });
        }
        self.document.wires.push(wire);
        self.publish_changed();
        Ok(())
    }

    pub fn remove_wire(&mut self, wire_id: &str) -> bool {
        let before = self.document.wires.len();
        self.document.wires.retain(|w| w.wire_id != wire_id);
        if before != self.document.wires.len() {
            self.publish_changed();
            true
        } else {
            false
        }
    }

    pub fn validate(&self, rules: &[&str]) -> ValidationResult {
        let mut issues = Vec::new();
        let mvp = rules.is_empty() || rules.iter().any(|r| *r == "mvp");
        let rail_safety = rules.iter().any(|r| *r == "railSafety");

        for wire in &self.document.wires {
            if let Some(issue) = self.validate_wire_topology(wire) {
                issues.push(issue);
            }
        }

        if mvp {
            for wire in &self.document.wires {
                let dupes: Vec<_> = self
                    .document
                    .wires
                    .iter()
                    .filter(|w| same_connection(w, wire))
                    .collect();
                if dupes.len() > 1 {
                    issues.push(ValidationIssue {
                        code: "duplicate_wire".into(),
                        message: format!("duplicate connection on wire {}", wire.wire_id),
                        wire_id: Some(wire.wire_id.clone()),
                        instance_id: None,
                    });
                }
            }
        }

        if rail_safety {
            for wire in &self.document.wires {
                if is_rail_short(&wire.from.pin_id, &wire.to.pin_id) {
                    issues.push(ValidationIssue {
                        code: "rail_short".into(),
                        message: format!(
                            "rail short between {} and {}",
                            wire.from.pin_id, wire.to.pin_id
                        ),
                        wire_id: Some(wire.wire_id.clone()),
                        instance_id: None,
                    });
                }
            }
        }

        issues.sort_by(|a, b| a.code.cmp(&b.code));
        issues.dedup_by(|a, b| a.code == b.code && a.message == b.message);

        ValidationResult {
            schema_version: SCHEMA_VERSION_VALIDATION,
            valid: issues.is_empty(),
            issues,
        }
    }

    pub fn pin_binding_map(&self) -> PinBindingMap {
        let mut bindings: HashMap<String, Vec<NetlistPinRef>> = HashMap::new();
        for wire in &self.document.wires {
            if wire.from.node_id == "mcu" || wire.from.node_id == "board" {
                bindings
                    .entry(wire.from.pin_id.clone())
                    .or_default()
                    .push(wire.to.clone());
            }
            if wire.to.node_id == "mcu" || wire.to.node_id == "board" {
                bindings
                    .entry(wire.to.pin_id.clone())
                    .or_default()
                    .push(wire.from.clone());
            }
        }
        PinBindingMap {
            schema_version: SCHEMA_VERSION_NETLIST,
            bindings,
        }
    }

    fn publish_changed(&self) {
        let payload = serde_json::to_value(&self.document).unwrap_or(json!({}));
        self.bus.publish(AppEvent::new(
            event_types::NETLIST_CHANGED,
            "M10",
            payload,
        ));
    }

    fn validate_wire_topology(&self, wire: &NetlistWire) -> Option<ValidationIssue> {
        if !self.module_has_pin(&wire.from) {
            return Some(ValidationIssue {
                code: "unknown_pin".into(),
                message: format!(
                    "unknown source {}.{}",
                    wire.from.node_id, wire.from.pin_id
                ),
                wire_id: Some(wire.wire_id.clone()),
                instance_id: Some(wire.from.node_id.clone()),
            });
        }
        if !self.module_has_pin(&wire.to) {
            return Some(ValidationIssue {
                code: "unknown_pin".into(),
                message: format!("unknown target {}.{}", wire.to.node_id, wire.to.pin_id),
                wire_id: Some(wire.wire_id.clone()),
                instance_id: Some(wire.to.node_id.clone()),
            });
        }
        None
    }

    fn module_has_pin(&self, pin_ref: &NetlistPinRef) -> bool {
        let mcu_pins = mcu_pin_ids();
        if pin_ref.node_id == "mcu" {
            return mcu_pins.contains(&pin_ref.pin_id);
        }
        let Some(module) = self
            .document
            .modules
            .iter()
            .find(|m| m.instance_id == pin_ref.node_id)
        else {
            return false;
        };
        module.pins.iter().any(|p| p == &pin_ref.pin_id)
    }
}

fn same_connection(a: &NetlistWire, b: &NetlistWire) -> bool {
    (a.from == b.from && a.to == b.to) || (a.from == b.to && a.to == b.from)
}

fn is_rail_short(a: &str, b: &str) -> bool {
    let rails = ["VCC", "3V3", "5V", "GND"];
    rails.contains(&a) && rails.contains(&b) && a != b
}

fn mcu_pin_ids() -> HashSet<String> {
    [
        "PG13", "PG14", "PA0", "PA9", "PA10", "PD14", "PD15", "PE2", "PE4", "PE5", "PE6",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn module(
    instance_id: &str,
    module_type: &str,
    pins: &[&str],
    position: Option<NetlistPosition>,
) -> NetlistModule {
    NetlistModule {
        schema_version: SCHEMA_VERSION_NETLIST,
        instance_id: instance_id.into(),
        module_type: module_type.into(),
        pins: pins.iter().map(|p| (*p).to_string()).collect(),
        position,
        parameters: json!({}),
    }
}

fn wire(
    wire_id: &str,
    from_node: &str,
    from_pin: &str,
    to_node: &str,
    to_pin: &str,
    color: Option<&str>,
    role: Option<&str>,
) -> NetlistWire {
    NetlistWire {
        schema_version: SCHEMA_VERSION_NETLIST,
        wire_id: wire_id.into(),
        from: NetlistPinRef {
            node_id: from_node.into(),
            pin_id: from_pin.into(),
        },
        to: NetlistPinRef {
            node_id: to_node.into(),
            pin_id: to_pin.into(),
        },
        color: color.map(str::to_string),
        signal_role: role.map(str::to_string),
    }
}

/// Build DISC1 lab default topology from `BoardConfig`.
pub fn build_defaults_from_board(board: &BoardConfig) -> NetlistDocument {
    let board_name = if board.board_id.is_empty() {
        "STM32F429I-DISC1".to_string()
    } else {
        board.board_id.clone()
    };

    let mut doc = NetlistDocument::empty(board_name);
    doc.modules.push(module(
        "mcu",
        "board-disc1",
        &[
            "PG13", "PG14", "PA0", "PA9", "PA10", "PD14", "PD15", "RESET", "USER",
        ],
        None,
    ));
    doc.modules.push(module("led-1", "led", &["anode", "cathode"], None));
    doc.modules.push(module("led-2", "led", &["anode", "cathode"], None));
    doc.modules.push(module("user-btn", "button", &["a", "b"], None));
    doc.modules.push(module(
        "hc-sr04-1",
        "hc-sr04",
        &["vcc", "trig", "echo", "gnd"],
        None,
    ));

    for pin in &board.pins {
        map_pin_to_defaults(&mut doc, pin);
    }

    if doc.wires.is_empty() {
        apply_static_lab_wires(&mut doc);
    }

    doc
}

fn apply_static_lab_wires(doc: &mut NetlistDocument) {
    doc.wires.push(wire(
        "w-led3",
        "mcu",
        "PG13",
        "led-1",
        "anode",
        Some("green"),
        Some("LED3"),
    ));
    doc.wires.push(wire(
        "w-led4",
        "mcu",
        "PG14",
        "led-2",
        "anode",
        Some("green"),
        Some("LED4"),
    ));
    doc.wires.push(wire(
        "w-user",
        "mcu",
        "PA0",
        "user-btn",
        "a",
        Some("yellow"),
        Some("USER"),
    ));
    doc.wires.push(wire(
        "w-hc-trig",
        "mcu",
        "PD14",
        "hc-sr04-1",
        "trig",
        Some("blue"),
        Some("HC-SR04_TRIG"),
    ));
    doc.wires.push(wire(
        "w-hc-echo",
        "mcu",
        "PD15",
        "hc-sr04-1",
        "echo",
        Some("blue"),
        Some("HC-SR04_ECHO"),
    ));
}

fn map_pin_to_defaults(doc: &mut NetlistDocument, pin: &PinConfig) {
    let pin_id = pin.pin_id();
    let label = pin.label.as_deref().unwrap_or("");
    match label {
        "LED3" => doc.wires.push(wire(
            &format!("w-{pin_id}-led1"),
            "mcu",
            &pin_id,
            "led-1",
            "anode",
            Some("green"),
            Some("LED3"),
        )),
        "LED4" => doc.wires.push(wire(
            &format!("w-{pin_id}-led2"),
            "mcu",
            &pin_id,
            "led-2",
            "anode",
            Some("green"),
            Some("LED4"),
        )),
        "USER" => doc.wires.push(wire(
            &format!("w-{pin_id}-user"),
            "mcu",
            &pin_id,
            "user-btn",
            "a",
            Some("yellow"),
            Some("USER"),
        )),
        "HC-SR04_TRIG" => doc.wires.push(wire(
            &format!("w-{pin_id}-trig"),
            "mcu",
            &pin_id,
            "hc-sr04-1",
            "trig",
            Some("blue"),
            Some("HC-SR04_TRIG"),
        )),
        "HC-SR04_ECHO" => doc.wires.push(wire(
            &format!("w-{pin_id}-echo"),
            "mcu",
            &pin_id,
            "hc-sr04-1",
            "echo",
            Some("blue"),
            Some("HC-SR04_ECHO"),
        )),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::BoardConfig;

    fn store() -> NetlistStore {
        NetlistStore::new(EventBus::new())
    }

    #[test]
    fn tc_m10_01_add_led_wire() {
        let mut s = store();
        s.apply_board_defaults(&BoardConfig::lab_disc1_defaults());
        let map = s.pin_binding_map();
        let bindings = map.bindings.get("PG13").expect("PG13 binding");
        assert!(bindings.iter().any(|b| b.node_id == "led-1" && b.pin_id == "anode"));
    }

    #[test]
    fn tc_m10_02_duplicate_rejected() {
        let mut s = store();
        s.apply_board_defaults(&BoardConfig::lab_disc1_defaults());
        let dup = wire("w-dup", "mcu", "PG13", "led-1", "anode", None, None);
        let err = s.add_wire(dup).expect_err("duplicate");
        assert_eq!(err.code, "duplicate_wire");
    }

    #[test]
    fn tc_m10_03_hc_sr04_lab_wiring_valid() {
        let mut s = store();
        s.apply_board_defaults(&BoardConfig::lab_disc1_defaults());
        let result = s.validate(&["mvp"]);
        assert!(result.valid, "{:?}", result.issues);
    }

    #[test]
    fn tc_m10_04_import_export_roundtrip() {
        let mut s = store();
        s.apply_board_defaults(&BoardConfig::lab_disc1_defaults());
        let exported = s.document().clone();
        let json = serde_json::to_string(&exported).unwrap();
        let imported: NetlistDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(exported, imported);
    }

    #[test]
    fn tc_m10_05_remove_module_cascades() {
        let bus = EventBus::new();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let seen_c = seen.clone();
        bus.subscribe(move |ev| {
            if ev.event_type == event_types::NETLIST_CHANGED {
                *seen_c.lock().unwrap() += 1;
            }
        });
        let mut s = NetlistStore::new(bus);
        s.apply_board_defaults(&BoardConfig::lab_disc1_defaults());
        let wires_before: usize = s.document().wires.len();
        assert!(wires_before > 0);
        assert!(s.remove_module("led-1"));
        assert!(!s.document().wires.iter().any(|w| w.to.node_id == "led-1"));
        assert!(*seen.lock().unwrap() >= 1);
    }

    #[test]
    fn apply_board_defaults_uses_board_config_schema() {
        let board = BoardConfig::lab_disc1_defaults();
        assert_eq!(board.schema_version, 1);
        let doc = build_defaults_from_board(&board);
        assert_eq!(doc.schema_version, SCHEMA_VERSION_NETLIST);
        assert_eq!(doc.board, "STM32F429I-DISC1");
    }
}
