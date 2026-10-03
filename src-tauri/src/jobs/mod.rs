//! In-process job scheduling and bounded task execution.
//!
//! This module deliberately has no Tauri dependency. [`JobManager`] owns all
//! observable queue, task, progress, cancellation, and logical worker-slot
//! state. The image engine is connected through the small [`Executor`] trait.

mod executor;
mod manager;
mod types;

pub use executor::{
    CancellationToken, ExecutionContext, ExecutionOutcome, ExecutionTask, Executor,
    PreparationOutcome, PreparedExecution, ProgressReporter,
};
pub use manager::{Clock, JobManager, SystemClock};
pub use types::{
    ClearResult, JobSubmission, ManagerError, RetryMode, RevisionSubscription, ShutdownReport,
};

#[cfg(test)]
mod tests;
