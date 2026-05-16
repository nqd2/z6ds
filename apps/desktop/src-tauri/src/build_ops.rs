//! M06 — Tauri commands for build orchestration.

use std::sync::Arc;

use tauri::State;
use z6ds_build::BuildOrchestrator;
use z6ds_core::contracts::{
    BuildRequest, BuildResult, CancelBuildRequest, CleanRequest, ConfigureToolchainRequest,
    ToolchainInfo,
};

pub type SharedBuildOrchestrator = Arc<BuildOrchestrator>;

#[tauri::command]
pub async fn detect_toolchain(state: State<'_, SharedBuildOrchestrator>) -> Result<ToolchainInfo, String> {
    Ok(state.detect_toolchain())
}

#[tauri::command]
pub async fn configure_toolchain(
    request: ConfigureToolchainRequest,
    state: State<'_, SharedBuildOrchestrator>,
) -> Result<ToolchainInfo, String> {
    state
        .configure_toolchain(request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn build_project(
    request: BuildRequest,
    state: State<'_, SharedBuildOrchestrator>,
) -> Result<BuildResult, String> {
    Ok(state.build(request).await)
}

#[tauri::command]
pub async fn clean_project(
    request: CleanRequest,
    state: State<'_, SharedBuildOrchestrator>,
) -> Result<BuildResult, String> {
    Ok(state.clean(request).await)
}

#[tauri::command]
pub async fn cancel_build(
    request: CancelBuildRequest,
    state: State<'_, SharedBuildOrchestrator>,
) -> Result<(), String> {
    state.cancel_build(request).await
}
