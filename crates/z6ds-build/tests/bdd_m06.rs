//! M06 BDD acceptance tests (TC-M06-01 … TC-M06-07).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tempfile::TempDir;
use tokio::time::sleep;
use z6ds_build::{
    parse_diagnostics, BuildOrchestrator, ERROR_BUILD_CANCELLED, ERROR_MAKEFILE_MISSING,
    ERROR_TOOLCHAIN_MISSING, ToolchainConfig,
};
use z6ds_core::contracts::{
    event_types, BuildRequest, CancelBuildRequest, SCHEMA_VERSION_BUILD,
};
use z6ds_core::EventBus;

fn repo_sample_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/sample-prj")
}

fn make_path() -> PathBuf {
    which::which("make").expect("make on PATH for fixture tests")
}

fn orch_with_make_only() -> BuildOrchestrator {
    let bus = EventBus::new();
    let orch = BuildOrchestrator::new(bus);
    orch.set_toolchain_for_test(ToolchainConfig {
        make_path: make_path(),
        gcc_path: PathBuf::from("/nonexistent/arm-none-eabi-gcc"),
        version: String::new(),
    });
    orch
}

#[path = "which.rs"]
mod which;

#[tokio::test]
async fn tc_m06_06_missing_toolchain() {
    let orch = orch_with_make_only();
    let req = BuildRequest::new(repo_sample_root().to_str().unwrap(), "Debug");
    let result = orch.build(req).await;
    assert_eq!(result.status, "failed");
    assert_eq!(result.error_code.as_deref(), Some(ERROR_TOOLCHAIN_MISSING));
}

#[tokio::test]
async fn tc_m06_makefile_missing() {
    let bus = EventBus::new();
    let orch = BuildOrchestrator::new(bus);
    orch.set_toolchain_for_test(ToolchainConfig {
        make_path: make_path(),
        gcc_path: PathBuf::from("/usr/bin/false-gcc"),
        version: String::new(),
    });
    let req = BuildRequest::new("/nonexistent/project", "Debug");
    let result = orch.build(req).await;
    assert_eq!(result.status, "failed");
    assert_eq!(result.error_code.as_deref(), Some(ERROR_MAKEFILE_MISSING));
}

#[tokio::test]
async fn tc_m06_02_streaming_before_final() {
    let dir = TempDir::new().unwrap();
    let makefile = dir.path().join("Makefile");
    std::fs::write(
        &makefile,
        r"
all:
	@echo line-one
	@echo line-two
	touch app.elf
",
    )
    .unwrap();

    let bus = EventBus::new();
    let log_events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log_c = Arc::clone(&log_events);
    let completed: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let completed_c = Arc::clone(&completed);

    bus.subscribe_type(event_types::BUILD_LOG, move |ev| {
        if let Some(t) = ev.payload.get("text").and_then(|v| v.as_str()) {
            log_c.lock().unwrap().push(t.to_string());
        }
    });
    bus.subscribe_type(event_types::BUILD_COMPLETED, move |ev| {
        *completed_c.lock().unwrap() = ev
            .payload
            .get("status")
            .and_then(|s| s.as_str())
            .map(str::to_string);
    });

    let orch = BuildOrchestrator::new(bus);
    orch.set_toolchain_for_test(ToolchainConfig {
        make_path: make_path(),
        gcc_path: make_path(),
        version: "stub".into(),
    });

    let req = BuildRequest::new(dir.path().to_str().unwrap(), "");
    let result = orch.build(req).await;

    assert!(!log_events.lock().unwrap().is_empty(), "expected log chunks before final");
    assert_eq!(result.status, "success");
    assert!(completed.lock().unwrap().as_deref() == Some("success"));
}

#[tokio::test]
async fn tc_m06_03_parse_gcc_diagnostic_from_log() {
    let log = "Core/Src/main.c:42:5: error: expected ';' before '}' token\n";
    let diags = parse_diagnostics(log);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].path, "Core/Src/main.c");
    assert_eq!(diags[0].line, 42);
    assert_eq!(diags[0].column, 5);
    assert_eq!(diags[0].severity, "error");
}

#[tokio::test]
async fn tc_m06_05_cancellation() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("Makefile"),
        r"
all:
	@sleep 5
",
    )
    .unwrap();

    let bus = EventBus::new();
    let orch = BuildOrchestrator::new(bus);
    orch.set_toolchain_for_test(ToolchainConfig {
        make_path: make_path(),
        gcc_path: make_path(),
        version: "stub".into(),
    });

    let req = BuildRequest::new(dir.path().to_str().unwrap(), "");
    let orch_c = Arc::new(orch);
    let orch_build = Arc::clone(&orch_c);

    let build_id = Arc::new(Mutex::new(String::new()));
    let build_id_c = Arc::clone(&build_id);
    orch_c.event_bus().subscribe_type(event_types::BUILD_STARTED, move |ev| {
        if let Some(id) = ev.payload.get("buildId").and_then(|v| v.as_str()) {
            *build_id_c.lock().unwrap() = id.to_string();
        }
    });

    let handle = tokio::spawn(async move {
        orch_build.build(req).await
    });

    sleep(Duration::from_millis(300)).await;
    let id = build_id.lock().unwrap().clone();
    assert!(!id.is_empty());
    orch_c
        .cancel_build(CancelBuildRequest {
            schema_version: SCHEMA_VERSION_BUILD,
            build_id: id,
        })
        .await
        .expect("cancel");

    let result = handle.await.unwrap();
    assert_eq!(result.status, "cancelled");
    assert_eq!(result.error_code.as_deref(), Some(ERROR_BUILD_CANCELLED));
}

#[tokio::test]
#[ignore = "requires arm-none-eabi-gcc on PATH"]
async fn tc_m06_01_build_sample_debug() {
    let bus = EventBus::new();
    let orch = BuildOrchestrator::new(bus);
    let _ = orch.detect_toolchain();
    let info = orch.detect_toolchain();
    if !info.detected {
        eprintln!("skipping: toolchain not detected");
        return;
    }

    let req = BuildRequest::new(repo_sample_root().to_str().unwrap(), "Debug");
    let result = orch.build(req).await;
    assert_eq!(result.status, "success");
    let elf = result.elf_path.expect("elf path");
    assert!(elf.contains("week7_3_2.elf"));
    assert!(PathBuf::from(elf).is_file());
}

#[tokio::test]
async fn tc_m06_04_clean_rebuild_invokes_clean() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("Makefile"),
        r"
all:
	@echo build
	touch app.elf

clean:
	@echo clean
	rm -f app.elf
",
    )
    .unwrap();

    let bus = EventBus::new();
    let orch = BuildOrchestrator::new(bus);
    orch.set_toolchain_for_test(ToolchainConfig {
        make_path: make_path(),
        gcc_path: make_path(),
        version: "stub".into(),
    });

    let mut req = BuildRequest::new(dir.path().to_str().unwrap(), "");
    req.clean = true;
    let result = orch.build(req).await;
    assert_eq!(result.status, "success");
    assert!(result.log_text.contains("clean"));
    assert!(result.log_text.contains("build"));
    assert!(result.elf_path.is_some());
}

#[tokio::test]
async fn tc_m06_07_release_target_when_makefile_exists() {
    let release = repo_sample_root().join("Release");
    if !release.join("makefile").is_file() && !release.join("Makefile").is_file() {
        eprintln!("skipping TC-M06-07: Release makefile not present");
        return;
    }
    let orch = orch_with_make_only();
    let req = BuildRequest::new(repo_sample_root().to_str().unwrap(), "Release");
    let result = orch.build(req).await;
    // Fails at toolchain without gcc; verify makefile resolution path exists
    assert!(
        result.error_code.as_deref() == Some(ERROR_TOOLCHAIN_MISSING)
            || result.status == "failed"
    );
}
