//! z6ds-core — shared contracts, M00 event bus, M02 discovery, M04 IOC parser.

pub mod contracts;
pub mod discovery;
pub mod event_bus;
pub mod ioc;
pub mod netlist;

pub use contracts::*;
pub use discovery::{
    discover_and_publish, discover_project, refresh_project, sample_project_root,
    validate_manifest,
};
pub use event_bus::EventBus;
pub use ioc::parse_ioc_file;
pub use netlist::{
    build_defaults_from_board, NetlistDocument, NetlistModule, NetlistStore, NetlistWire,
    ValidationResult,
};
