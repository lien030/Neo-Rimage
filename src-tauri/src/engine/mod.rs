//! Tauri-independent single-image orchestration.
//!
//! This module directly composes rimage library codecs/operations with
//! neo-rimage-owned validation, progress/cancellation boundaries, and atomic
//! output handling. It never invokes a CLI, executable, sidecar, or shell.

mod capability;
mod input;
mod local;
mod output;
mod pipeline;
mod runtime;
mod svg;
mod validation;

pub use capability::{
    engine_capabilities, EngineCapabilitySet, RIMAGE_SOURCE_REPOSITORY, RIMAGE_SOURCE_REVISION,
    RIMAGE_SOURCE_VERSION,
};
pub use input::{PrepareError, PreparedInput};
pub use local::{Engine, LocalEngine};
pub(crate) use pipeline::encoder_output_extension;
pub use runtime::{AtomicCancellationToken, EngineContext, NeverCancelled, NoopProgressReporter};
