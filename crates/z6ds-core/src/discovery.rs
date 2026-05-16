//! M02 — STM32CubeIDE project discovery → `ProjectManifest`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::json;

use crate::contracts::{
    event_types, AppEvent, BuildTarget, DiscoverRequest, DiscoveryIssue, DiscoveryResult,
    DiscoveryStatus, ElfCandidate, ProjectManifest, RefreshProjectRequest, ValidateManifestRequest,
    SCHEMA_VERSION_BUILD_TARGET, SCHEMA_VERSION_DISCOVERY_RESULT, SCHEMA_VERSION_ELF_CANDIDATE,
    SCHEMA_VERSION_PROJECT_MANIFEST,
};
use crate::EventBus;

const LAB_BOARD_ID: &str = "STM32F429I-DISC1";
const BUILD_TARGET_NAMES: &[&str] = &["Debug", "Release"];

pub fn discover_project(request: &DiscoverRequest) -> DiscoveryResult {
    discover_internal(&request.root_path, false)
}

pub fn refresh_project(request: &RefreshProjectRequest) -> DiscoveryResult {
    discover_internal(&request.root_path, true)
}

pub fn validate_manifest(request: &ValidateManifestRequest) -> Result<(), Vec<DiscoveryIssue>> {
    let mut issues = Vec::new();
    let m = &request.manifest;

    if m.schema_version != SCHEMA_VERSION_PROJECT_MANIFEST {
        issues.push(DiscoveryIssue::new(
            "invalid_schema_version",
            format!(
                "expected manifest schemaVersion {}, got {}",
                SCHEMA_VERSION_PROJECT_MANIFEST, m.schema_version
            ),
        ));
    }
    if m.root_path.is_empty() {
        issues.push(DiscoveryIssue::new("missing_root_path", "rootPath is required"));
    }
    if m.ioc_path.is_empty() {
        issues.push(DiscoveryIssue::new("missing_ioc_path", "iocPath is required"));
    }
    if m.mcu_id.is_empty() {
        issues.push(DiscoveryIssue::new("missing_mcu_id", "mcuId is required"));
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Run discovery and publish lifecycle events on `bus`.
pub fn discover_and_publish(
    bus: &EventBus,
    request: &DiscoverRequest,
    refresh: bool,
) -> DiscoveryResult {
    let correlation_id = request.correlation_id.clone();
    let result = if refresh {
        refresh_project(&RefreshProjectRequest {
            schema_version: request.schema_version,
            root_path: request.root_path.clone(),
            reason: None,
            correlation_id: correlation_id.clone(),
        })
    } else {
        discover_project(request)
    };

    publish_discovery_events(bus, &result, refresh, correlation_id);
    result
}

fn publish_discovery_events(
    bus: &EventBus,
    result: &DiscoveryResult,
    refresh: bool,
    correlation_id: Option<String>,
) {
    match result.status {
        DiscoveryStatus::Success | DiscoveryStatus::Partial => {
            if let Some(manifest) = &result.manifest {
                let payload = serde_json::to_value(manifest).unwrap_or(json!({}));
                let event_type = if refresh {
                    event_types::PROJECT_REFRESHED
                } else {
                    event_types::PROJECT_OPENED
                };
                let mut event = AppEvent::new(event_type, "M02", payload);
                if let Some(id) = correlation_id {
                    event = event.with_correlation_id(id);
                }
                bus.publish(event);
            }
        }
        DiscoveryStatus::Failed => {
            let payload = json!({
                "errors": result.errors,
                "warnings": result.warnings,
            });
            let mut event = AppEvent::new(event_types::PROJECT_DISCOVERY_FAILED, "M02", payload);
            if let Some(id) = correlation_id {
                event = event.with_correlation_id(id);
            }
            bus.publish(event);
        }
    }
}

fn discover_internal(root_path: &str, _refresh: bool) -> DiscoveryResult {
    let root = Path::new(root_path);
    if !root.exists() {
        return DiscoveryResult::failed(vec![DiscoveryIssue::new(
            "root_not_found",
            format!("project root does not exist: {root_path}"),
        )]);
    }

    let root = match root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return DiscoveryResult::failed(vec![DiscoveryIssue::new(
                "root_not_found",
                format!("cannot canonicalize root {root_path}: {e}"),
            )]);
        }
    };

    let root_str = root.to_string_lossy().into_owned();
    let has_project = root.join(".project").is_file();
    let has_cproject = root.join(".cproject").is_file();
    let ioc_files = find_ioc_files(&root);
    let has_cubeide_marker = has_project || has_cproject;

    if !has_cubeide_marker && ioc_files.is_empty() {
        return DiscoveryResult::failed(vec![DiscoveryIssue::new(
            "not_cubeide_project",
            "folder is missing .project, .cproject, and *.ioc markers",
        )]);
    }

    let ioc_path = match ioc_files.first() {
        Some(p) => p.clone(),
        None if has_cubeide_marker => {
            let mut errors = vec![DiscoveryIssue::new(
                "not_cubeide_project",
                "CubeIDE markers found but no *.ioc file",
            )];
            if !has_cubeide_marker {
                errors.push(DiscoveryIssue::new(
                    "not_cubeide_project",
                    "missing .project and .cproject",
                ));
            }
            return DiscoveryResult::failed(errors);
        }
        None => {
            return DiscoveryResult::failed(vec![DiscoveryIssue::new(
                "not_cubeide_project",
                "folder is not a recognizable STM32CubeIDE project",
            )]);
        }
    };

    let project_name = ioc_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let ioc_kv = read_ioc_key_values(&ioc_path);
    let mcu_from_ioc = ioc_kv
        .get("Mcu.Name")
        .or_else(|| ioc_kv.get("Mcu.UserName"))
        .cloned();
    let mcu_from_cproject = read_mcu_from_cproject(&root.join(".cproject"));
    let mcu_id = mcu_from_ioc
        .or(mcu_from_cproject)
        .unwrap_or_else(|| "unknown".to_string());

    let board_id = ioc_kv
        .get("board")
        .filter(|b| !b.eq_ignore_ascii_case("custom"))
        .cloned()
        .unwrap_or_else(|| infer_board_id(&mcu_id));

    let build_targets = collect_build_targets(&root, &root_str);
    let elf_candidates = collect_elf_candidates(&root, &root_str);

    let mut warnings = Vec::new();
    let mut status = DiscoveryStatus::Success;

    if build_targets.is_empty() {
        status = DiscoveryStatus::Partial;
        warnings.push(DiscoveryIssue::new(
            "no_build_target",
            "no Debug/Release makefile found; generate or build the project in STM32CubeIDE",
        ));
    }

    if elf_candidates.is_empty() {
        if status == DiscoveryStatus::Success {
            status = DiscoveryStatus::Partial;
        }
        warnings.push(DiscoveryIssue::new(
            "no_elf_candidate",
            "no Debug/*.elf or Release/*.elf found; build the project to produce firmware",
        ));
    }

    if !is_supported_lab_mcu(&mcu_id) {
        if status == DiscoveryStatus::Success {
            status = DiscoveryStatus::Partial;
        }
        warnings.push(DiscoveryIssue::new(
            "unsupported_mcu",
            format!(
                "MCU {mcu_id} is outside the IT4210 lab target (expected STM32F429ZITx on {LAB_BOARD_ID})"
            ),
        ));
    } else if board_id != LAB_BOARD_ID && !board_id.eq_ignore_ascii_case("custom") {
        warnings.push(DiscoveryIssue::new(
            "board_hint",
            format!(
                "boardId {board_id} may differ from lab default {LAB_BOARD_ID}; verify wiring in 3D view"
            ),
        ));
    }

    let manifest = ProjectManifest {
        schema_version: SCHEMA_VERSION_PROJECT_MANIFEST,
        root_path: root_str,
        project_name,
        mcu_id,
        board_id,
        ioc_path: ioc_path.to_string_lossy().into_owned(),
        build_targets,
        elf_candidates,
    };

    DiscoveryResult {
        schema_version: SCHEMA_VERSION_DISCOVERY_RESULT,
        status,
        manifest: Some(manifest),
        errors: Vec::new(),
        warnings,
    }
}

fn find_ioc_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ioc"))
        })
        .collect();
    files.sort();
    files
}

fn read_ioc_key_values(path: &Path) -> HashMap<String, String> {
    let Ok(content) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        map.insert(key.trim().to_string(), value.trim().to_string());
    }
    map
}

fn read_mcu_from_cproject(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if !line.contains("target_mcu") {
            continue;
        }
        if let Some(value) = extract_xml_attribute(line, "value") {
            return Some(value);
        }
    }
    None
}

fn extract_xml_attribute(line: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn infer_board_id(mcu_id: &str) -> String {
    if mcu_id.to_ascii_uppercase().contains("STM32F429") {
        LAB_BOARD_ID.to_string()
    } else {
        "unknown".to_string()
    }
}

fn is_supported_lab_mcu(mcu_id: &str) -> bool {
    let upper = mcu_id.to_ascii_uppercase();
    upper.contains("STM32F429")
}

fn collect_build_targets(root: &Path, root_str: &str) -> Vec<BuildTarget> {
    let mut targets = Vec::new();
    for name in BUILD_TARGET_NAMES {
        let dir = root.join(name);
        let makefile = dir.join("makefile");
        if !makefile.is_file() {
            continue;
        }
        targets.push(BuildTarget {
            schema_version: SCHEMA_VERSION_BUILD_TARGET,
            name: (*name).to_string(),
            makefile_path: makefile.to_string_lossy().into_owned(),
            working_directory: dir.to_string_lossy().into_owned(),
            artifact_glob: format!("{name}/*.elf"),
        });
    }
    targets.sort_by(|a, b| a.name.cmp(&b.name));
    let _ = root_str;
    targets
}

fn collect_elf_candidates(root: &Path, root_str: &str) -> Vec<ElfCandidate> {
    let mut candidates = Vec::new();
    for target in BUILD_TARGET_NAMES {
        let dir = root.join(target);
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path
                .extension()
                .and_then(|e| e.to_str())
                .is_none_or(|e| !e.eq_ignore_ascii_case("elf"))
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let size_bytes = meta.len();
            if size_bytes == 0 {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            candidates.push(ElfCandidate {
                schema_version: SCHEMA_VERSION_ELF_CANDIDATE,
                path: path.to_string_lossy().into_owned(),
                target: (*target).to_string(),
                mtime,
                size_bytes,
            });
        }
    }

    candidates.sort_by(|a, b| {
        b.mtime
            .cmp(&a.mtime)
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
            .then_with(|| {
                target_rank(&a.target)
                    .cmp(&target_rank(&b.target))
                    .reverse()
            })
    });

    let _ = root_str;
    candidates
}

fn target_rank(target: &str) -> u8 {
    match target {
        "Debug" => 2,
        "Release" => 1,
        _ => 0,
    }
}

/// Integration-test anchor: `docs/sample-prj` or repo-root `sample-prj`.
pub fn sample_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../../docs/sample-prj"),
        manifest_dir.join("../../sample-prj"),
    ];
    for candidate in candidates {
        if candidate.join("week7_3_2.ioc").is_file() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }
    panic!("sample project not found (tried docs/sample-prj and sample-prj)");
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::contracts::SCHEMA_VERSION_DISCOVER_REQUEST;
    use crate::EventBus;

    fn discover_root(root: &Path) -> DiscoveryResult {
        discover_project(&DiscoverRequest::new(root.to_string_lossy()))
    }

    /// TC-M02-01/06 need `Success` (ELF on disk). When `arm-none-eabi-gcc` is absent,
    /// `docs/sample-prj/Debug/*.elf` may be missing; a tiny placeholder unblocks discovery tests.
    fn ensure_sample_elf_for_integration_tests(root: &Path) {
        let elf = root.join("Debug/week7_3_2.elf");
        if elf.is_file() {
            return;
        }
        fs::create_dir_all(root.join("Debug")).expect("Debug dir");
        fs::write(&elf, vec![0u8; 64]).expect("stub elf for M02 integration tests");
    }

    #[test]
    fn tc_m02_01_sample_project_success() {
        let root = sample_project_root();
        ensure_sample_elf_for_integration_tests(&root);
        let result = discover_root(&root);
        assert_eq!(result.status, DiscoveryStatus::Success, "{:?}", result);
        let manifest = result.manifest.expect("manifest");
        assert_eq!(manifest.schema_version, SCHEMA_VERSION_PROJECT_MANIFEST);
        assert_eq!(manifest.mcu_id, "STM32F429ZITx");
        assert!(manifest.ioc_path.ends_with("week7_3_2.ioc"));
        assert!(
            manifest
                .elf_candidates
                .iter()
                .any(|e| e.path.replace('\\', "/").ends_with("Debug/week7_3_2.elf")),
            "elf candidates: {:?}",
            manifest.elf_candidates
        );
    }

    #[test]
    fn tc_m02_02_build_targets_debug() {
        let root = sample_project_root();
        let manifest = discover_root(&root).manifest.unwrap();
        let debug = manifest
            .build_targets
            .iter()
            .find(|t| t.name == "Debug")
            .expect("Debug target");
        assert!(debug.makefile_path.replace('\\', "/").ends_with("Debug/makefile"));
        assert!(
            debug
                .working_directory
                .replace('\\', "/")
                .ends_with("sample-prj/Debug")
        );
        assert_eq!(debug.artifact_glob, "Debug/*.elf");
    }

    #[test]
    fn tc_m02_03_partial_without_elf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join(".project"), "<?xml version=\"1.0\"?>").unwrap();
        fs::write(root.join(".cproject"), r#"<option name="MCU" value="STM32F429ZITx"/>"#)
            .unwrap();
        fs::write(root.join("lab.ioc"), "Mcu.Name=STM32F429ZITx\n").unwrap();
        fs::create_dir_all(root.join("Debug")).unwrap();
        fs::write(root.join("Debug/makefile"), "# stub\n").unwrap();

        let result = discover_root(root);
        assert_eq!(result.status, DiscoveryStatus::Partial);
        assert!(result.manifest.is_some());
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.code == "no_elf_candidate")
        );
    }

    #[test]
    fn tc_m02_04_invalid_folder() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "not a project").unwrap();
        let result = discover_root(dir.path());
        assert_eq!(result.status, DiscoveryStatus::Failed);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == "not_cubeide_project")
        );
    }

    #[test]
    fn tc_m02_05_refresh_prefers_newest_elf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join(".project"), "p").unwrap();
        fs::write(root.join("app.ioc"), "Mcu.Name=STM32F429ZITx\n").unwrap();
        fs::create_dir_all(root.join("Debug")).unwrap();
        fs::write(root.join("Debug/makefile"), "# m\n").unwrap();

        let old_elf = root.join("Debug/old.elf");
        let new_elf = root.join("Debug/new.elf");
        fs::write(&old_elf, vec![0u8; 64]).unwrap();
        thread::sleep(Duration::from_millis(50));
        fs::write(&new_elf, vec![0u8; 128]).unwrap();

        let manifest = discover_root(root).manifest.unwrap();
        assert!(
            manifest.elf_candidates[0]
                .path
                .replace('\\', "/")
                .ends_with("Debug/new.elf")
        );
    }

    #[test]
    fn tc_m02_06_publishes_project_opened_with_correlation_id() {
        let root = sample_project_root();
        ensure_sample_elf_for_integration_tests(&root);
        let bus = EventBus::new();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_c = std::sync::Arc::clone(&seen);
        bus.subscribe_type(event_types::PROJECT_OPENED, move |ev| {
            seen_c.lock().unwrap().push((
                ev.event_type.clone(),
                ev.correlation_id.clone(),
                ev.payload.get("mcuId").and_then(|v| v.as_str()).map(str::to_string),
            ));
        });

        let request = DiscoverRequest {
            schema_version: SCHEMA_VERSION_DISCOVER_REQUEST,
            root_path: root.to_string_lossy().into_owned(),
            correlation_id: Some("req-42".into()),
        };
        let result = discover_and_publish(&bus, &request, false);
        assert_eq!(result.status, DiscoveryStatus::Success);

        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, event_types::PROJECT_OPENED);
        assert_eq!(events[0].1.as_deref(), Some("req-42"));
        assert_eq!(events[0].2.as_deref(), Some("STM32F429ZITx"));
    }

    #[test]
    fn tc_m02_07_unsupported_mcu_warning() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join(".project"), "p").unwrap();
        fs::write(root.join("other.ioc"), "Mcu.Name=STM32F103C8Tx\n").unwrap();
        fs::create_dir_all(root.join("Debug")).unwrap();
        fs::write(root.join("Debug/makefile"), "# m\n").unwrap();
        fs::write(root.join("Debug/other.elf"), vec![1u8; 32]).unwrap();

        let result = discover_root(root);
        assert_eq!(result.status, DiscoveryStatus::Partial);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.code == "unsupported_mcu")
        );
    }

    #[test]
    fn validate_manifest_rejects_empty_fields() {
        let manifest = ProjectManifest {
            schema_version: SCHEMA_VERSION_PROJECT_MANIFEST,
            root_path: String::new(),
            project_name: "x".into(),
            mcu_id: String::new(),
            board_id: "b".into(),
            ioc_path: String::new(),
            build_targets: vec![],
            elf_candidates: vec![],
        };
        let err = validate_manifest(&ValidateManifestRequest {
            schema_version: 1,
            manifest,
        })
        .unwrap_err();
        assert!(err.iter().any(|i| i.code == "missing_root_path"));
    }

    #[test]
    fn refresh_publishes_project_refreshed() {
        let root = sample_project_root();
        let bus = EventBus::new();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_c = std::sync::Arc::clone(&seen);
        bus.subscribe_type(event_types::PROJECT_REFRESHED, move |ev| {
            seen_c
                .lock()
                .unwrap()
                .push(ev.payload.get("rootPath").and_then(|v| v.as_str()).map(str::to_string));
        });

        let request = DiscoverRequest::new(root.to_string_lossy());
        discover_and_publish(&bus, &request, true);

        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].as_ref().is_some_and(|p| p.contains("sample-prj")));
    }

    #[test]
    fn root_not_found_does_not_publish_opened() {
        let bus = EventBus::new();
        let count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let count_c = std::sync::Arc::clone(&count);
        bus.subscribe_type(event_types::PROJECT_OPENED, move |_| {
            *count_c.lock().unwrap() += 1;
        });

        let request = DiscoverRequest::new("/no/such/project/root");
        let result = discover_and_publish(&bus, &request, false);
        assert_eq!(result.status, DiscoveryStatus::Failed);
        assert_eq!(*count.lock().unwrap(), 0);
    }
}
