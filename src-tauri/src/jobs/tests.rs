use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::domain::{
    AppError, BackupPolicy, CollisionPolicy, EncoderConfig, EngineProgressEvent, EngineResult,
    ErrorCategory, ImageFormat, ImageProperties, ItemId, ItemProgress, ItemSpec, ItemStatus, JobId,
    JobSpec, JobStatus, MetadataOutcome, MetadataPolicy, MozJpegConfig, Operation, OutputLocation,
    OutputPlan, OutputPolicy, ProcessingStage, ProgressMeasure, SchedulerMode, TimestampMs,
    WorkerSlotStatus, JOB_CONFIG_VERSION,
};

use super::{
    ExecutionContext, ExecutionOutcome, ExecutionTask, Executor, JobManager, JobSubmission,
    ManagerError, RetryMode,
};

const TEST_TIMEOUT: Duration = Duration::from_secs(3);

struct SequenceExecutor {
    outcomes: Mutex<VecDeque<ExecutionOutcome>>,
}

impl SequenceExecutor {
    fn new(outcomes: Vec<ExecutionOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
        }
    }
}

impl Executor for SequenceExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        _budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: 1,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        _context: &ExecutionContext,
    ) -> ExecutionOutcome {
        let mut outcome = self
            .outcomes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| ExecutionOutcome::Succeeded(engine_result(task)));
        if let ExecutionOutcome::Succeeded(result) = &mut outcome {
            result.job_id = task.job.id.clone();
            result.item_id = task.item.id.clone();
            result.attempt = task.attempt;
            result.input_path = task.item.input_path.clone();
            result.output_path = task.item.output.output_path.clone();
        }
        outcome
    }
}

#[derive(Default)]
struct PermitGate {
    permits: Mutex<usize>,
    signal: Condvar,
}

impl PermitGate {
    fn release(&self, permits: usize) {
        let mut current = self
            .permits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current = current.saturating_add(permits);
        self.signal.notify_all();
    }

    fn acquire(&self, context: &ExecutionContext) -> bool {
        let mut current = self
            .permits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if context.cancellation.is_cancelled() {
                return false;
            }
            if *current > 0 {
                *current -= 1;
                return true;
            }
            let waited = self
                .signal
                .wait_timeout(current, Duration::from_millis(20))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            current = waited.0;
        }
    }
}

struct GatedExecutor {
    gate: Arc<PermitGate>,
    started: Sender<ItemId>,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
}

impl GatedExecutor {
    fn new() -> (Arc<Self>, Receiver<ItemId>) {
        let (started, receiver) = mpsc::channel();
        (
            Arc::new(Self {
                gate: Arc::new(PermitGate::default()),
                started,
                active: AtomicUsize::new(0),
                maximum_active: AtomicUsize::new(0),
            }),
            receiver,
        )
    }
}

impl Executor for GatedExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        _budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: 1,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        context: &ExecutionContext,
    ) -> ExecutionOutcome {
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        self.maximum_active.fetch_max(active, Ordering::AcqRel);
        let _ = self.started.send(task.item.id.clone());
        let acquired = self.gate.acquire(context);
        self.active.fetch_sub(1, Ordering::AcqRel);
        if acquired {
            ExecutionOutcome::Succeeded(engine_result(task))
        } else {
            ExecutionOutcome::Cancelled
        }
    }
}

struct CancellationExecutor {
    started: Sender<ItemId>,
}

impl Executor for CancellationExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        _budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: 1,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        context: &ExecutionContext,
    ) -> ExecutionOutcome {
        let _ = self.started.send(task.item.id.clone());
        if context.cancellation.wait_cancelled(TEST_TIMEOUT) {
            ExecutionOutcome::Cancelled
        } else {
            ExecutionOutcome::Failed(AppError::new(
                "test.cancel_timeout",
                ErrorCategory::Internal,
                "errors.testCancelTimeout",
                "test executor did not receive cancellation",
            ))
        }
    }
}

struct ProgressExecutor;

impl Executor for ProgressExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        _budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: 1,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        context: &ExecutionContext,
    ) -> ExecutionOutcome {
        let event = EngineProgressEvent::StageProgress {
            progress: ItemProgress {
                stage: ProcessingStage::Encode,
                measure: ProgressMeasure::Fraction {
                    completed: 1,
                    total: 2,
                },
            },
        };
        context.progress.report(event.clone());
        context.progress.report(event);
        ExecutionOutcome::Succeeded(engine_result(task))
    }
}

struct PanicExecutor;

impl Executor for PanicExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        _budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: 1,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        _task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        _context: &ExecutionContext,
    ) -> ExecutionOutcome {
        panic!("intentional fake executor panic");
    }
}

struct BadIdentityExecutor;

impl Executor for BadIdentityExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        _budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: 1,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        _task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        _context: &ExecutionContext,
    ) -> ExecutionOutcome {
        ExecutionOutcome::Succeeded(blank_result())
    }
}

#[test]
fn completes_job_and_exposes_authoritative_snapshot() {
    let manager =
        JobManager::new(Arc::new(SequenceExecutor::new(Vec::new())), 2).expect("create manager");
    let job_id = manager
        .submit(submission("lifecycle", 2))
        .expect("submit job");
    let job = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");

    assert_eq!(job.status, JobStatus::Succeeded);
    assert_eq!(job.counts.succeeded, 2);
    assert_eq!(job.counts.total, 2);
    assert!(job.controls.can_remove);
    let snapshot = manager.snapshot();
    assert!(snapshot.revision.0 >= 5);
    assert_eq!(snapshot.scheduler.active_items, 0);
    assert_eq!(snapshot.jobs, vec![job]);
}

#[test]
fn active_execution_never_exceeds_the_configured_budget() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::with_concurrency(executor.clone(), 2, 4).expect("create manager");
    let job_id = manager
        .submit(submission("bounded", 8))
        .expect("submit job");

    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("first item starts");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("second item starts");
    let running = manager.snapshot();
    assert_eq!(running.scheduler.active_items, 2);
    assert_eq!(running.worker_slots.len(), 2);
    assert!(running.worker_slots.iter().all(|slot| {
        slot.input_path
            .as_deref()
            .is_some_and(|path| path.starts_with("input-") && path.ends_with(".png"))
    }));

    executor.gate.release(8);
    manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");
    assert!(executor.maximum_active.load(Ordering::Acquire) <= 2);
}

#[test]
fn lowering_concurrency_marks_busy_slots_as_draining() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::with_concurrency(executor.clone(), 3, 3).expect("create manager");
    let job_id = manager.submit(submission("retire", 3)).expect("submit job");
    for _ in 0..3 {
        started.recv_timeout(TEST_TIMEOUT).expect("item starts");
    }

    manager.set_desired_concurrency(1).expect("lower budget");
    let snapshot = manager.snapshot();
    assert_eq!(snapshot.worker_slots.len(), 3);
    assert_eq!(snapshot.worker_slots[0].status, WorkerSlotStatus::Busy);
    assert_eq!(snapshot.worker_slots[1].status, WorkerSlotStatus::Draining);
    assert_eq!(snapshot.worker_slots[2].status, WorkerSlotStatus::Draining);
    assert!(snapshot
        .worker_slots
        .iter()
        .all(|slot| slot.input_path.is_some()));

    executor.gate.release(3);
    manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");
    let idle = manager.snapshot();
    assert_eq!(idle.worker_slots.len(), 1);
    assert_eq!(idle.worker_slots[0].input_path, None);
}

#[test]
fn global_scheduler_pause_keeps_concurrency_configuration_and_queue_intact() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::with_concurrency(executor.clone(), 2, 4).expect("create manager");
    manager.set_scheduler_paused(true).expect("pause scheduler");
    let job_id = manager
        .submit(submission("scheduler-paused", 3))
        .expect("submit job");

    assert!(started.recv_timeout(Duration::from_millis(80)).is_err());
    manager.set_desired_concurrency(3).expect("adjust budget");
    let paused = manager.snapshot();
    assert_eq!(paused.scheduler.mode, SchedulerMode::Paused);
    assert_eq!(paused.scheduler.desired_concurrency, 3);
    assert_eq!(paused.scheduler.active_items, 0);
    assert_eq!(paused.scheduler.queued_items, 3);

    manager
        .set_scheduler_paused(false)
        .expect("resume scheduler");
    for _ in 0..3 {
        started.recv_timeout(TEST_TIMEOUT).expect("item starts");
    }
    executor.gate.release(3);
    manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");
    assert_eq!(manager.snapshot().scheduler.mode, SchedulerMode::Running);
}

#[test]
fn global_scheduler_pause_drains_running_work_before_resume() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let job_id = manager
        .submit(submission("scheduler-drain", 2))
        .expect("submit job");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("first item starts");
    manager.set_scheduler_paused(true).expect("pause scheduler");

    executor.gate.release(1);
    assert!(started.recv_timeout(Duration::from_millis(80)).is_err());
    let drained = manager.snapshot();
    assert_eq!(drained.scheduler.mode, SchedulerMode::Paused);
    assert_eq!(drained.scheduler.active_items, 0);
    assert_eq!(drained.scheduler.queued_items, 1);
    assert_eq!(drained.scheduler.desired_concurrency, 1);

    manager
        .set_scheduler_paused(false)
        .expect("resume scheduler");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("second item starts");
    executor.gate.release(1);
    manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");
}

#[test]
fn pause_stops_new_dispatch_and_resume_continues() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let job_id = manager.submit(submission("pause", 2)).expect("submit job");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("first item starts");

    manager.pause_job(&job_id).expect("pause job");
    assert_eq!(
        manager.job_snapshot(&job_id).expect("snapshot").status,
        JobStatus::Paused
    );
    executor.gate.release(1);
    assert!(started.recv_timeout(Duration::from_millis(80)).is_err());
    assert_eq!(
        manager.job_snapshot(&job_id).expect("snapshot").status,
        JobStatus::Paused
    );

    manager.resume_job(&job_id).expect("resume job");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("second item starts");
    executor.gate.release(1);
    assert_eq!(
        manager
            .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
            .expect("job completes")
            .status,
        JobStatus::Succeeded
    );
}

#[test]
fn paused_job_can_finish_when_already_running_items_drain() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let job_id = manager
        .submit(submission("pause-drain", 1))
        .expect("submit job");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("running item starts");
    manager.pause_job(&job_id).expect("pause job");

    executor.gate.release(1);
    assert_eq!(
        manager
            .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
            .expect("drained job completes")
            .status,
        JobStatus::Succeeded
    );
}

#[test]
fn cancelling_a_drained_paused_job_uses_the_cancelling_transition() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let job_id = manager
        .submit(submission("pause-cancel", 2))
        .expect("submit job");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("running item starts");
    manager.pause_job(&job_id).expect("pause job");
    executor.gate.release(1);
    assert!(started.recv_timeout(Duration::from_millis(80)).is_err());

    manager.cancel_job(&job_id).expect("cancel paused job");
    let terminal = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("paused cancellation completes");
    assert_eq!(terminal.status, JobStatus::PartiallySucceeded);
    assert_eq!(terminal.counts.succeeded, 1);
    assert_eq!(terminal.counts.cancelled, 1);
}

#[test]
fn cancellation_is_cooperative_and_cancels_queued_items_immediately() {
    let (started_tx, started_rx) = mpsc::channel();
    let manager = JobManager::new(
        Arc::new(CancellationExecutor {
            started: started_tx,
        }),
        1,
    )
    .expect("create manager");
    let job_id = manager.submit(submission("cancel", 3)).expect("submit job");
    started_rx.recv_timeout(TEST_TIMEOUT).expect("item starts");

    manager.cancel_job(&job_id).expect("request cancellation");
    let cancelling = manager.job_snapshot(&job_id).expect("snapshot");
    assert!(matches!(
        cancelling.status,
        JobStatus::Cancelling | JobStatus::Cancelled
    ));
    assert!(cancelling.counts.cancelled >= 2);
    let terminal = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("cancellation completes");
    assert_eq!(terminal.status, JobStatus::Cancelled);
    assert_eq!(terminal.counts.cancelled, 3);
}

#[test]
fn queued_item_can_be_cancelled_without_cancelling_the_whole_job() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let job_id = manager
        .submit(submission("cancel-item", 2))
        .expect("submit job");
    started
        .recv_timeout(TEST_TIMEOUT)
        .expect("first item starts");
    let queued_item = ItemId::new(format!("{job_id}-item-2"));

    manager
        .cancel_item(&queued_item)
        .expect("cancel queued item");
    executor.gate.release(1);
    let terminal = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("remaining item completes");
    assert_eq!(terminal.status, JobStatus::PartiallySucceeded);
    assert_eq!(terminal.counts.succeeded, 1);
    assert_eq!(terminal.counts.cancelled, 1);
}

#[test]
fn retry_increments_attempt_without_rewriting_successful_history() {
    let failure = AppError::new(
        "test.transient",
        ErrorCategory::Output,
        "errors.testTransient",
        "transient test failure",
    )
    .with_retryable(true);
    let executor = SequenceExecutor::new(vec![
        ExecutionOutcome::Failed(failure),
        ExecutionOutcome::Succeeded(blank_result()),
    ]);
    let manager = JobManager::new(Arc::new(executor), 1).expect("create manager");
    let job_id = manager.submit(submission("retry", 1)).expect("submit job");
    let failed = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("first attempt ends");
    assert_eq!(failed.status, JobStatus::Failed);

    assert_eq!(
        manager
            .retry_job(&job_id, RetryMode::FailedOnly)
            .expect("retry"),
        1
    );
    let succeeded = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("retry completes");
    assert_eq!(succeeded.status, JobStatus::Succeeded);
    let details = manager
        .job_detail_snapshot(&job_id, 0, 10)
        .expect("details");
    assert_eq!(details.items.items[0].attempt, 2);
    assert_eq!(details.items.items[0].status, ItemStatus::Succeeded);
}

#[test]
fn retry_item_requeues_only_the_selected_retryable_item() {
    let failure = AppError::new(
        "test.item_transient",
        ErrorCategory::Output,
        "errors.testItemTransient",
        "transient item failure",
    )
    .with_retryable(true);
    let executor = SequenceExecutor::new(vec![
        ExecutionOutcome::Failed(failure),
        ExecutionOutcome::Succeeded(blank_result()),
        ExecutionOutcome::Succeeded(blank_result()),
    ]);
    let manager = JobManager::new(Arc::new(executor), 1).expect("create manager");
    let job_id = manager
        .submit(submission("retry-item", 2))
        .expect("submit job");
    let initial = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("initial job completes");
    assert_eq!(initial.status, JobStatus::PartiallySucceeded);

    let failed_item = ItemId::new(format!("{job_id}-item-1"));
    manager.retry_item(&failed_item).expect("retry failed item");
    let retried = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("retried job completes");
    assert_eq!(retried.status, JobStatus::Succeeded);
    let detail = manager
        .job_detail_snapshot(&job_id, 0, 10)
        .expect("job detail");
    assert_eq!(detail.items.items[0].attempt, 2);
    assert_eq!(detail.items.items[1].attempt, 1);
}

#[test]
fn duplicate_progress_is_coalesced_and_revision_stays_monotonic() {
    let manager = JobManager::new(Arc::new(ProgressExecutor), 1).expect("create manager");
    let job_id = manager
        .submit(submission("progress", 1))
        .expect("submit job");
    manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");

    // submit + dispatch + one unique progress value + terminal transition
    assert_eq!(manager.snapshot().revision.0, 6);
}

#[test]
fn bounded_revision_subscription_is_a_snapshot_wakeup_not_an_event_log() {
    let manager = JobManager::new(Arc::new(ProgressExecutor), 1).expect("create manager");
    let subscription = manager.subscribe_revisions();
    let job_id = manager
        .submit(submission("subscription", 1))
        .expect("submit job");

    let notified = subscription
        .recv_timeout(TEST_TIMEOUT)
        .expect("revision notification");
    let terminal = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("job completes");
    let snapshot = manager.snapshot();
    assert!(snapshot.revision >= notified);
    assert_eq!(snapshot.jobs[0], terminal);
    let (revision, occurred_at) = manager.observation_revision();
    assert_eq!(revision, snapshot.revision);
    assert!(occurred_at >= snapshot.generated_at);
}

#[test]
fn observation_revision_tracks_paused_manager_changes() {
    let manager = JobManager::new(Arc::new(ProgressExecutor), 2).expect("create manager");
    manager.set_scheduler_paused(true).expect("pause manager");
    let initial_revision = manager.observation_revision().0;
    manager
        .submit(submission("revision", 3))
        .expect("submit job");
    let submitted_revision = manager.observation_revision().0;
    assert!(submitted_revision > initial_revision);
    assert_eq!(submitted_revision, manager.snapshot().revision);
    manager
        .set_desired_concurrency(2)
        .expect("change concurrency");
    assert!(manager.observation_revision().0 > submitted_revision);
}

#[test]
fn executor_panic_becomes_a_terminal_internal_error() {
    let manager = JobManager::new(Arc::new(PanicExecutor), 1).expect("create manager");
    let job_id = manager.submit(submission("panic", 1)).expect("submit job");
    let terminal = manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("panic is contained");
    assert_eq!(terminal.status, JobStatus::Failed);

    let details = manager
        .job_detail_snapshot(&job_id, 0, 10)
        .expect("details");
    let error = details.items.items[0]
        .error
        .as_ref()
        .expect("internal error");
    assert_eq!(error.category, ErrorCategory::Internal);
    assert_eq!(error.code.0, "executor.panicked");
}

#[test]
fn mismatched_executor_result_identity_is_rejected() {
    let manager = JobManager::new(Arc::new(BadIdentityExecutor), 1).expect("create manager");
    let job_id = manager
        .submit(submission("identity", 1))
        .expect("submit job");
    assert_eq!(
        manager
            .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
            .expect("job terminates")
            .status,
        JobStatus::Failed
    );
    let detail = manager
        .job_detail_snapshot(&job_id, 0, 1)
        .expect("job detail");
    assert_eq!(
        detail.items.items[0]
            .error
            .as_ref()
            .expect("identity error")
            .code
            .0,
        "executor.result_identity_mismatch"
    );
}

#[test]
fn clear_only_removes_terminal_jobs() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let running_id = manager
        .submit(submission("running", 1))
        .expect("submit running");
    started.recv_timeout(TEST_TIMEOUT).expect("item starts");

    let immediate = Arc::new(SequenceExecutor::new(Vec::new()));
    let second_manager = JobManager::new(immediate, 1).expect("create second manager");
    let terminal_id = second_manager
        .submit(submission("terminal", 1))
        .expect("submit terminal");
    second_manager
        .wait_for_job_terminal(&terminal_id, TEST_TIMEOUT)
        .expect("terminal job");

    assert!(manager.clear_terminal_jobs(None).cleared_job_ids.is_empty());
    assert_eq!(manager.snapshot().jobs[0].id, running_id);
    assert_eq!(
        second_manager.clear_terminal_jobs(None).cleared_job_ids,
        vec![terminal_id]
    );
    assert!(second_manager.snapshot().jobs.is_empty());
    executor.gate.release(1);
    manager
        .wait_for_job_terminal(&running_id, TEST_TIMEOUT)
        .expect("running job completes");
}

#[test]
fn graceful_shutdown_cancels_work_and_rejects_new_jobs() {
    let (started_tx, started_rx) = mpsc::channel();
    let manager = JobManager::new(
        Arc::new(CancellationExecutor {
            started: started_tx,
        }),
        1,
    )
    .expect("create manager");
    let job_id = manager
        .submit(submission("shutdown", 2))
        .expect("submit job");
    started_rx.recv_timeout(TEST_TIMEOUT).expect("item starts");

    let report = manager.shutdown(TEST_TIMEOUT);
    assert!(report.graceful);
    assert!(report.unfinished_item_ids.is_empty());
    assert_eq!(
        manager.job_snapshot(&job_id).expect("snapshot").status,
        JobStatus::Cancelled
    );
    assert_eq!(
        manager.submit(submission("after-shutdown", 1)),
        Err(ManagerError::ShuttingDown)
    );
}

#[test]
fn shutdown_timeout_reports_items_that_have_not_reached_a_safe_point() {
    let (executor, started) = GatedExecutor::new();
    let manager = JobManager::new(executor.clone(), 1).expect("create manager");
    let job_id = manager
        .submit(submission("shutdown-timeout", 1))
        .expect("submit job");
    let item_id = started.recv_timeout(TEST_TIMEOUT).expect("item starts");

    let report = manager.shutdown(Duration::ZERO);
    assert!(!report.graceful);
    assert_eq!(report.unfinished_item_ids, vec![item_id]);
    assert_eq!(
        manager.job_snapshot(&job_id).expect("snapshot").status,
        JobStatus::Cancelling
    );

    // The fake checks cancellation while waiting, so it can now finish its
    // cooperative cleanup without needing an execution permit.
    manager
        .wait_for_job_terminal(&job_id, TEST_TIMEOUT)
        .expect("cancel eventually completes");
}

fn submission(name: &str, item_count: usize) -> JobSubmission {
    let job_id = JobId::new(format!("job-{name}"));
    let output = OutputPolicy {
        location: OutputLocation::SameDirectory,
        preserve_structure: false,
        suffix: "-optimized".to_owned(),
        collision: CollisionPolicy::Replace,
        source_backup: BackupPolicy::Disabled,
        existing_output_backup: BackupPolicy::Disabled,
    };
    let job = JobSpec {
        id: job_id.clone(),
        created_at: TimestampMs(1),
        config_version: JOB_CONFIG_VERSION,
        operations: Vec::<Operation>::new(),
        encoder: EncoderConfig::MozJpeg(MozJpegConfig::default()),
        output,
        metadata: MetadataPolicy::default(),
        scheduling: None,
    };
    let items = (0..item_count)
        .map(|index| ItemSpec {
            id: ItemId::new(format!("{job_id}-item-{}", index + 1)),
            job_id: job_id.clone(),
            sequence: u32::try_from(index).unwrap_or(u32::MAX),
            attempt: 1,
            input_path: PathBuf::from(format!("input-{index}.png")),
            scan_root: None,
            output: OutputPlan {
                output_path: PathBuf::from(format!("output-{index}.jpg")),
                collision: CollisionPolicy::Replace,
                source_backup_path: None,
                existing_output_backup_path: None,
            },
        })
        .collect();
    JobSubmission::new(job, items)
}

fn engine_result(task: &ExecutionTask) -> EngineResult {
    let mut result = blank_result();
    result.job_id = task.job.id.clone();
    result.item_id = task.item.id.clone();
    result.attempt = task.attempt;
    result.input_path = task.item.input_path.clone();
    result.output_path = task.item.output.output_path.clone();
    result
}

fn blank_result() -> EngineResult {
    let properties = ImageProperties {
        format: ImageFormat::Jpeg,
        width: 1,
        height: 1,
        bit_depth: Some(8),
        color_space: Some("rgb".to_owned()),
        has_alpha: Some(false),
        frame_count: Some(1),
    };
    EngineResult {
        job_id: JobId::default(),
        item_id: ItemId::default(),
        attempt: 1,
        input_path: PathBuf::new(),
        output_path: PathBuf::new(),
        input: properties.clone(),
        output: properties,
        input_bytes: 100,
        output_bytes: 50,
        duration_ms: 1,
        metadata: MetadataOutcome::default(),
        warnings: Vec::new(),
        stage_durations_ms: Default::default(),
    }
}

struct ResourceExecutor {
    requirements: Vec<u64>,
    gates: Mutex<std::collections::HashMap<ItemId, Arc<PermitGate>>>,
    started: Sender<ItemId>,
    prepared: Sender<(ItemId, u32)>,
    preparation_gate: Option<Arc<PermitGate>>,
    preparation_fault: Option<&'static str>,
}

impl Executor for ResourceExecutor {
    fn prepare(
        &self,
        task: &ExecutionTask,
        budget: u64,
        cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        let _ = self.prepared.send((task.item.id.clone(), task.attempt));
        if let Some(gate) = &self.preparation_gate {
            if !gate.acquire(&ExecutionContext {
                cancellation: cancellation.clone(),
                progress: super::ProgressReporter::new(|_| {}),
            }) {
                return super::PreparationOutcome::Cancelled;
            }
        }
        let required = self.requirements[task.item.sequence as usize];
        match self.preparation_fault {
            Some("panic") => panic!("preparation panic"),
            Some("invalid-budget") => return super::PreparationOutcome::NeedsBudget(budget),
            Some("expand") if required > budget => {
                return super::PreparationOutcome::NeedsBudget(required)
            }
            _ => {}
        }
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: required,
            payload: Arc::new(()),
        })
    }
    fn execute(
        &self,
        task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        context: &ExecutionContext,
    ) -> ExecutionOutcome {
        let gate = Arc::new(PermitGate::default());
        self.gates
            .lock()
            .unwrap()
            .insert(task.item.id.clone(), gate.clone());
        self.started.send(task.item.id.clone()).unwrap();
        if gate.acquire(context) {
            ExecutionOutcome::Succeeded(engine_result(task))
        } else {
            ExecutionOutcome::Cancelled
        }
    }
}

fn resource_executor(
    requirements: Vec<u64>,
    preparation_gate: Option<Arc<PermitGate>>,
) -> (
    Arc<ResourceExecutor>,
    Receiver<ItemId>,
    Receiver<(ItemId, u32)>,
) {
    let (started, started_receiver) = mpsc::channel();
    let (prepared, prepared_receiver) = mpsc::channel();
    (
        Arc::new(ResourceExecutor {
            requirements,
            gates: Mutex::new(Default::default()),
            started,
            prepared,
            preparation_gate,
            preparation_fault: None,
        }),
        started_receiver,
        prepared_receiver,
    )
}

fn wait_until(predicate: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + TEST_TIMEOUT;
    while !predicate() {
        assert!(
            std::time::Instant::now() < deadline,
            "condition did not become true"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn memory_wait_uses_no_worker_and_small_items_bypass_only_eight_times() {
    assert_memory_fairness(None);
}

#[test]
fn expanded_preparation_barrier_does_not_deadlock_a_full_preparation_window() {
    assert_memory_fairness(Some("expand"));
}

fn assert_memory_fairness(preparation_fault: Option<&'static str>) {
    let unit = 1024 * 1024;
    let mut requirements = vec![unit; 14];
    requirements[0] = if preparation_fault.is_some() {
        unit
    } else {
        3 * unit
    };
    requirements[1] = 4 * unit;
    let (mut executor, started, _) = resource_executor(requirements, None);
    Arc::get_mut(&mut executor).unwrap().preparation_fault = preparation_fault;
    let manager = JobManager::with_memory_budget(
        executor.clone(),
        Arc::new(super::SystemClock),
        2,
        2,
        4 * unit,
    )
    .unwrap();
    let job_id = manager.submit(submission("fair-memory", 14)).unwrap();
    let first = started.recv_timeout(TEST_TIMEOUT).unwrap();
    assert!(first.as_str().ends_with("item-1"));
    for index in 3..=10 {
        let small = started.recv_timeout(TEST_TIMEOUT).unwrap();
        assert!(small.as_str().ends_with(&format!("item-{index}")));
        if preparation_fault.is_some() && index == 3 {
            wait_until(|| manager.resource_state().3 >= 3 && manager.resource_state().2 == 0);
        }
        let resources = manager.resource_state();
        assert!(resources.0 <= resources.1);
        assert!(resources.2 <= 1);
        assert!(resources.3 <= 4);
        if preparation_fault.is_some() {
            assert!(resources.3 <= 3);
        }
        executor.gates.lock().unwrap()[&small].release(1);
    }
    wait_until(|| manager.snapshot().scheduler.active_items == 1);
    assert!(started.try_recv().is_err());
    let waiting = manager.job_snapshot(&job_id).unwrap();
    assert!(waiting.counts.waiting_for_memory > 0);
    assert!(waiting.counts.waiting_for_memory <= waiting.counts.queued);
    assert_eq!(
        manager
            .snapshot()
            .worker_slots
            .iter()
            .filter(|slot| slot.status == WorkerSlotStatus::Idle)
            .count(),
        1
    );
    executor.gates.lock().unwrap()[&first].release(1);
    let large = started.recv_timeout(TEST_TIMEOUT).unwrap();
    assert!(large.as_str().ends_with("item-2"));
    assert!(manager.shutdown(TEST_TIMEOUT).graceful);
    assert_eq!(manager.resource_state().0, 0);
}

#[test]
fn oversize_item_fails_instead_of_waiting_forever() {
    let (executor, started, _) = resource_executor(vec![32 * 1024 * 1024], None);
    let manager = JobManager::with_memory_budget(
        executor,
        Arc::new(super::SystemClock),
        1,
        1,
        8 * 1024 * 1024,
    )
    .unwrap();
    let job = manager.submit(submission("oversize", 1)).unwrap();
    let snapshot = manager.wait_for_job_terminal(&job, TEST_TIMEOUT).unwrap();
    assert_eq!(snapshot.status, JobStatus::Failed);
    assert!(started.try_recv().is_err());
    assert_eq!(manager.resource_state().0, 0);
}

#[test]
fn cancelled_preparation_cannot_resurrect_a_retried_or_removed_item() {
    let gate = Arc::new(PermitGate::default());
    let (executor, started, prepared) = resource_executor(vec![1024 * 1024], Some(gate.clone()));
    let manager = JobManager::with_memory_budget(
        executor,
        Arc::new(super::SystemClock),
        1,
        1,
        8 * 1024 * 1024,
    )
    .unwrap();
    let job = manager.submit(submission("stale-prepare", 1)).unwrap();
    let (item, attempt) = prepared.recv_timeout(TEST_TIMEOUT).unwrap();
    assert_eq!(attempt, 1);
    assert_eq!(manager.snapshot().scheduler.active_items, 0);
    manager.pause_job(&job).unwrap();
    manager.cancel_item(&item).unwrap();
    manager
        .retry_job(&job, RetryMode::FailedAndCancelled)
        .unwrap();
    let (_, attempt) = prepared.recv_timeout(TEST_TIMEOUT).unwrap();
    assert_eq!(attempt, 2);
    manager.pause_job(&job).unwrap();
    gate.release(1);
    wait_until(|| manager.resource_state().2 == 0);
    assert!(started.try_recv().is_err());
    assert_eq!(
        manager.job_snapshot(&job).unwrap().status,
        JobStatus::Paused
    );
    manager.resume_job(&job).unwrap();
    assert_eq!(started.recv_timeout(TEST_TIMEOUT).unwrap(), item);
    assert!(manager.shutdown(TEST_TIMEOUT).graceful);
    manager.clear_terminal_jobs(None);
    assert_eq!(manager.resource_state().0, 0);
}

#[test]
fn preparation_window_is_bounded_when_execution_is_memory_blocked() {
    let unit = 1024 * 1024;
    let (executor, started, prepared) = resource_executor(vec![6 * unit; 30], None);
    let manager =
        JobManager::with_memory_budget(executor, Arc::new(super::SystemClock), 1, 2, 8 * unit)
            .unwrap();
    manager.submit(submission("lookahead", 30)).unwrap();
    started.recv_timeout(TEST_TIMEOUT).unwrap();
    for _ in 0..5 {
        prepared.recv_timeout(TEST_TIMEOUT).unwrap();
    }
    wait_until(|| manager.resource_state().3 == 4 && manager.resource_state().2 == 0);
    assert!(prepared.try_recv().is_err());
    assert!(manager.shutdown(TEST_TIMEOUT).graceful);
    assert_eq!(manager.resource_state().0, 0);
}

#[test]
fn preparation_panics_and_invalid_expansion_release_the_reservation() {
    for fault in ["panic", "invalid-budget"] {
        let (mut executor, started, _) = resource_executor(vec![1024 * 1024], None);
        Arc::get_mut(&mut executor).unwrap().preparation_fault = Some(fault);
        let manager = JobManager::with_memory_budget(
            executor,
            Arc::new(super::SystemClock),
            1,
            1,
            8 * 1024 * 1024,
        )
        .unwrap();
        let job = manager.submit(submission(fault, 1)).unwrap();
        assert_eq!(
            manager
                .wait_for_job_terminal(&job, TEST_TIMEOUT)
                .unwrap()
                .status,
            JobStatus::Failed
        );
        assert_eq!(manager.resource_state().0, 0);
        assert!(started.try_recv().is_err());
        assert!(manager.shutdown(TEST_TIMEOUT).graceful);
    }
}

#[test]
fn preparation_expansion_returns_to_manager_without_occupying_a_worker() {
    let (mut executor, started, prepared) = resource_executor(vec![4 * 1024 * 1024], None);
    Arc::get_mut(&mut executor).unwrap().preparation_fault = Some("expand");
    let manager = JobManager::with_memory_budget(
        executor.clone(),
        Arc::new(super::SystemClock),
        1,
        1,
        8 * 1024 * 1024,
    )
    .unwrap();
    manager.submit(submission("expand", 1)).unwrap();
    prepared.recv_timeout(TEST_TIMEOUT).unwrap();
    prepared.recv_timeout(TEST_TIMEOUT).unwrap();
    let item = started.recv_timeout(TEST_TIMEOUT).unwrap();
    assert_eq!(manager.resource_state().0, 4 * 1024 * 1024);
    executor.gates.lock().unwrap()[&item].release(1);
    assert!(manager.shutdown(TEST_TIMEOUT).graceful);
    assert_eq!(manager.resource_state().0, 0);
}

struct DelayedPreparationExecutor {
    started: Sender<()>,
    release: Mutex<Receiver<()>>,
}

impl Executor for DelayedPreparationExecutor {
    fn prepare(
        &self,
        _task: &ExecutionTask,
        budget: u64,
        _cancellation: &super::CancellationToken,
    ) -> super::PreparationOutcome {
        self.started.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(TEST_TIMEOUT)
            .unwrap();
        super::PreparationOutcome::Ready(super::PreparedExecution {
            memory_bytes: budget,
            payload: Arc::new(()),
        })
    }

    fn execute(
        &self,
        _task: &ExecutionTask,
        _plan: &super::PreparedExecution,
        _context: &ExecutionContext,
    ) -> ExecutionOutcome {
        panic!("cancelled preparation must not execute")
    }
}

#[test]
fn removed_job_retains_preparation_reservation_until_exit_and_shutdown_waits() {
    let (started, started_receiver) = mpsc::channel();
    let (release, release_receiver) = mpsc::channel();
    let manager = JobManager::with_memory_budget(
        Arc::new(DelayedPreparationExecutor {
            started,
            release: Mutex::new(release_receiver),
        }),
        Arc::new(super::SystemClock),
        1,
        1,
        8 * 1024 * 1024,
    )
    .unwrap();
    let job = manager
        .submit(submission("removed-preparation", 1))
        .unwrap();
    started_receiver.recv_timeout(TEST_TIMEOUT).unwrap();
    manager.cancel_job(&job).unwrap();
    manager.clear_terminal_jobs(None);
    assert!(manager.job_snapshot(&job).is_err());
    assert_eq!(manager.resource_state().0, 1024 * 1024);
    let report = manager.shutdown(Duration::from_millis(10));
    assert!(!report.graceful);
    assert_eq!(report.unfinished_item_ids.len(), 1);
    release.send(()).unwrap();
    assert!(manager.shutdown(TEST_TIMEOUT).graceful);
    assert_eq!(manager.resource_state().0, 0);
}
