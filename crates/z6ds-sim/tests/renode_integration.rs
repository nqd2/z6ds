//! TC-M09-* integration — requires built sample ELF and Renode on PATH.

use std::path::PathBuf;
use z6ds_core::contracts::{BoardConfig, SCHEMA_VERSION_SIMULATOR};
use z6ds_core::EventBus;
use z6ds_sim::SessionController;

fn sample_elf() -> Option<PathBuf> {
    let root = z6ds_core::sample_project_root();
    let elf = root.join("Debug/week7_3_2.elf");
    if elf.is_file() {
        return Some(elf);
    }
    if let Ok(rd) = std::fs::read_dir(root.join("Debug")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "elf") {
                return Some(p);
            }
        }
    }
    None
}

#[test]
#[ignore = "requires Renode + arm-none-eabi built sample-prj/Debug/*.elf"]
fn tc_m09_01_engine_start_sample_elf() {
    let elf = sample_elf().expect("build sample-prj Debug first");
    let bus = EventBus::new();
    let ctrl = SessionController::new(bus);
    let req = z6ds_core::contracts::SimulatorRunRequest {
        schema_version: SCHEMA_VERSION_SIMULATOR,
        elf_path: elf.to_string_lossy().into_owned(),
        netlist_ref: None,
        board_config: Some(BoardConfig::lab_disc1_defaults()),
        session_options: Default::default(),
    };
    let state = ctrl.run(req, None).expect("engine start");
    assert_eq!(state.status, "running");
    let stopped = ctrl.stop().expect("stop");
    assert_eq!(stopped.status, "stopped");
}
