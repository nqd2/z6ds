//! M13 — UART bridge (M09 ↔ M14).

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde_json::json;
use z6ds_core::contracts::{
    event_types, AppEvent, BridgeStatus, BridgeUartConfig, HostSendBytes, UartPortConfig,
    SCHEMA_VERSION_BRIDGE, SCHEMA_VERSION_UART_CHUNK,
};
use z6ds_core::EventBus;

struct PortState {
    config: BridgeUartConfig,
    bytes_rx: u64,
    bytes_tx: u64,
    connected: bool,
}

pub struct UartBridge {
    bus: EventBus,
    ports: Mutex<HashMap<String, PortState>>,
}

impl UartBridge {
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            ports: Mutex::new(HashMap::new()),
        }
    }

    pub fn attach_port(&self, session_id: &str, config: &UartPortConfig) -> Result<BridgeStatus> {
        let bridge_cfg = BridgeUartConfig::from(config);
        let key = format!("{session_id}:{}", bridge_cfg.port_id);
        {
            let mut ports = self.ports.lock().unwrap();
            ports.insert(
                key,
                PortState {
                    config: bridge_cfg.clone(),
                    bytes_rx: 0,
                    bytes_tx: 0,
                    connected: true,
                },
            );
        }
        let status = BridgeStatus {
            schema_version: SCHEMA_VERSION_BRIDGE,
            session_id: session_id.into(),
            port_id: bridge_cfg.port_id.clone(),
            connected: true,
            bytes_rx: 0,
            bytes_tx: 0,
            config: bridge_cfg,
        };
        self.bus.publish(AppEvent::new(
            event_types::UART_BRIDGE_CONNECTED,
            "M13",
            serde_json::to_value(&status).unwrap_or(json!({})),
        ));
        Ok(status)
    }

    pub fn detach_all(&self) {
        let mut ports = self.ports.lock().unwrap();
        for (key, _) in ports.drain() {
            let parts: Vec<_> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                self.bus.publish(AppEvent::new(
                    event_types::UART_BRIDGE_DISCONNECTED,
                    "M13",
                    json!({
                        "schemaVersion": SCHEMA_VERSION_BRIDGE,
                        "sessionId": parts[0],
                        "portId": parts[1],
                    }),
                ));
            }
        }
    }

    pub fn on_engine_tx(&self, session_id: &str, port_id: &str, bytes: &[u8]) {
        let key = format!("{session_id}:{port_id}");
        if let Ok(mut ports) = self.ports.lock() {
            if let Some(p) = ports.get_mut(&key) {
                p.bytes_rx += bytes.len() as u64;
            }
        }
        let chunk = z6ds_core::contracts::UartStreamChunk {
            schema_version: SCHEMA_VERSION_UART_CHUNK,
            session_id: session_id.into(),
            port_id: port_id.into(),
            direction: "rx".into(),
            bytes_base64: B64.encode(bytes),
            timestamp: format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()),
            virtual_time_ns: 0,
        };
        self.bus.publish(AppEvent::new(
            event_types::UART_RX,
            "M13",
            serde_json::to_value(&chunk).unwrap_or(json!({})),
        ));
    }

    pub fn host_send(&self, session_id: &str, port_id: &str, bytes: &[u8]) -> Result<()> {
        let key = format!("{session_id}:{port_id}");
        let mut ports = self.ports.lock().unwrap();
        let port = ports
            .get_mut(&key)
            .ok_or_else(|| anyhow!("port_not_found: {port_id}"))?;
        if !port.connected {
            return Err(anyhow!("bridge_closed"));
        }
        port.bytes_tx += bytes.len() as u64;
        let chunk = z6ds_core::contracts::UartStreamChunk {
            schema_version: SCHEMA_VERSION_UART_CHUNK,
            session_id: session_id.into(),
            port_id: port_id.into(),
            direction: "tx".into(),
            bytes_base64: B64.encode(bytes),
            timestamp: format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()),
            virtual_time_ns: 0,
        };
        self.bus.publish(AppEvent::new(
            event_types::UART_TX,
            "M13",
            serde_json::to_value(&chunk).unwrap_or(json!({})),
        ));
        Ok(())
    }

    pub fn status(&self, session_id: &str, port_id: &str) -> Option<BridgeStatus> {
        let key = format!("{session_id}:{port_id}");
        let ports = self.ports.lock().ok()?;
        let p = ports.get(&key)?;
        Some(BridgeStatus {
            schema_version: SCHEMA_VERSION_BRIDGE,
            session_id: session_id.into(),
            port_id: port_id.into(),
            connected: p.connected,
            bytes_rx: p.bytes_rx,
            bytes_tx: p.bytes_tx,
            config: p.config.clone(),
        })
    }

    pub fn handle_host_send(&self, req: &HostSendBytes) -> Result<()> {
        let bytes = B64
            .decode(&req.bytes_base64)
            .map_err(|e| anyhow!("encoding_error: {e}"))?;
        self.host_send(&req.session_id, &req.port_id, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z6ds_core::contracts::UartPortConfig;

    #[test]
    fn attach_and_status() {
        let bus = EventBus::new();
        let bridge = UartBridge::new(bus);
        let cfg = UartPortConfig {
            peripheral: "USART1".into(),
            baud_rate: 115_200,
            data_bits: 8,
            parity: "none".into(),
            stop_bits: 1,
            tx_pin: "PA9".into(),
            rx_pin: "PA10".into(),
        };
        let st = bridge.attach_port("sim-1", &cfg).unwrap();
        assert!(st.connected);
    }
}
