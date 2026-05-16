//! M06 — Build Orchestrator for STM32CubeIDE `make` projects.

pub mod diagnostics;
pub mod orchestrator;
pub mod toolchain;

pub use diagnostics::parse_diagnostics;
pub use orchestrator::{
    find_elf_in_dir, resolve_build_dir, stub_sample_manifest, BuildOrchestrator,
    ERROR_BUILD_CANCELLED, ERROR_ELF_NOT_FOUND, ERROR_MAKEFILE_MISSING, ERROR_PROCESS_SPAWN_FAILED,
    ERROR_TOOLCHAIN_MISSING,
};
pub use toolchain::{detect_toolchain_info, ToolchainConfig, ToolchainError};
