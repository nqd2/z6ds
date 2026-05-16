//! M10 — Tauri commands for netlist store.

use tauri::State;
use z6ds_core::contracts::BoardConfig;
use z6ds_core::netlist::{NetlistDocument, ValidationResult};
use crate::AppStateHandle;

#[tauri::command(rename_all = "camelCase")]
pub fn get_netlist(state: State<'_, AppStateHandle>) -> NetlistDocument {
    state
        .lock()
        .expect("app state lock")
        .netlist
        .document()
        .clone()
}

#[tauri::command(rename_all = "camelCase")]
pub fn apply_netlist_defaults(
    board_config: BoardConfig,
    state: State<'_, AppStateHandle>,
) -> NetlistDocument {
    state
        .lock()
        .expect("app state lock")
        .netlist
        .apply_board_defaults(&board_config)
        .clone()
}

#[tauri::command(rename_all = "camelCase")]
pub fn validate_netlist_cmd(
    rules: Option<Vec<String>>,
    state: State<'_, AppStateHandle>,
) -> ValidationResult {
    let guard = state.lock().expect("app state lock");
    let rule_refs: Vec<&str> = rules
        .as_ref()
        .map(|r| r.iter().map(String::as_str).collect())
        .unwrap_or_else(|| vec!["mvp"]);
    guard.netlist.validate(&rule_refs)
}
