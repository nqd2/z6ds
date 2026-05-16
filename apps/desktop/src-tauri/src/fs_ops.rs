//! M03 — project file read/write/list (black-box FS layer).

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("path not found: {0}")]
    NotFound(String),
    #[error("path is not a directory: {0}")]
    NotDirectory(String),
    #[error("path is not a file: {0}")]
    NotFile(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid path")]
    InvalidPath,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub fn list_dir(path: &str) -> Result<Vec<FsEntry>, FsError> {
    let dir = Path::new(path);
    if !dir.exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    if !dir.is_dir() {
        return Err(FsError::NotDirectory(path.to_string()));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        entries.push(FsEntry {
            path: entry.path().to_string_lossy().into_owned(),
            name,
            is_dir: meta.is_dir(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read_file(path: &str) -> Result<String, FsError> {
    let file = Path::new(path);
    if !file.exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    if !file.is_file() {
        return Err(FsError::NotFile(path.to_string()));
    }
    Ok(fs::read_to_string(file)?)
}

pub fn write_file(path: &str, contents: &str) -> Result<(), FsError> {
    let file = Path::new(path);
    if file.exists() && !file.is_file() {
        return Err(FsError::NotFile(path.to_string()));
    }
    if let Some(parent) = file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(file, contents)?;
    Ok(())
}

/// Resolve `child` under `root` and reject path traversal.
pub fn resolve_under_root(root: &str, child: &str) -> Result<PathBuf, FsError> {
    let root = Path::new(root)
        .canonicalize()
        .map_err(|_| FsError::NotFound(root.to_string()))?;
    let joined = root.join(child);
    let normalized = normalize_path(&joined);
    if !normalized.starts_with(&root) {
        return Err(FsError::InvalidPath);
    }
    Ok(normalized)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(c) => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_under_root_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        std::fs::write(dir.path().join("secret.txt"), "x").unwrap();
        let err = resolve_under_root(root, "../secret.txt").unwrap_err();
        assert!(matches!(err, FsError::InvalidPath | FsError::NotFound(_)));
    }

    #[test]
    fn list_read_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        let file_path = dir.path().join("hello.txt");
        write_file(file_path.to_str().unwrap(), "Hello").unwrap();

        let entries = list_dir(root).unwrap();
        assert!(entries.iter().any(|e| e.name == "hello.txt"));

        let text = read_file(file_path.to_str().unwrap()).unwrap();
        assert_eq!(text, "Hello");
    }
}
