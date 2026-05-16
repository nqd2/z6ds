//! Toolchain auto-detect and configuration (GNU Tools / arm-none-eabi-gcc).

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;
use z6ds_core::contracts::{ConfigureToolchainRequest, ToolchainInfo, SCHEMA_VERSION_BUILD};

#[derive(Debug, Error)]
pub enum ToolchainError {
    #[error("toolchain not found: {0}")]
    NotFound(String),
    #[error("failed to run {tool}: {message}")]
    ExecFailed { tool: String, message: String },
}

#[derive(Debug, Clone)]
pub struct ToolchainConfig {
    pub make_path: PathBuf,
    pub gcc_path: PathBuf,
    pub version: String,
}

impl ToolchainConfig {
    pub fn detect() -> Result<Self, ToolchainError> {
        let make_path = find_executable("make")?;
        let gcc_path = find_arm_gcc()?;
        let version = read_gcc_version(&gcc_path)?;
        Ok(Self {
            make_path,
            gcc_path,
            version,
        })
    }

    pub fn from_request(req: &ConfigureToolchainRequest) -> Result<Self, ToolchainError> {
        let make_path = PathBuf::from(&req.make_path);
        let gcc_path = PathBuf::from(&req.gcc_path);
        if !make_path.is_file() {
            return Err(ToolchainError::NotFound(format!(
                "make not found at {}",
                make_path.display()
            )));
        }
        if !gcc_path.is_file() {
            return Err(ToolchainError::NotFound(format!(
                "arm-none-eabi-gcc not found at {}",
                gcc_path.display()
            )));
        }
        let version = read_gcc_version(&gcc_path)?;
        Ok(Self {
            make_path,
            gcc_path,
            version,
        })
    }

    pub fn to_info(&self, detected: bool) -> ToolchainInfo {
        ToolchainInfo {
            schema_version: SCHEMA_VERSION_BUILD,
            make_path: self.make_path.display().to_string(),
            gcc_path: self.gcc_path.display().to_string(),
            version: self.version.clone(),
            detected,
        }
    }

    /// Directory containing `arm-none-eabi-gcc` for PATH prepending.
    pub fn gcc_bin_dir(&self) -> PathBuf {
        self.gcc_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

pub fn detect_toolchain_info() -> ToolchainInfo {
    match ToolchainConfig::detect() {
        Ok(cfg) => cfg.to_info(true),
        Err(e) => ToolchainInfo {
            schema_version: SCHEMA_VERSION_BUILD,
            make_path: String::new(),
            gcc_path: String::new(),
            version: e.to_string(),
            detected: false,
        },
    }
}

fn find_executable(name: &str) -> Result<PathBuf, ToolchainError> {
    if let Some(path) = which::which(name).ok() {
        return Ok(path);
    }
    for dir in std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
    {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(ToolchainError::NotFound(name.to_string()))
}

fn find_arm_gcc() -> Result<PathBuf, ToolchainError> {
    if let Ok(p) = find_executable("arm-none-eabi-gcc") {
        return Ok(p);
    }
    if let Ok(p) = find_arm_gcc_in_cubeide_roots() {
        return Ok(p);
    }
    Err(ToolchainError::NotFound(
        "arm-none-eabi-gcc".to_string(),
    ))
}

/// STM32CubeIDE bundles GNU Tools for STM32 under `plugins/.../tools/bin`.
fn find_arm_gcc_in_cubeide_roots() -> Result<PathBuf, ToolchainError> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(root) = std::env::var("STM32CUBEIDE_ROOT") {
        roots.push(PathBuf::from(root));
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("STM32CubeIDE"));
    }
    roots.push(PathBuf::from("/opt/st"));
    for root in roots {
        if let Ok(found) = search_gcc_in_cubeide_tree(&root) {
            return Ok(found);
        }
    }
    Err(ToolchainError::NotFound(
        "arm-none-eabi-gcc".to_string(),
    ))
}

fn search_gcc_in_cubeide_tree(root: &Path) -> Result<PathBuf, ToolchainError> {
    if !root.is_dir() {
        return Err(ToolchainError::NotFound(
            "arm-none-eabi-gcc".to_string(),
        ));
    }
    // Direct install root (e.g. STM32CUBEIDE_ROOT=/opt/st/stm32cubeide_2.1.1)
    if root
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("stm32cubeide"))
    {
        return search_gcc_in_dir(root);
    }
    // /opt/st — scan stm32cubeide_* siblings
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut installs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("stm32cubeide"))
            })
            .collect();
        installs.sort();
        installs.reverse();
        for install in installs {
            if let Ok(found) = search_gcc_in_dir(&install) {
                return Ok(found);
            }
        }
    }
    search_gcc_in_dir(root)
}

fn search_gcc_in_dir(root: &Path) -> Result<PathBuf, ToolchainError> {
    if !root.is_dir() {
        return Err(ToolchainError::NotFound(
            "arm-none-eabi-gcc".to_string(),
        ));
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().and_then(|n| n.to_str()) == Some("arm-none-eabi-gcc") {
                    return Ok(path);
                }
            }
        }
    }
    Err(ToolchainError::NotFound(
        "arm-none-eabi-gcc".to_string(),
    ))
}

fn read_gcc_version(gcc: &Path) -> Result<String, ToolchainError> {
    let output = Command::new(gcc)
        .arg("--version")
        .output()
        .map_err(|e| ToolchainError::ExecFailed {
            tool: gcc.display().to_string(),
            message: e.to_string(),
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next().unwrap_or("unknown").trim();
    Ok(first.to_string())
}

mod which {
    use std::path::PathBuf;

    pub fn which(name: &str) -> Result<PathBuf, ()> {
        for dir in std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
    {
            let p = dir.join(name);
            if p.is_file() {
                return Ok(p);
            }
        }
        Err(())
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
