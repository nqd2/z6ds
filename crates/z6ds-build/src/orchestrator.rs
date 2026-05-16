//! M06 — async `make` orchestration, log streaming, EventBus integration.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;
use z6ds_core::contracts::{
    event_types, AppEvent, BuildLogChunk, BuildRequest, BuildResult, BuildStarted,
    CancelBuildRequest, CleanRequest, ConfigureToolchainRequest, ProjectManifest,
    SCHEMA_VERSION_BUILD,
};
use z6ds_core::EventBus;

use crate::diagnostics::parse_diagnostics;
use crate::toolchain::{detect_toolchain_info, ToolchainConfig, ToolchainError};

pub const ERROR_TOOLCHAIN_MISSING: &str = "toolchain_missing";
pub const ERROR_MAKEFILE_MISSING: &str = "makefile_missing";
pub const ERROR_PROCESS_SPAWN_FAILED: &str = "process_spawn_failed";
pub const ERROR_ELF_NOT_FOUND: &str = "elf_not_found";
pub const ERROR_BUILD_CANCELLED: &str = "build_cancelled";

struct ActiveBuild {
    child: Arc<Mutex<Option<tokio::process::Child>>>,
    cancel_flag: Arc<AtomicBool>,
}

/// M06 build orchestrator — one active build at a time.
pub struct BuildOrchestrator {
    event_bus: EventBus,
    toolchain: std::sync::Mutex<Option<ToolchainConfig>>,
    active: Mutex<Option<(String, ActiveBuild)>>,
}

impl BuildOrchestrator {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            event_bus,
            toolchain: std::sync::Mutex::new(None),
            active: Mutex::new(None),
        }
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Inject toolchain (used by tests and manual configuration).
    pub fn set_toolchain_for_test(&self, cfg: ToolchainConfig) {
        *self.toolchain.lock().expect("toolchain lock") = Some(cfg);
    }

    pub fn detect_toolchain(&self) -> z6ds_core::contracts::ToolchainInfo {
        let info = detect_toolchain_info();
        self.event_bus.publish(AppEvent::new(
            event_types::TOOLCHAIN_DETECTED,
            "M06",
            serde_json::to_value(&info).unwrap_or(json!({})),
        ));
        if info.detected {
            if let Ok(cfg) = ToolchainConfig::detect() {
                *self.toolchain.lock().expect("toolchain lock") = Some(cfg);
            }
        }
        info
    }

    pub async fn configure_toolchain(
        &self,
        req: ConfigureToolchainRequest,
    ) -> Result<z6ds_core::contracts::ToolchainInfo, ToolchainError> {
        let cfg = ToolchainConfig::from_request(&req)?;
        let info = cfg.to_info(false);
        *self.toolchain.lock().expect("toolchain lock") = Some(cfg);
        self.event_bus.publish(AppEvent::new(
            event_types::TOOLCHAIN_DETECTED,
            "M06",
            serde_json::to_value(&info).unwrap_or(json!({})),
        ));
        Ok(info)
    }

    pub async fn build(&self, request: BuildRequest) -> BuildResult {
        let build_id = format!("build-{}", Uuid::new_v4());
        let started = BuildStarted {
            schema_version: SCHEMA_VERSION_BUILD,
            build_id: build_id.clone(),
            project_root: request.project_root.clone(),
            target: request.target.clone(),
            timestamp: chrono_now(),
        };
        self.event_bus.publish(AppEvent::new(
            event_types::BUILD_STARTED,
            "M06",
            serde_json::to_value(&started).unwrap_or(json!({})),
        ));

        let result = self.run_build(&build_id, &request).await;
        self.publish_completed(&result);
        result
    }

    pub async fn clean(&self, request: CleanRequest) -> BuildResult {
        let build_id = format!("clean-{}", Uuid::new_v4());
        let make_req = BuildRequest {
            schema_version: SCHEMA_VERSION_BUILD,
            project_root: request.project_root,
            target: request.target,
            clean: false,
            environment: HashMap::new(),
        };
        let result = self
            .run_make_goal(&build_id, &make_req, "clean")
            .await;
        self.publish_completed(&result);
        result
    }

    pub async fn cancel_build(&self, request: CancelBuildRequest) -> Result<(), String> {
        let guard = self.active.lock().await;
        if let Some((id, active)) = guard.as_ref() {
            if id != &request.build_id {
                return Err(format!("active build is {id}, not {}", request.build_id));
            }
            active.cancel_flag.store(true, Ordering::SeqCst);
            if let Some(child) = active.child.lock().await.as_mut() {
                let _ = child.start_kill();
            }
            return Ok(());
        }
        Err("no active build".to_string())
    }

    fn publish_completed(&self, result: &BuildResult) {
        let event_type = if result.status == "cancelled" {
            event_types::BUILD_CANCELLED
        } else {
            event_types::BUILD_COMPLETED
        };
        self.event_bus.publish(AppEvent::new(
            event_type,
            "M06",
            serde_json::to_value(result).unwrap_or(json!({})),
        ));
    }

    async fn run_build(&self, build_id: &str, request: &BuildRequest) -> BuildResult {
        let started = Instant::now();

        let build_dir = match resolve_build_dir(&request.project_root, &request.target) {
            Ok(d) => d,
            Err(e) => {
                return failed_result(
                    build_id,
                    started,
                    ERROR_MAKEFILE_MISSING,
                    e,
                    String::new(),
                );
            }
        };

        let toolchain = match self.resolve_toolchain().await {
            Ok(t) => t,
            Err(e) => {
                return failed_result(
                    build_id,
                    started,
                    ERROR_TOOLCHAIN_MISSING,
                    e.to_string(),
                    String::new(),
                );
            }
        };

        let mut log = String::new();

        if request.clean {
            match self
                .run_make_in_dir(build_id, &toolchain, &build_dir, "clean", &request.environment, &mut log)
                .await
            {
                Ok(()) => {}
                Err(outcome) => return finalize_result(build_id, started, Err(outcome), &log, &build_dir),
            }
        }

        let build_out = self
            .run_make_in_dir(build_id, &toolchain, &build_dir, "all", &request.environment, &mut log)
            .await;

        finalize_result(build_id, started, build_out, &log, &build_dir)
    }

    async fn run_make_goal(&self, build_id: &str, request: &BuildRequest, goal: &str) -> BuildResult {
        let started = Instant::now();
        let build_dir = match resolve_build_dir(&request.project_root, &request.target) {
            Ok(d) => d,
            Err(e) => {
                return failed_result(
                    build_id,
                    started,
                    ERROR_MAKEFILE_MISSING,
                    e,
                    String::new(),
                );
            }
        };
        let toolchain = match self.resolve_toolchain().await {
            Ok(t) => t,
            Err(e) => {
                return failed_result(
                    build_id,
                    started,
                    ERROR_TOOLCHAIN_MISSING,
                    e.to_string(),
                    String::new(),
                );
            }
        };
        let mut log = String::new();
        let out = self
            .run_make_in_dir(build_id, &toolchain, &build_dir, goal, &request.environment, &mut log)
            .await;
        finalize_result(build_id, started, out, &log, &build_dir)
    }

    async fn resolve_toolchain(&self) -> Result<ToolchainConfig, ToolchainError> {
        if let Some(cfg) = self.toolchain.lock().expect("toolchain lock").clone() {
            if !cfg.gcc_path.is_file() {
                return Err(ToolchainError::NotFound(format!(
                    "arm-none-eabi-gcc not found at {}",
                    cfg.gcc_path.display()
                )));
            }
            if !cfg.make_path.is_file() {
                return Err(ToolchainError::NotFound(format!(
                    "make not found at {}",
                    cfg.make_path.display()
                )));
            }
            return Ok(cfg);
        }
        let cfg = ToolchainConfig::detect()?;
        *self.toolchain.lock().expect("toolchain lock") = Some(cfg.clone());
        Ok(cfg)
    }

    async fn run_make_in_dir(
        &self,
        build_id: &str,
        toolchain: &ToolchainConfig,
        build_dir: &Path,
        goal: &str,
        extra_env: &HashMap<String, String>,
        log: &mut String,
    ) -> Result<(), BuildRunOutcome> {
        if self.active.lock().await.is_some() {
            return Err(BuildRunOutcome::Busy);
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut cmd = Command::new(&toolchain.make_path);
        cmd.arg("-C")
            .arg(build_dir)
            .arg(goal)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let mut env: HashMap<String, String> = extra_env.clone();
        prepend_path_env(&mut env, &toolchain.gcc_bin_dir());
        cmd.envs(env);

        let child = cmd
            .spawn()
            .map_err(|e| BuildRunOutcome::SpawnFailed(e.to_string()))?;

        let child_slot = Arc::new(Mutex::new(Some(child)));
        *self.active.lock().await = Some((
            build_id.to_string(),
            ActiveBuild {
                child: Arc::clone(&child_slot),
                cancel_flag: Arc::clone(&cancel_flag),
            },
        ));

        let mut child = child_slot.lock().await.take().expect("child process");
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        *child_slot.lock().await = Some(child);

        let mut stdout_task = None;
        let mut stderr_task = None;

        if let Some(out) = stdout {
            let bus = self.event_bus.clone();
            let bid = build_id.to_string();
            let cancel_c = Arc::clone(&cancel_flag);
            stdout_task = Some(tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                let mut collected = String::new();
                while let Ok(Some(line)) = lines.next_line().await {
                    if cancel_c.load(Ordering::SeqCst) {
                        break;
                    }
                    let text = format!("{line}\n");
                    collected.push_str(&text);
                    let chunk = BuildLogChunk {
                        schema_version: SCHEMA_VERSION_BUILD,
                        build_id: bid.clone(),
                        stream: "stdout".into(),
                        text: text.clone(),
                    };
                    bus.publish(AppEvent::new(
                        event_types::BUILD_LOG,
                        "M06",
                        serde_json::to_value(&chunk).unwrap_or(json!({})),
                    ));
                }
                collected
            }));
        }

        if let Some(err) = stderr {
            let bus = self.event_bus.clone();
            let bid = build_id.to_string();
            let cancel_c = Arc::clone(&cancel_flag);
            stderr_task = Some(tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                let mut collected = String::new();
                while let Ok(Some(line)) = lines.next_line().await {
                    if cancel_c.load(Ordering::SeqCst) {
                        break;
                    }
                    let text = format!("{line}\n");
                    collected.push_str(&text);
                    let chunk = BuildLogChunk {
                        schema_version: SCHEMA_VERSION_BUILD,
                        build_id: bid.clone(),
                        stream: "stderr".into(),
                        text: text.clone(),
                    };
                    bus.publish(AppEvent::new(
                        event_types::BUILD_LOG,
                        "M06",
                        serde_json::to_value(&chunk).unwrap_or(json!({})),
                    ));
                }
                collected
            }));
        }

        let status = {
            let mut guard = child_slot.lock().await;
            if let Some(child) = guard.as_mut() {
                child.wait().await
            } else {
                return Err(BuildRunOutcome::Cancelled);
            }
        };

        if let Some(handle) = stdout_task {
            if let Ok(chunk) = handle.await {
                log.push_str(&chunk);
            }
        }
        if let Some(handle) = stderr_task {
            if let Ok(chunk) = handle.await {
                log.push_str(&chunk);
            }
        }

        self.active.lock().await.take();

        if cancel_flag.load(Ordering::SeqCst) {
            return Err(BuildRunOutcome::Cancelled);
        }

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(BuildRunOutcome::MakeFailed(s.code().unwrap_or(1) as i32)),
            Err(e) => Err(BuildRunOutcome::SpawnFailed(e.to_string())),
        }
    }
}

#[derive(Debug)]
enum BuildRunOutcome {
    Cancelled,
    Busy,
    SpawnFailed(String),
    MakeFailed(i32),
}

fn finalize_result(
    build_id: &str,
    started: Instant,
    outcome: Result<(), BuildRunOutcome>,
    log: &str,
    build_dir: &Path,
) -> BuildResult {
    let diagnostics = parse_diagnostics(log);
    let duration_ms = started.elapsed().as_millis() as u64;

    match outcome {
        Ok(()) => {
            let elf = find_elf_in_dir(build_dir);
            if let Some(elf_path) = elf {
                BuildResult {
                    schema_version: SCHEMA_VERSION_BUILD,
                    build_id: build_id.to_string(),
                    status: "success".into(),
                    elf_path: Some(elf_path),
                    duration_ms,
                    log_text: log.to_string(),
                    diagnostics,
                    error_code: None,
                }
            } else {
                BuildResult {
                    schema_version: SCHEMA_VERSION_BUILD,
                    build_id: build_id.to_string(),
                    status: "failed".into(),
                    elf_path: None,
                    duration_ms,
                    log_text: log.to_string(),
                    diagnostics,
                    error_code: Some(ERROR_ELF_NOT_FOUND.to_string()),
                }
            }
        }
        Err(BuildRunOutcome::Cancelled) => BuildResult {
            schema_version: SCHEMA_VERSION_BUILD,
            build_id: build_id.to_string(),
            status: "cancelled".into(),
            elf_path: None,
            duration_ms,
            log_text: log.to_string(),
            diagnostics,
            error_code: Some(ERROR_BUILD_CANCELLED.to_string()),
        },
        Err(BuildRunOutcome::SpawnFailed(msg)) => failed_result(
            build_id,
            started,
            ERROR_PROCESS_SPAWN_FAILED,
            msg,
            log.to_string(),
        ),
        Err(BuildRunOutcome::Busy) => failed_result(
            build_id,
            started,
            "build_busy",
            "another build is already active".to_string(),
            log.to_string(),
        ),
        Err(BuildRunOutcome::MakeFailed(_code)) => BuildResult {
            schema_version: SCHEMA_VERSION_BUILD,
            build_id: build_id.to_string(),
            status: "failed".into(),
            elf_path: None,
            duration_ms,
            log_text: log.to_string(),
            diagnostics,
            error_code: None,
        },
    }
}

fn failed_result(
    build_id: &str,
    started: Instant,
    error_code: &str,
    message: String,
    log_text: String,
) -> BuildResult {
    BuildResult {
        schema_version: SCHEMA_VERSION_BUILD,
        build_id: build_id.to_string(),
        status: "failed".into(),
        elf_path: None,
        duration_ms: started.elapsed().as_millis() as u64,
        log_text: if log_text.is_empty() {
            message
        } else {
            format!("{log_text}\n{message}")
        },
        diagnostics: Vec::new(),
        error_code: Some(error_code.to_string()),
    }
}

pub fn resolve_build_dir(project_root: &str, target: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(project_root);
    let build_dir = if target.is_empty() {
        root
    } else {
        root.join(target)
    };
    let makefile = build_dir.join("makefile");
    let makefile_alt = build_dir.join("Makefile");
    if makefile.is_file() || makefile_alt.is_file() {
        Ok(build_dir.canonicalize().unwrap_or(build_dir))
    } else {
        Err(format!("makefile missing at {}", makefile.display()))
    }
}

pub fn find_elf_in_dir(dir: &Path) -> Option<String> {
    let mut newest: Option<(u64, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("elf") {
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                match &newest {
                    Some((best, _)) if mtime <= *best => {}
                    _ => newest = Some((mtime, path)),
                }
            }
        }
    }
    newest.map(|(_, p)| p.display().to_string())
}

fn prepend_path_env(env: &mut HashMap<String, String>, bin_dir: &Path) {
    let bin = bin_dir.display().to_string();
    let current = std::env::var("PATH").unwrap_or_default();
    env.insert("PATH".into(), format!("{bin}:{current}"));
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}

/// Stub manifest when M02 is unavailable (points at docs sample project).
pub fn stub_sample_manifest(repo_relative_root: &str) -> ProjectManifest {
    let root = PathBuf::from(repo_relative_root);
    let debug_dir = root.join("Debug");
    ProjectManifest {
        schema_version: z6ds_core::contracts::SCHEMA_VERSION_PROJECT_MANIFEST,
        root_path: root.display().to_string(),
        project_name: "week7_3_2".into(),
        mcu_id: "STM32F429ZIT6".into(),
        board_id: "STM32F429I-DISC1".into(),
        ioc_path: root.join("week7_3_2.ioc").display().to_string(),
        build_targets: vec![z6ds_core::contracts::BuildTarget {
            schema_version: z6ds_core::contracts::SCHEMA_VERSION_BUILD_TARGET,
            name: "Debug".into(),
            makefile_path: debug_dir.join("makefile").display().to_string(),
            working_directory: debug_dir.display().to_string(),
            artifact_glob: "*.elf".into(),
        }],
        elf_candidates: vec![z6ds_core::contracts::ElfCandidate {
            schema_version: z6ds_core::contracts::SCHEMA_VERSION_ELF_CANDIDATE,
            path: debug_dir.join("week7_3_2.elf").display().to_string(),
            target: "Debug".into(),
            mtime: 0,
            size_bytes: 0,
        }],
    }
}
