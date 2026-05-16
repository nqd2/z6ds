//! Shared JSON contracts (camelCase serde).

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION_APP_EVENT: u32 = 1;
pub const SCHEMA_VERSION_BOARD_CONFIG: u32 = 1;
pub const SCHEMA_VERSION_DISCOVER_REQUEST: u32 = 1;
pub const SCHEMA_VERSION_DISCOVERY_RESULT: u32 = 1;
pub const SCHEMA_VERSION_PROJECT_MANIFEST: u32 = 1;
pub const SCHEMA_VERSION_BUILD_TARGET: u32 = 1;
pub const SCHEMA_VERSION_ELF_CANDIDATE: u32 = 1;
pub const SCHEMA_VERSION_BUILD: u32 = 1;
pub const SCHEMA_VERSION_SIMULATOR: u32 = 1;
pub const SCHEMA_VERSION_GPIO: u32 = 1;
pub const SCHEMA_VERSION_UART_CHUNK: u32 = 1;
pub const SCHEMA_VERSION_BRIDGE: u32 = 1;
pub const SCHEMA_VERSION_PERIPHERAL_VISUAL: u32 = 1;

/// Catalog event types from modules/00-INDEX (AGENTS.md).
pub mod event_types {
    pub const PROJECT_OPENED: &str = "project.opened";
    pub const PROJECT_REFRESHED: &str = "project.refreshed";
    pub const PROJECT_DISCOVERY_FAILED: &str = "project.discovery.failed";
    pub const BUILD_STARTED: &str = "build.started";
    pub const BUILD_LOG: &str = "build.log";
    pub const BUILD_COMPLETED: &str = "build.completed";
    pub const BUILD_CANCELLED: &str = "build.cancelled";
    pub const TOOLCHAIN_DETECTED: &str = "toolchain.detected";
    pub const SIMULATOR_STARTING: &str = "simulator.starting";
    pub const SIMULATOR_STARTED: &str = "simulator.started";
    pub const SIMULATOR_STOPPED: &str = "simulator.stopped";
    pub const SIMULATOR_ERROR: &str = "simulator.error";
    pub const SIMULATOR_RESET: &str = "simulator.reset";
    pub const SIMULATOR_PAUSED: &str = "simulator.paused";
    pub const SIMULATOR_RESUMED: &str = "simulator.resumed";
    pub const GPIO_CHANGED: &str = "gpio.changed";
    pub const PERIPHERAL_VISUAL_CHANGED: &str = "peripheral.visual.changed";
    pub const UART_RX: &str = "uart.rx";
    pub const UART_TX: &str = "uart.tx";
    pub const UART_BRIDGE_CONNECTED: &str = "uart.bridge.connected";
    pub const UART_BRIDGE_DISCONNECTED: &str = "uart.bridge.disconnected";
    pub const UART_BRIDGE_ERROR: &str = "uart.bridge.error";
    pub const UART_ENGINE_TX: &str = "uart.engine.tx";
    pub const UART_ENGINE_RX: &str = "uart.engine.rx";
    pub const NETLIST_CHANGED: &str = "netlist.changed";
    pub const FILE_SAVED: &str = "file.saved";
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppEvent {
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub source: String,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl AppEvent {
    pub fn new(
        event_type: impl Into<String>,
        source: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_APP_EVENT,
            event_type: event_type.into(),
            source: source.into(),
            payload,
            correlation_id: None,
        }
    }

    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoardConfig {
    pub schema_version: u32,
    pub mcu_id: String,
    pub board_id: String,
    pub pins: Vec<PinConfig>,
    pub clock: ClockConfig,
    pub uart_profiles: Vec<UartPortConfig>,
}

impl BoardConfig {
    pub fn lab_disc1_defaults() -> Self {
        Self {
            schema_version: SCHEMA_VERSION_BOARD_CONFIG,
            mcu_id: "STM32F429ZIT6".into(),
            board_id: "STM32F429I-DISC1".into(),
            pins: vec![
                pin("PG", 13, Some("LED3"), Some("GPIO_Output")),
                pin("PG", 14, Some("LED4"), Some("GPIO_Output")),
                pin("PA", 0, Some("USER"), Some("GPIO_EXTI0")),
                pin("PD", 14, Some("HC-SR04_TRIG"), Some("GPIO_Output")),
                pin("PD", 15, Some("HC-SR04_ECHO"), Some("GPIO_Input")),
                pin("PA", 9, Some("USART1_TX"), Some("USART1_TX")),
                pin("PA", 10, Some("USART1_RX"), Some("USART1_RX")),
            ],
            clock: ClockConfig {
                sysclk_hz: 180_000_000,
                apb1_hz: 90_000_000,
                apb2_hz: 90_000_000,
            },
            uart_profiles: vec![UartPortConfig {
                peripheral: "USART1".into(),
                baud_rate: 115_200,
                data_bits: 8,
                parity: "none".into(),
                stop_bits: 1,
                tx_pin: "PA9".into(),
                rx_pin: "PA10".into(),
            }],
        }
    }
}

fn pin(port: &str, pin: u8, label: Option<&str>, signal: Option<&str>) -> PinConfig {
    PinConfig {
        port: port.into(),
        pin,
        label: label.map(str::to_string),
        signal: signal.map(str::to_string),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinConfig {
    pub port: String,
    pub pin: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

impl PinConfig {
    pub fn pin_id(&self) -> String {
        format!("{}{}", self.port, self.pin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClockConfig {
    pub sysclk_hz: u64,
    pub apb1_hz: u64,
    pub apb2_hz: u64,
}

// --- M06 build contracts ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildRequest {
    pub schema_version: u32,
    pub project_root: String,
    pub target: String,
    pub clean: bool,
    #[serde(default)]
    pub environment: std::collections::HashMap<String, String>,
}

impl BuildRequest {
    pub fn new(project_root: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_BUILD,
            project_root: project_root.into(),
            target: target.into(),
            clean: false,
            environment: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanRequest {
    pub schema_version: u32,
    pub project_root: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CancelBuildRequest {
    pub schema_version: u32,
    pub build_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureToolchainRequest {
    pub schema_version: u32,
    pub make_path: String,
    pub gcc_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cube_ide_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainInfo {
    pub schema_version: u32,
    pub make_path: String,
    pub gcc_path: String,
    pub version: String,
    pub detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildStarted {
    pub schema_version: u32,
    pub build_id: String,
    pub project_root: String,
    pub target: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildLogChunk {
    pub schema_version: u32,
    pub build_id: String,
    pub stream: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub schema_version: u32,
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub severity: String,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    pub schema_version: u32,
    pub build_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elf_path: Option<String>,
    pub duration_ms: u64,
    pub log_text: String,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UartPortConfig {
    pub peripheral: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub tx_pin: String,
    pub rx_pin: String,
}

// --- M02 Project Discovery contracts ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverRequest {
    pub schema_version: u32,
    pub root_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl DiscoverRequest {
    pub fn new(root_path: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_DISCOVER_REQUEST,
            root_path: root_path.into(),
            correlation_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefreshProjectRequest {
    pub schema_version: u32,
    pub root_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidateManifestRequest {
    pub schema_version: u32,
    pub manifest: ProjectManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryStatus {
    Success,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryIssue {
    pub code: String,
    pub message: String,
}

impl DiscoveryIssue {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub schema_version: u32,
    pub status: DiscoveryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ProjectManifest>,
    pub errors: Vec<DiscoveryIssue>,
    pub warnings: Vec<DiscoveryIssue>,
}

impl DiscoveryResult {
    pub fn failed(errors: Vec<DiscoveryIssue>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_DISCOVERY_RESULT,
            status: DiscoveryStatus::Failed,
            manifest: None,
            errors,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub root_path: String,
    pub project_name: String,
    pub mcu_id: String,
    pub board_id: String,
    pub ioc_path: String,
    pub build_targets: Vec<BuildTarget>,
    pub elf_candidates: Vec<ElfCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildTarget {
    pub schema_version: u32,
    pub name: String,
    pub makefile_path: String,
    pub working_directory: String,
    pub artifact_glob: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElfCandidate {
    pub schema_version: u32,
    pub path: String,
    pub target: String,
    pub mtime: u64,
    pub size_bytes: u64,
}

// --- M08/M09 simulator contracts ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionOptions {
    pub schema_version: u32,
    #[serde(default = "default_true")]
    pub reset_on_start: bool,
    #[serde(default)]
    pub enable_gdb: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION_SIMULATOR,
            reset_on_start: true,
            enable_gdb: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorRunRequest {
    pub schema_version: u32,
    pub elf_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netlist_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_config: Option<BoardConfig>,
    #[serde(default)]
    pub session_options: SessionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorSessionState {
    pub schema_version: u32,
    pub session_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elf_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl SimulatorSessionState {
    pub fn idle() -> Self {
        Self {
            schema_version: SCHEMA_VERSION_SIMULATOR,
            session_id: String::new(),
            status: "idle".into(),
            elf_path: None,
            started_at: None,
            message: None,
            error_code: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoardInteractionRequest {
    pub schema_version: u32,
    pub session_id: String,
    #[serde(rename = "type")]
    pub interaction_type: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpioStateMap {
    pub schema_version: u32,
    pub session_id: String,
    pub pins: std::collections::HashMap<String, u8>,
    pub virtual_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PwmSignalUpdate {
    pub schema_version: u32,
    pub session_id: String,
    pub pin_id: String,
    pub frequency_hz: f64,
    pub duty_cycle: f64,
    pub virtual_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UartStreamChunk {
    pub schema_version: u32,
    pub session_id: String,
    pub port_id: String,
    pub direction: String,
    pub bytes_base64: String,
    pub timestamp: String,
    pub virtual_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeUartConfig {
    pub schema_version: u32,
    pub port_id: String,
    pub baud: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
}

impl From<&UartPortConfig> for BridgeUartConfig {
    fn from(c: &UartPortConfig) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_BRIDGE,
            port_id: c.peripheral.clone(),
            baud: c.baud_rate,
            data_bits: c.data_bits,
            parity: c.parity.clone(),
            stop_bits: c.stop_bits,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HostSendBytes {
    pub schema_version: u32,
    pub session_id: String,
    pub port_id: String,
    pub bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub schema_version: u32,
    pub session_id: String,
    pub port_id: String,
    pub connected: bool,
    pub bytes_rx: u64,
    pub bytes_tx: u64,
    pub config: BridgeUartConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinDriveRequest {
    pub schema_version: u32,
    pub session_id: String,
    pub pin_id: String,
    pub level: u8,
    pub drive: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralVisualState {
    pub schema_version: u32,
    pub instance_id: String,
    pub module_type: String,
    pub state: serde_json::Value,
    pub virtual_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralTelemetry {
    pub schema_version: u32,
    pub instance_id: String,
    pub kind: String,
    pub value: f64,
    pub unit: String,
    pub virtual_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EngineHealth {
    pub schema_version: u32,
    pub session_id: String,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

#[cfg(test)]
mod simulator_contract_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn simulator_run_request_roundtrip() {
        let req = SimulatorRunRequest {
            schema_version: SCHEMA_VERSION_SIMULATOR,
            elf_path: "/tmp/test.elf".into(),
            netlist_ref: None,
            board_config: Some(BoardConfig::lab_disc1_defaults()),
            session_options: SessionOptions::default(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: SimulatorRunRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.elf_path, req.elf_path);
        assert_eq!(back.schema_version, 1);
    }

    #[test]
    fn gpio_state_map_roundtrip() {
        let mut pins = HashMap::new();
        pins.insert("PG13".into(), 1);
        let map = GpioStateMap {
            schema_version: SCHEMA_VERSION_GPIO,
            session_id: "sim-1".into(),
            pins,
            virtual_time_ns: 1000,
        };
        let json = serde_json::to_string(&map).unwrap();
        let back: GpioStateMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pins.get("PG13"), Some(&1));
    }

    #[test]
    fn uart_stream_chunk_roundtrip() {
        let chunk = UartStreamChunk {
            schema_version: SCHEMA_VERSION_UART_CHUNK,
            session_id: "sim-1".into(),
            port_id: "USART1".into(),
            direction: "tx".into(),
            bytes_base64: "SGVsbG8=".into(),
            timestamp: "2026-05-16T00:00:00Z".into(),
            virtual_time_ns: 0,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("bytesBase64"));
        let back: UartStreamChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(back.port_id, "USART1");
    }
}
