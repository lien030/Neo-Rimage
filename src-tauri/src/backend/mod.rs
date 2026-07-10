//! Tauri-independent integration services.
//!
//! This layer converts formal IPC requests into normalized JobManager input,
//! connects the local engine to the scheduler, and translates internal errors
//! into stable domain errors. It owns no queue or execution state.

mod error;
mod executor;
mod normalize;
mod service;

pub use error::{command_error, manager_command_error, manager_error};
pub use executor::{build_engine_request, LocalEngineExecutor};
pub use normalize::{NormalizationOptions, NormalizedJob, RequestNormalizer};
pub use service::BackendService;

#[cfg(test)]
mod tests;
