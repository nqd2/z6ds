//! M08 — Tauri commands for simulator session.

use std::sync::Arc;

use tauri::State;
use z6ds_core::contracts::{
    BoardConfig, BoardInteractionRequest, HostSendBytes, SimulatorRunRequest,
    SimulatorSessionState,
};
use z6ds_core::EventBus;
use z6ds_sim::SessionController;

use crate::AppStateHandle;

pub struct SimState {
    pub controller: SessionController,
}

impl SimState {
    pub fn new(bus: EventBus) -> Self {
        Self {
            controller: SessionController::new(bus),
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn run_simulator(
    request: SimulatorRunRequest,
    app: State<'_, AppStateHandle>,
    sim: State<'_, Arc<SimState>>,
) -> Result<SimulatorSessionState, String> {
    let netlist = {
        let guard = app.lock().expect("app state");
        Some(guard.netlist.document().clone())
    };
    sim.controller
        .run(request, netlist)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn stop_simulator(sim: State<'_, Arc<SimState>>) -> Result<SimulatorSessionState, String> {
    sim.controller.stop().map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reset_simulator(sim: State<'_, Arc<SimState>>) -> Result<(), String> {
    sim.controller.reset().map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_simulator_state(sim: State<'_, Arc<SimState>>) -> SimulatorSessionState {
    sim.controller.state()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn handle_board_interaction(
    request: BoardInteractionRequest,
    sim: State<'_, Arc<SimState>>,
) -> Result<(), String> {
    sim.controller
        .handle_board_interaction(request)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn host_send_uart(request: HostSendBytes, sim: State<'_, Arc<SimState>>) -> Result<(), String> {
    sim.controller
        .host_send_bytes(request)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn resolve_sim_elf(
    manifest_elf: Option<String>,
    project_root: Option<String>,
) -> Result<String, String> {
    if let Some(p) = manifest_elf {
        if std::path::Path::new(&p).is_file() {
            return Ok(p);
        }
    }
    if let Some(root) = project_root {
        for name in ["Debug/week7_3_2.elf", "Debug/sample.elf"] {
            let p = std::path::Path::new(&root).join(name);
            if p.is_file() {
                return Ok(p.to_string_lossy().into_owned());
            }
        }
        if let Ok(entries) = std::fs::read_dir(std::path::Path::new(&root).join("Debug")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "elf") {
                    return Ok(path.to_string_lossy().into_owned());
                }
            }
        }
    }
    Err("elf_not_found: build Debug target first".into())
}

#[tauri::command]
pub fn lab_board_config() -> BoardConfig {
    BoardConfig::lab_disc1_defaults()
}
