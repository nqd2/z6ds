//! M08 — simulator session controller.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use base64::Engine;
use serde_json::json;
use uuid::Uuid;
use z6ds_core::contracts::{
    event_types, AppEvent, BoardConfig, BoardInteractionRequest, SimulatorRunRequest,
    SimulatorSessionState, SCHEMA_VERSION_SIMULATOR,
};
use z6ds_core::netlist::NetlistDocument;
use z6ds_core::EventBus;
use z6ds_uart::UartBridge;

use crate::engine::EmulationEngine;

fn port_pair(session_id: &str) -> (u16, u16) {
    let hash = session_id
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
    let base: u16 = 40_000 + (hash % 5000) as u16;
    (base, base.saturating_add(1))
}

struct SimulatorSession {
    state: SimulatorSessionState,
    engine: Option<EmulationEngine>,
    uart: UartBridge,
    board: BoardConfig,
}

pub struct SessionController {
    bus: EventBus,
    session: Mutex<Option<SimulatorSession>>,
}

impl SessionController {
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            session: Mutex::new(None),
        }
    }

    pub fn state(&self) -> SimulatorSessionState {
        self.session
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|x| x.state.clone()))
            .unwrap_or_else(SimulatorSessionState::idle)
    }

    fn publish_state(&self, source: &str) {
        let state = self.state();
        let payload = serde_json::to_value(&state).unwrap_or(json!({}));
        let event_type = match state.status.as_str() {
            "running" => event_types::SIMULATOR_STARTED,
            "stopped" | "idle" => event_types::SIMULATOR_STOPPED,
            "error" => event_types::SIMULATOR_ERROR,
            "starting" => event_types::SIMULATOR_STARTING,
            _ => event_types::SIMULATOR_STARTED,
        };
        self.bus
            .publish(AppEvent::new(event_type, source, payload));
    }

    pub fn run(
        &self,
        request: SimulatorRunRequest,
        netlist: Option<NetlistDocument>,
    ) -> Result<SimulatorSessionState> {
        let elf = Path::new(&request.elf_path);
        if !elf.is_file() {
            let err = SimulatorSessionState {
                schema_version: SCHEMA_VERSION_SIMULATOR,
                session_id: String::new(),
                status: "error".into(),
                elf_path: Some(request.elf_path.clone()),
                started_at: None,
                message: Some(format!("ELF not found: {}", request.elf_path)),
                error_code: Some("elf_not_found".into()),
            };
            self.bus.publish(AppEvent::new(
                event_types::SIMULATOR_ERROR,
                "M08",
                serde_json::to_value(&err).unwrap_or(json!({})),
            ));
            return Err(anyhow!("elf_not_found"));
        }

        let session_id = format!("sim-{}", Uuid::new_v4());
        let board = request
            .board_config
            .clone()
            .unwrap_or_else(BoardConfig::lab_disc1_defaults);

        let starting = SimulatorSessionState {
            schema_version: SCHEMA_VERSION_SIMULATOR,
            session_id: session_id.clone(),
            status: "starting".into(),
            elf_path: Some(request.elf_path.clone()),
            started_at: None,
            message: None,
            error_code: None,
        };
        {
            let mut guard = self.session.lock().unwrap();
            *guard = Some(SimulatorSession {
                state: starting.clone(),
                engine: None,
                uart: UartBridge::new(self.bus.clone()),
                board: board.clone(),
            });
        }
        self.bus.publish(AppEvent::new(
            event_types::SIMULATOR_STARTING,
            "M08",
            serde_json::to_value(&starting).unwrap_or(json!({})),
        ));

        let (mon_port, uart_port) = port_pair(&session_id);
        let mut engine = EmulationEngine::new(session_id.clone(), self.bus.clone(), mon_port, uart_port);

        if let Some(ref doc) = netlist {
            engine.peripheral_host_mut().sync_netlist(doc);
        }

        if let Err(e) = engine.start(elf, &board) {
            let err = SimulatorSessionState {
                schema_version: SCHEMA_VERSION_SIMULATOR,
                session_id,
                status: "error".into(),
                elf_path: Some(request.elf_path),
                started_at: None,
                message: Some(e.to_string()),
                error_code: Some("engine_start_failed".into()),
            };
            *self.session.lock().unwrap() = Some(SimulatorSession {
                state: err.clone(),
                engine: None,
                uart: UartBridge::new(self.bus.clone()),
                board,
            });
            self.publish_state("M08");
            return Err(e);
        }

        if let Some(profile) = board.uart_profiles.first() {
            if let Ok(mut guard) = self.session.lock() {
                if let Some(s) = guard.as_mut() {
                    if let Err(e) = s.uart.attach_port(&session_id, profile) {
                        eprintln!("uart_attach_failed: {e}");
                    }
                }
            }
        }

        let running = SimulatorSessionState {
            schema_version: SCHEMA_VERSION_SIMULATOR,
            session_id: session_id.clone(),
            status: "running".into(),
            elf_path: Some(request.elf_path),
            started_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string(),
            ),
            message: None,
            error_code: None,
        };

        {
            let mut guard = self.session.lock().unwrap();
            let s = guard.as_mut().unwrap();
            s.engine = Some(engine);
            s.state = running.clone();
        }
        self.publish_state("M08");
        Ok(running)
    }

    pub fn stop(&self) -> Result<SimulatorSessionState> {
        let out = {
            let mut guard = self.session.lock().unwrap();
            if let Some(s) = guard.as_mut() {
                if let Some(mut eng) = s.engine.take() {
                    eng.stop();
                }
                s.uart.detach_all();
                s.state = SimulatorSessionState {
                    schema_version: SCHEMA_VERSION_SIMULATOR,
                    session_id: s.state.session_id.clone(),
                    status: "stopped".into(),
                    elf_path: s.state.elf_path.clone(),
                    started_at: s.state.started_at.clone(),
                    message: Some("stopped by user".into()),
                    error_code: None,
                };
                s.state.clone()
            } else {
                SimulatorSessionState::idle()
            }
        };
        self.publish_state("M08");
        Ok(out)
    }

    pub fn reset(&self) -> Result<()> {
        if let Ok(guard) = self.session.lock() {
            if let Some(s) = guard.as_ref() {
                if let Some(ref eng) = s.engine {
                    eng.reset()?;
                }
            }
        }
        self.bus.publish(AppEvent::new(
            event_types::SIMULATOR_RESET,
            "M08",
            json!({ "schemaVersion": SCHEMA_VERSION_SIMULATOR }),
        ));
        Ok(())
    }

    pub fn handle_board_interaction(&self, req: BoardInteractionRequest) -> Result<()> {
        if req.interaction_type == "reset" {
            return self.reset();
        }
        let guard = self.session.lock().unwrap();
        let s = guard.as_ref().ok_or_else(|| anyhow!("session_not_running"))?;
        let eng = s
            .engine
            .as_ref()
            .ok_or_else(|| anyhow!("session_not_running"))?;
        let drive = {
            let host = eng.peripheral_host_ref();
            match req.interaction_type.as_str() {
                "buttonPress" | "press" => host.on_button_press(&req.session_id, &req.target_id),
                "buttonRelease" | "release" => {
                    host.on_button_release(&req.session_id, &req.target_id)
                }
                _ => None,
            }
        };
        if let Some(drive) = drive {
            eng.inject_pin(&drive.pin_id, drive.level)?;
        }
        Ok(())
    }

    pub fn host_send_bytes(&self, req: z6ds_core::contracts::HostSendBytes) -> Result<()> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&req.bytes_base64)
            .map_err(|e| anyhow!("encoding_error: {e}"))?;
        let guard = self.session.lock().unwrap();
        let s = guard.as_ref().ok_or_else(|| anyhow!("session_not_running"))?;
        s.uart.host_send(&req.session_id, &req.port_id, &bytes)?;
        if let Some(ref eng) = s.engine {
            eng.host_uart_tx_blocking(&bytes)?;
        }
        Ok(())
    }
}
