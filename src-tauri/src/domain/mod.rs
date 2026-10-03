//! Shared domain vocabulary for the local engine, JobManager, and Tauri adapter.
//!
//! These types intentionally contain no Tauri or CLI dependencies. IPC-facing
//! enums use stable string tags; internal scheduling and engine modules may add
//! private state without changing these contracts.

mod capability;
mod config;
mod error;
mod ids;
mod ipc;
mod progress;
mod snapshot;

#[cfg(test)]
mod contracts;

pub use capability::*;
pub use config::*;
pub use error::*;
pub use ids::*;
pub use ipc::*;
pub use progress::*;
pub use snapshot::*;

/// First formal neo-rimage IPC schema.
pub const IPC_SCHEMA_VERSION: u16 = 2;

/// Version of the normalized job configuration stored by JobManager.
pub const JOB_CONFIG_VERSION: u16 = 1;

pub const DEFAULT_ITEM_PAGE_SIZE: u32 = 200;
pub const MAX_ITEM_PAGE_SIZE: u32 = 1_000;
