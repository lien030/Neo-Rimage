use std::any::Any;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::domain::{
    AppError, CancellationProbe, EngineProgressEvent, EngineResult, ItemSpec, JobSpec,
    ProgressReporter as DomainProgressReporter,
};

/// A single immutable execution request handed from the scheduler to an engine
/// adapter. The attempt is owned by JobManager and can differ from the original
/// normalized `ItemSpec` after a retry.
#[derive(Clone, Debug)]
pub struct ExecutionTask {
    pub job: Arc<JobSpec>,
    pub item: Arc<ItemSpec>,
    pub attempt: u32,
}

#[derive(Debug)]
struct CancellationState {
    cancelled: AtomicBool,
    wait_lock: Mutex<()>,
    wait_signal: Condvar,
}

/// A cheap, cloneable cooperative cancellation token.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            state: Arc::new(CancellationState {
                cancelled: AtomicBool::new(false),
                wait_lock: Mutex::new(()),
                wait_signal: Condvar::new(),
            }),
        }
    }

    pub fn cancel(&self) {
        if !self.state.cancelled.swap(true, Ordering::AcqRel) {
            self.state.wait_signal.notify_all();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    /// Wait until cancellation is requested or the timeout expires. This is
    /// useful for deterministic fakes; production engines normally poll at
    /// documented safe stage boundaries.
    pub fn wait_cancelled(&self, timeout: Duration) -> bool {
        if self.is_cancelled() {
            return true;
        }

        let deadline = Instant::now() + timeout;
        let mut guard = self
            .state
            .wait_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        while !self.is_cancelled() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }

            let waited = self
                .state
                .wait_signal
                .wait_timeout(guard, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = waited.0;
            if waited.1.timed_out() && !self.is_cancelled() {
                return false;
            }
        }

        true
    }
}

impl CancellationProbe for CancellationToken {
    fn is_cancel_requested(&self) -> bool {
        self.is_cancelled()
    }
}

/// Lightweight progress hand-off supplied to an [`Executor`].
#[derive(Clone)]
pub struct ProgressReporter {
    report: Arc<dyn Fn(EngineProgressEvent) + Send + Sync>,
}

impl fmt::Debug for ProgressReporter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProgressReporter(..)")
    }
}

impl ProgressReporter {
    pub(crate) fn new(report: impl Fn(EngineProgressEvent) + Send + Sync + 'static) -> Self {
        Self {
            report: Arc::new(report),
        }
    }

    pub fn report(&self, progress: EngineProgressEvent) {
        (self.report)(progress);
    }
}

impl DomainProgressReporter for ProgressReporter {
    fn report(&self, event: EngineProgressEvent) {
        (self.report)(event);
    }
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub cancellation: CancellationToken,
    pub progress: ProgressReporter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Succeeded(EngineResult),
    Failed(AppError),
    Cancelled,
    Skipped,
}

#[derive(Clone)]
pub struct PreparedExecution {
    pub memory_bytes: u64,
    pub payload: Arc<dyn Any + Send + Sync>,
}

#[allow(clippy::large_enum_variant)]
pub enum PreparationOutcome {
    Ready(PreparedExecution),
    NeedsBudget(u64),
    Failed(AppError),
    Cancelled,
}

/// Object-safe integration boundary between JobManager and the local engine.
///
/// The implementation in the integration layer converts `ExecutionTask` into
/// the engine's request type and maps its result to `ExecutionOutcome`.
pub trait Executor: Send + Sync + 'static {
    fn prepare(
        &self,
        task: &ExecutionTask,
        budget: u64,
        cancellation: &CancellationToken,
    ) -> PreparationOutcome;
    fn execute(
        &self,
        task: &ExecutionTask,
        plan: &PreparedExecution,
        context: &ExecutionContext,
    ) -> ExecutionOutcome;
}
