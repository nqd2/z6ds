//! z6ds desktop — Tauri shell; M02 discovery, M03 file commands, M06 build, M10 netlist.

mod build_ops;
mod event_bridge;
mod fs_ops;
mod netlist_ops;
mod sim_ops;

use std::sync::Mutex;

use serde_json::json;
use tauri::State;
use z6ds_core::{
    contracts::{
        event_types, AppEvent, BoardConfig, DiscoverRequest, DiscoveryResult, ProjectManifest,
        RefreshProjectRequest, ValidateManifestRequest,
    },
    discover_and_publish, parse_ioc_file, sample_project_root,
    validate_manifest as validate_project_manifest, EventBus, NetlistStore,
};
use z6ds_build::BuildOrchestrator;

pub(crate) type AppStateHandle = Mutex<AppState>;

pub(crate) struct AppState {
    event_bus: EventBus,
    project_root: Option<String>,
    manifest: Option<ProjectManifest>,
    netlist: NetlistStore,
}

#[tauri::command]
fn fs_list_dir(path: String) -> Result<Vec<fs_ops::FsEntry>, String> {
    fs_ops::list_dir(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn fs_read_file(path: String) -> Result<String, String> {
    fs_ops::read_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn fs_write_file(path: String, contents: String, state: State<'_, AppStateHandle>) -> Result<(), String> {
    fs_ops::write_file(&path, &contents).map_err(|e| e.to_string())?;
    let bus = &state.lock().expect("app state lock").event_bus;
    bus.publish(AppEvent::new(
        event_types::FILE_SAVED,
        "M03",
        json!({ "path": path }),
    ));
    Ok(())
}

/// Read a file relative to the active project root (M03 sandbox via M02 root).
#[tauri::command]
fn fs_read_project_file(relative_path: String, state: State<'_, AppStateHandle>) -> Result<String, String> {
    let guard = state.lock().expect("app state lock");
    let root = guard
        .project_root
        .as_deref()
        .ok_or_else(|| "no project root; run discover_project first".to_string())?;
    let resolved = fs_ops::resolve_under_root(root, &relative_path).map_err(|e| e.to_string())?;
    fs_ops::read_file(resolved.to_str().unwrap_or_default()).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_project_root(state: State<'_, AppStateHandle>) -> Option<String> {
    state.lock().expect("app state lock").project_root.clone()
}

#[tauri::command]
fn get_project_manifest(state: State<'_, AppStateHandle>) -> Option<ProjectManifest> {
    state.lock().expect("app state lock").manifest.clone()
}

#[tauri::command(rename_all = "camelCase")]
fn discover_project(
    request: DiscoverRequest,
    state: State<'_, AppStateHandle>,
) -> Result<DiscoveryResult, String> {
    let mut guard = state.lock().expect("app state lock");
    let result = discover_and_publish(&guard.event_bus, &request, false);
    if let Some(manifest) = &result.manifest {
        guard.project_root = Some(manifest.root_path.clone());
        guard.manifest = Some(manifest.clone());
    } else {
        guard.project_root = None;
        guard.manifest = None;
    }
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
fn refresh_project(
    request: RefreshProjectRequest,
    state: State<'_, AppStateHandle>,
) -> Result<DiscoveryResult, String> {
    let mut guard = state.lock().expect("app state lock");
    let discover_req = DiscoverRequest {
        schema_version: request.schema_version,
        root_path: request.root_path.clone(),
        correlation_id: request.correlation_id.clone(),
    };
    let _ = request.reason;
    let result = discover_and_publish(&guard.event_bus, &discover_req, true);
    if let Some(manifest) = &result.manifest {
        guard.project_root = Some(manifest.root_path.clone());
        guard.manifest = Some(manifest.clone());
    }
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
fn validate_manifest(request: ValidateManifestRequest) -> Result<(), String> {
    validate_project_manifest(&request)
        .map_err(|issues| serde_json::to_string(&issues).unwrap_or_else(|_| "invalid manifest".into()))
}

#[tauri::command]
fn parse_board_config(ioc_path: String) -> Result<BoardConfig, String> {
    parse_ioc_file(&ioc_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_sample_project_root() -> String {
    sample_project_root().to_string_lossy().into_owned()
}

#[tauri::command]
fn publish_app_event(
    event_type: String,
    source: String,
    payload: serde_json::Value,
    correlation_id: Option<String>,
    state: State<'_, AppStateHandle>,
) -> Result<(), String> {
    let mut event = AppEvent::new(event_type, source, payload);
    if let Some(id) = correlation_id {
        event = event.with_correlation_id(id);
    }
    state
        .lock()
        .expect("app state lock")
        .event_bus
        .publish(event);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let event_bus = EventBus::new();
    let netlist = NetlistStore::new(event_bus.clone());
    let build_orchestrator = std::sync::Arc::new(BuildOrchestrator::new(event_bus.clone()));
    let sim_state = std::sync::Arc::new(sim_ops::SimState::new(event_bus.clone()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppStateHandle::new(AppState {
            event_bus: event_bus.clone(),
            project_root: None,
            manifest: None,
            netlist,
        }))
        .manage(build_orchestrator)
        .manage(sim_state)
        .setup(move |app| {
            event_bridge::attach_event_bridge(app.handle().clone(), &event_bus);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fs_list_dir,
            fs_read_file,
            fs_write_file,
            fs_read_project_file,
            get_project_root,
            get_project_manifest,
            discover_project,
            refresh_project,
            validate_manifest,
            parse_board_config,
            publish_app_event,
            netlist_ops::get_netlist,
            netlist_ops::apply_netlist_defaults,
            netlist_ops::validate_netlist_cmd,
            build_ops::detect_toolchain,
            build_ops::configure_toolchain,
            build_ops::build_project,
            build_ops::clean_project,
            build_ops::cancel_build,
            get_sample_project_root,
            sim_ops::run_simulator,
            sim_ops::stop_simulator,
            sim_ops::reset_simulator,
            sim_ops::get_simulator_state,
            sim_ops::handle_board_interaction,
            sim_ops::host_send_uart,
            sim_ops::resolve_sim_elf,
            sim_ops::lab_board_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use z6ds_core::{
        contracts::{DiscoverRequest, DiscoveryStatus, SCHEMA_VERSION_DISCOVER_REQUEST},
        discover_project as run_discovery, sample_project_root,
    };

    #[test]
    fn discover_sample_via_core() {
        let root = sample_project_root();
        let result = run_discovery(&DiscoverRequest {
            schema_version: SCHEMA_VERSION_DISCOVER_REQUEST,
            root_path: root.to_string_lossy().into_owned(),
            correlation_id: None,
        });
        assert_eq!(result.status, DiscoveryStatus::Success);
    }
}
