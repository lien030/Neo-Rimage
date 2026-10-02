use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::domain::{
    AppError, BackendSnapshot, EngineProgressEvent, EngineResult, EngineWarning, ItemId,
    ItemProgress, ItemSpec, ItemStatus, JobCounts, JobDetailSnapshot, JobId, JobSnapshot, JobSpec,
    JobStatus, Page, ProcessingStage, ProgressMeasure, Revision, TimestampMs, WorkerSlotId,
    IPC_SCHEMA_VERSION,
};

use super::{
    CancellationToken, ClearResult, ExecutionContext, ExecutionOutcome, ExecutionTask, Executor,
    JobSubmission, ManagerError, ProgressReporter, RetryMode, RevisionSubscription, ShutdownReport,
};

mod completion;
mod projection;

use completion::{apply_finished_outcome, panic_message};
use projection::{snapshot_item, snapshot_job, snapshot_state};

pub trait Clock: Send + Sync + 'static {
    fn now_ms(&self) -> TimestampMs;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> TimestampMs {
        let value = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        TimestampMs(value)
    }
}

/// The sole in-process authority for jobs, item lifecycle, progress,
/// cancellation, and logical worker slots.
pub struct JobManager {
    shared: Arc<Shared>,
}

impl Clone for JobManager {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

struct Shared {
    executor: Arc<dyn Executor>,
    clock: Arc<dyn Clock>,
    next_id: AtomicU64,
    state: Mutex<State>,
    state_changed: Condvar,
    revision_subscribers: Mutex<Vec<SyncSender<Revision>>>,
}

struct State {
    revision: Revision,
    desired_concurrency: usize,
    maximum_concurrency: usize,
    scheduler_paused: bool,
    shutting_down: bool,
    jobs: HashMap<JobId, JobRecord>,
    job_order: Vec<JobId>,
    runnable_jobs: VecDeque<JobId>,
    slots: Vec<SlotRecord>,
}

struct JobRecord {
    spec: Arc<JobSpec>,
    status: JobStatus,
    updated_at: TimestampMs,
    paused: bool,
    cancel_requested: bool,
    items: Vec<ItemRecord>,
}

struct ItemRecord {
    spec: Arc<ItemSpec>,
    status: ItemStatus,
    attempt: u32,
    stage: Option<ProcessingStage>,
    progress: Option<ItemProgress>,
    warnings: Vec<EngineWarning>,
    slot_id: Option<WorkerSlotId>,
    cancellation: Option<CancellationToken>,
    created_at: TimestampMs,
    started_at: Option<TimestampMs>,
    finished_at: Option<TimestampMs>,
    result: Option<EngineResult>,
    error: Option<AppError>,
}

struct SlotRecord {
    id: WorkerSlotId,
    item_id: Option<ItemId>,
}

struct DispatchWork {
    task: ExecutionTask,
    cancellation: CancellationToken,
}

impl JobManager {
    pub fn new(
        executor: Arc<dyn Executor>,
        maximum_concurrency: usize,
    ) -> Result<Self, ManagerError> {
        Self::with_dependencies(executor, Arc::new(SystemClock), 1, maximum_concurrency)
    }

    pub fn with_concurrency(
        executor: Arc<dyn Executor>,
        desired_concurrency: usize,
        maximum_concurrency: usize,
    ) -> Result<Self, ManagerError> {
        Self::with_dependencies(
            executor,
            Arc::new(SystemClock),
            desired_concurrency,
            maximum_concurrency,
        )
    }

    pub fn with_dependencies(
        executor: Arc<dyn Executor>,
        clock: Arc<dyn Clock>,
        desired_concurrency: usize,
        maximum_concurrency: usize,
    ) -> Result<Self, ManagerError> {
        validate_concurrency(desired_concurrency, maximum_concurrency)?;
        let slots = (0..maximum_concurrency)
            .map(|index| SlotRecord {
                id: WorkerSlotId::new(format!("slot-{:04}", index + 1)),
                item_id: None,
            })
            .collect();

        Ok(Self {
            shared: Arc::new(Shared {
                executor,
                clock,
                next_id: AtomicU64::new(1),
                state: Mutex::new(State {
                    revision: Revision::INITIAL,
                    desired_concurrency,
                    maximum_concurrency,
                    scheduler_paused: false,
                    shutting_down: false,
                    jobs: HashMap::new(),
                    job_order: Vec::new(),
                    runnable_jobs: VecDeque::new(),
                    slots,
                }),
                state_changed: Condvar::new(),
                revision_subscribers: Mutex::new(Vec::new()),
            }),
        })
    }

    /// Store an immutable normalized job and immediately make it dispatchable.
    pub fn submit(&self, mut submission: JobSubmission) -> Result<JobId, ManagerError> {
        if submission.items.is_empty() {
            return Err(ManagerError::EmptyJob);
        }

        let now = self.shared.clock.now_ms();
        if submission.job.id.is_empty() {
            submission.job.id = self.next_job_id();
        }
        if submission.job.created_at.0 == 0 {
            submission.job.created_at = now;
        }
        let job_id = submission.job.id.clone();

        let mut item_ids = HashSet::with_capacity(submission.items.len());
        for (index, item) in submission.items.iter_mut().enumerate() {
            if item.id.is_empty() {
                item.id = ItemId::new(format!("{job_id}-item-{:08}", index + 1));
            }
            if item.job_id.is_empty() {
                item.job_id = job_id.clone();
            }
            if item.job_id != job_id {
                return Err(ManagerError::ItemJobMismatch {
                    item_id: item.id.clone(),
                    job_id: job_id.clone(),
                });
            }
            if !item_ids.insert(item.id.clone()) {
                return Err(ManagerError::DuplicateItemId(item.id.clone()));
            }
            if item.attempt == 0 {
                item.attempt = 1;
            }
        }

        let job_spec = Arc::new(submission.job);
        let items: Vec<ItemRecord> = submission
            .items
            .into_iter()
            .map(|item| {
                let attempt = item.attempt;
                ItemRecord {
                    spec: Arc::new(item),
                    status: ItemStatus::Queued,
                    attempt,
                    stage: None,
                    progress: None,
                    warnings: Vec::new(),
                    slot_id: None,
                    cancellation: None,
                    created_at: now,
                    started_at: None,
                    finished_at: None,
                    result: None,
                    error: None,
                }
            })
            .collect();

        {
            let mut state = lock_state(&self.shared.state);
            if state.shutting_down {
                return Err(ManagerError::ShuttingDown);
            }
            if state.jobs.contains_key(&job_id) {
                return Err(ManagerError::DuplicateJobId(job_id));
            }
            if let Some(duplicate) = items.iter().find(|candidate| {
                state.jobs.values().any(|job| {
                    job.items
                        .iter()
                        .any(|existing| existing.spec.id == candidate.spec.id)
                })
            }) {
                return Err(ManagerError::DuplicateItemId(duplicate.spec.id.clone()));
            }
            state.jobs.insert(
                job_id.clone(),
                JobRecord {
                    spec: job_spec,
                    status: JobStatus::Queued,
                    updated_at: now,
                    paused: false,
                    cancel_requested: false,
                    items,
                },
            );
            state.job_order.push(job_id.clone());
            state.runnable_jobs.push_back(job_id.clone());
            bump_revision(&mut state);
        }

        announce_current_revision(&self.shared);
        dispatch(Arc::clone(&self.shared));
        Ok(job_id)
    }

    pub fn snapshot(&self) -> BackendSnapshot {
        let generated_at = self.shared.clock.now_ms();
        snapshot_state(&lock_state(&self.shared.state), generated_at)
    }

    pub fn observation_revision(&self) -> (Revision, TimestampMs) {
        let state = lock_state(&self.shared.state);
        (state.revision, self.shared.clock.now_ms())
    }

    /// Subscribe to bounded revision notifications. A full channel keeps its
    /// older notification; this intentionally coalesces bursts because the
    /// receiver must fetch an authoritative snapshot after every wake-up.
    pub fn subscribe_revisions(&self) -> RevisionSubscription {
        let (sender, receiver) = mpsc::sync_channel(1);
        lock_state(&self.shared.revision_subscribers).push(sender);
        RevisionSubscription { receiver }
    }

    pub fn job_snapshot(&self, job_id: &JobId) -> Result<JobSnapshot, ManagerError> {
        let state = lock_state(&self.shared.state);
        let job = state
            .jobs
            .get(job_id)
            .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
        Ok(snapshot_job(job, state.revision))
    }

    pub fn job_detail_snapshot(
        &self,
        job_id: &JobId,
        offset: u32,
        limit: u32,
    ) -> Result<JobDetailSnapshot, ManagerError> {
        let state = lock_state(&self.shared.state);
        let job = state
            .jobs
            .get(job_id)
            .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
        let total = u32_len(job.items.len());
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(job.items.len());
        let end = start
            .saturating_add(usize::try_from(limit).unwrap_or(usize::MAX))
            .min(job.items.len());

        Ok(JobDetailSnapshot {
            schema_version: IPC_SCHEMA_VERSION,
            revision: state.revision,
            job: snapshot_job(job, state.revision),
            items: Page {
                items: job.items[start..end]
                    .iter()
                    .map(|item| snapshot_item(item, state.revision))
                    .collect(),
                offset,
                limit,
                total,
            },
        })
    }

    pub fn set_desired_concurrency(&self, desired: usize) -> Result<(), ManagerError> {
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ManagerError::ShuttingDown);
        }
        validate_concurrency(desired, state.maximum_concurrency)?;
        if state.desired_concurrency == desired {
            return Ok(());
        }
        state.desired_concurrency = desired;
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        dispatch(Arc::clone(&self.shared));
        Ok(())
    }

    /// Pause or resume global dispatch without changing the configured
    /// concurrency budget. Already-running items continue to a safe terminal
    /// point; queued items remain queued until resumed.
    pub fn set_scheduler_paused(&self, paused: bool) -> Result<(), ManagerError> {
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ManagerError::ShuttingDown);
        }
        if state.scheduler_paused == paused {
            return Ok(());
        }
        state.scheduler_paused = paused;
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        if !paused {
            dispatch(Arc::clone(&self.shared));
        }
        Ok(())
    }

    pub fn pause_job(&self, job_id: &JobId) -> Result<(), ManagerError> {
        let now = self.shared.clock.now_ms();
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ManagerError::ShuttingDown);
        }
        let job = state
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
        if job.status != JobStatus::Running || !job.status.can_transition_to(JobStatus::Paused) {
            return Err(invalid_action(job, "pause"));
        }
        job.paused = true;
        transition_job(job, JobStatus::Paused);
        job.updated_at = now;
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        Ok(())
    }

    pub fn resume_job(&self, job_id: &JobId) -> Result<(), ManagerError> {
        let now = self.shared.clock.now_ms();
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ManagerError::ShuttingDown);
        }
        let job = state
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
        if job.status != JobStatus::Paused || !job.status.can_transition_to(JobStatus::Running) {
            return Err(invalid_action(job, "resume"));
        }
        job.paused = false;
        transition_job(job, JobStatus::Running);
        job.updated_at = now;
        enqueue_job_once(&mut state.runnable_jobs, job_id);
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        dispatch(Arc::clone(&self.shared));
        Ok(())
    }

    pub fn cancel_job(&self, job_id: &JobId) -> Result<(), ManagerError> {
        let now = self.shared.clock.now_ms();
        let mut state = lock_state(&self.shared.state);
        let job = state
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
        if job.status.is_terminal() {
            return Err(invalid_action(job, "cancel"));
        }
        if job.cancel_requested {
            return Ok(());
        }

        request_job_cancellation(job, now);
        let terminal = job.status.is_terminal();
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        if terminal {
            dispatch(Arc::clone(&self.shared));
        }
        Ok(())
    }

    pub fn cancel_item(&self, item_id: &ItemId) -> Result<(), ManagerError> {
        let now = self.shared.clock.now_ms();
        let mut state = lock_state(&self.shared.state);
        let job_id = job_id_for_item(&state, item_id)
            .ok_or_else(|| ManagerError::ItemNotFound(item_id.clone()))?;
        let job = state
            .jobs
            .get_mut(&job_id)
            .ok_or_else(|| ManagerError::ItemNotFound(item_id.clone()))?;
        let item = job
            .items
            .iter_mut()
            .find(|item| &item.spec.id == item_id)
            .ok_or_else(|| ManagerError::ItemNotFound(item_id.clone()))?;

        match item.status {
            ItemStatus::Queued => {
                transition_item(item, ItemStatus::Cancelled);
                item.finished_at = Some(now);
            }
            ItemStatus::Running => {
                transition_item(item, ItemStatus::Cancelling);
                if let Some(token) = &item.cancellation {
                    token.cancel();
                }
            }
            ItemStatus::Cancelling => return Ok(()),
            ItemStatus::Succeeded
            | ItemStatus::Failed
            | ItemStatus::Cancelled
            | ItemStatus::Skipped => {
                return Err(ManagerError::InvalidItemAction {
                    item_id: item_id.clone(),
                    state: item.status,
                    action: "cancel",
                });
            }
        }

        let next_status = derive_job_status(job);
        if next_status == JobStatus::Cancelled
            && !job.status.can_transition_to(JobStatus::Cancelled)
            && job.status.can_transition_to(JobStatus::Cancelling)
        {
            transition_job(job, JobStatus::Cancelling);
        }
        transition_job(job, next_status);
        job.updated_at = now;
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        dispatch(Arc::clone(&self.shared));
        Ok(())
    }

    pub fn retry_job(&self, job_id: &JobId, mode: RetryMode) -> Result<usize, ManagerError> {
        let now = self.shared.clock.now_ms();
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ManagerError::ShuttingDown);
        }
        let job = state
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
        if !job.status.is_terminal() {
            return Err(invalid_action(job, "retry"));
        }

        let include_cancelled = mode == RetryMode::FailedAndCancelled;
        let mut retried = 0;
        for item in &mut job.items {
            if !item_is_retryable(item, include_cancelled) {
                continue;
            }
            reset_item_for_retry(item);
            retried += 1;
        }
        if retried == 0 {
            return Err(invalid_action(job, "retry"));
        }

        job.cancel_requested = false;
        job.paused = false;
        job.status = if job
            .items
            .iter()
            .any(|item| item.status == ItemStatus::Queued)
        {
            JobStatus::Queued
        } else {
            derive_job_status(job)
        };
        job.updated_at = now;
        enqueue_job_once(&mut state.runnable_jobs, job_id);
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        dispatch(Arc::clone(&self.shared));
        Ok(retried)
    }

    pub fn retry_item(&self, item_id: &ItemId) -> Result<(), ManagerError> {
        let now = self.shared.clock.now_ms();
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ManagerError::ShuttingDown);
        }
        let job_id = job_id_for_item(&state, item_id)
            .ok_or_else(|| ManagerError::ItemNotFound(item_id.clone()))?;
        let job = state
            .jobs
            .get_mut(&job_id)
            .ok_or_else(|| ManagerError::ItemNotFound(item_id.clone()))?;
        if !job.status.is_terminal() {
            return Err(ManagerError::InvalidAction {
                job_id,
                state: job.status,
                action: "retry item in",
            });
        }
        let item = job
            .items
            .iter_mut()
            .find(|item| &item.spec.id == item_id)
            .ok_or_else(|| ManagerError::ItemNotFound(item_id.clone()))?;
        if !item_is_retryable(item, true) {
            return Err(ManagerError::InvalidItemAction {
                item_id: item_id.clone(),
                state: item.status,
                action: "retry",
            });
        }

        reset_item_for_retry(item);
        job.cancel_requested = false;
        job.paused = false;
        job.status = JobStatus::Queued;
        job.updated_at = now;
        enqueue_job_once(&mut state.runnable_jobs, &job_id);
        bump_revision(&mut state);
        drop(state);
        announce_current_revision(&self.shared);
        dispatch(Arc::clone(&self.shared));
        Ok(())
    }

    pub fn clear_terminal_jobs(&self, job_ids: Option<&[JobId]>) -> ClearResult {
        let requested: Option<HashSet<&JobId>> = job_ids.map(|ids| ids.iter().collect());
        let mut state = lock_state(&self.shared.state);
        let cleared = state
            .job_order
            .iter()
            .filter(|job_id| requested.as_ref().is_none_or(|ids| ids.contains(job_id)))
            .filter(|job_id| {
                state
                    .jobs
                    .get(*job_id)
                    .is_some_and(|job| job.status.is_terminal())
            })
            .cloned()
            .collect::<Vec<_>>();

        if !cleared.is_empty() {
            let cleared_set = cleared.iter().collect::<HashSet<_>>();
            state
                .job_order
                .retain(|job_id| !cleared_set.contains(job_id));
            state
                .runnable_jobs
                .retain(|job_id| !cleared_set.contains(job_id));
            for job_id in &cleared {
                state.jobs.remove(job_id);
            }
            bump_revision(&mut state);
            drop(state);
            announce_current_revision(&self.shared);
        }

        ClearResult {
            cleared_job_ids: cleared,
        }
    }

    pub fn wait_for_job_terminal(
        &self,
        job_id: &JobId,
        timeout: Duration,
    ) -> Result<JobSnapshot, ManagerError> {
        let deadline = Instant::now() + timeout;
        let mut state = lock_state(&self.shared.state);
        loop {
            let job = state
                .jobs
                .get(job_id)
                .ok_or_else(|| ManagerError::JobNotFound(job_id.clone()))?;
            if job.status.is_terminal() {
                return Ok(snapshot_job(job, state.revision));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ManagerError::TimedOut);
            }
            let waited = self
                .shared
                .state_changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state = waited.0;
            if waited.1.timed_out()
                && state
                    .jobs
                    .get(job_id)
                    .is_some_and(|job| !job.status.is_terminal())
            {
                return Err(ManagerError::TimedOut);
            }
        }
    }

    pub fn shutdown(&self, timeout: Duration) -> ShutdownReport {
        let now = self.shared.clock.now_ms();
        let deadline = Instant::now() + timeout;
        let mut state = lock_state(&self.shared.state);

        if !state.shutting_down {
            state.shutting_down = true;
            for job in state.jobs.values_mut() {
                if job.status.is_terminal() {
                    continue;
                }
                request_job_cancellation(job, now);
            }
            state.runnable_jobs.clear();
            bump_revision(&mut state);
            notify_revision(&self.shared, state.revision);
        }

        while active_items(&state) > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let waited = self
                .shared
                .state_changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state = waited.0;
            if waited.1.timed_out() && active_items(&state) > 0 {
                break;
            }
        }

        let unfinished_item_ids = state
            .jobs
            .values()
            .flat_map(|job| &job.items)
            .filter(|item| !item.status.is_terminal())
            .map(|item| item.spec.id.clone())
            .collect::<Vec<_>>();
        ShutdownReport {
            graceful: unfinished_item_ids.is_empty(),
            unfinished_item_ids,
        }
    }

    fn next_job_id(&self) -> JobId {
        let sequence = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        JobId::new(format!("job-{sequence:016x}"))
    }
}

fn validate_concurrency(desired: usize, maximum: usize) -> Result<(), ManagerError> {
    if maximum == 0 || desired == 0 || desired > maximum || maximum > usize::from(u16::MAX) {
        return Err(ManagerError::ConcurrencyOutOfRange {
            requested: desired,
            minimum: 1,
            maximum,
        });
    }
    Ok(())
}

fn invalid_action(job: &JobRecord, action: &'static str) -> ManagerError {
    ManagerError::InvalidAction {
        job_id: job.spec.id.clone(),
        state: job.status,
        action,
    }
}

fn lock_state<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn bump_revision(state: &mut State) {
    state.revision = Revision(state.revision.0.saturating_add(1));
}

/// Wake observers after a mutation whose state lock has already been released.
///
/// Re-reading the revision is intentional: a concurrent mutation may move the
/// notification forward, but it can never make this snapshot-dirty hint stale.
/// Revision subscriptions are coalesced wake-ups rather than an event log.
fn announce_current_revision(shared: &Shared) {
    let revision = lock_state(&shared.state).revision;
    notify_revision(shared, revision);
}

fn notify_revision(shared: &Shared, revision: Revision) {
    shared.state_changed.notify_all();
    lock_state(&shared.revision_subscribers).retain(|subscriber| {
        match subscriber.try_send(revision) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        }
    });
}

fn enqueue_job_once(queue: &mut VecDeque<JobId>, job_id: &JobId) {
    if !queue.iter().any(|queued| queued == job_id) {
        queue.push_back(job_id.clone());
    }
}

fn transition_item(item: &mut ItemRecord, next: ItemStatus) {
    debug_assert!(item.status.can_transition_to(next));
    if item.status.can_transition_to(next) {
        item.status = next;
    }
}

fn transition_job(job: &mut JobRecord, next: JobStatus) {
    if job.status == next {
        return;
    }
    debug_assert!(job.status.can_transition_to(next));
    if job.status.can_transition_to(next) {
        job.status = next;
    }
}

fn job_id_for_item(state: &State, item_id: &ItemId) -> Option<JobId> {
    state.jobs.iter().find_map(|(job_id, job)| {
        job.items
            .iter()
            .any(|item| &item.spec.id == item_id)
            .then(|| job_id.clone())
    })
}

/// Cancel queued work immediately and signal running work cooperatively.
///
/// Running items retain their worker slot until the executor reaches a safe
/// cancellation boundary and reports completion. This invariant keeps the
/// configured concurrency budget accurate during cancellation and shutdown.
fn request_job_cancellation(job: &mut JobRecord, now: TimestampMs) {
    debug_assert!(!job.status.is_terminal());
    if matches!(job.status, JobStatus::Running | JobStatus::Paused) {
        transition_job(job, JobStatus::Cancelling);
    }
    job.cancel_requested = true;
    job.paused = false;

    for item in &mut job.items {
        match item.status {
            ItemStatus::Queued => {
                transition_item(item, ItemStatus::Cancelled);
                item.finished_at = Some(now);
            }
            ItemStatus::Running => {
                transition_item(item, ItemStatus::Cancelling);
                if let Some(token) = &item.cancellation {
                    token.cancel();
                }
            }
            ItemStatus::Cancelling
            | ItemStatus::Succeeded
            | ItemStatus::Failed
            | ItemStatus::Cancelled
            | ItemStatus::Skipped => {}
        }
    }

    let next_status = derive_job_status(job);
    transition_job(job, next_status);
    job.updated_at = now;
}

fn reset_item_for_retry(item: &mut ItemRecord) {
    item.attempt = item.attempt.saturating_add(1);
    item.status = ItemStatus::Queued;
    item.stage = None;
    item.progress = None;
    item.warnings.clear();
    item.slot_id = None;
    item.cancellation = None;
    item.started_at = None;
    item.finished_at = None;
    item.result = None;
    item.error = None;
}

fn item_is_retryable(item: &ItemRecord, include_cancelled: bool) -> bool {
    (include_cancelled && item.status == ItemStatus::Cancelled)
        || (item.status == ItemStatus::Failed
            && item.error.as_ref().is_some_and(|error| error.retryable))
}

fn active_items(state: &State) -> usize {
    state
        .jobs
        .values()
        .flat_map(|job| &job.items)
        .filter(|item| matches!(item.status, ItemStatus::Running | ItemStatus::Cancelling))
        .count()
}

fn queued_items(state: &State) -> usize {
    state
        .jobs
        .values()
        .flat_map(|job| &job.items)
        .filter(|item| item.status == ItemStatus::Queued)
        .count()
}

fn derive_job_status(job: &JobRecord) -> JobStatus {
    let counts = count_items(&job.items);
    if job.cancel_requested && counts.running + counts.cancelling + counts.queued > 0 {
        return JobStatus::Cancelling;
    }
    if job.paused && counts.queued > 0 {
        return JobStatus::Paused;
    }
    if counts.running + counts.cancelling > 0 {
        return JobStatus::Running;
    }
    if counts.queued > 0 {
        return if job.status == JobStatus::Queued
            && job.items.iter().all(|item| item.started_at.is_none())
        {
            JobStatus::Queued
        } else {
            JobStatus::Running
        };
    }
    if counts.succeeded + counts.skipped == counts.total {
        return JobStatus::Succeeded;
    }
    if counts.cancelled == counts.total {
        return JobStatus::Cancelled;
    }
    if counts.succeeded + counts.skipped > 0 {
        return JobStatus::PartiallySucceeded;
    }
    JobStatus::Failed
}

fn count_items(items: &[ItemRecord]) -> JobCounts {
    let mut counts = JobCounts {
        total: u32_len(items.len()),
        ..JobCounts::default()
    };
    for item in items {
        match item.status {
            ItemStatus::Queued => counts.queued = counts.queued.saturating_add(1),
            ItemStatus::Running => counts.running = counts.running.saturating_add(1),
            ItemStatus::Cancelling => counts.cancelling = counts.cancelling.saturating_add(1),
            ItemStatus::Succeeded => counts.succeeded = counts.succeeded.saturating_add(1),
            ItemStatus::Failed => counts.failed = counts.failed.saturating_add(1),
            ItemStatus::Cancelled => counts.cancelled = counts.cancelled.saturating_add(1),
            ItemStatus::Skipped => counts.skipped = counts.skipped.saturating_add(1),
        }
    }
    counts
}

fn u16_len(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn u32_len(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn dispatch(shared: Arc<Shared>) {
    loop {
        let now = shared.clock.now_ms();
        let work = {
            let mut state = lock_state(&shared.state);
            prepare_next_work(&mut state, now)
        };
        let Some(work) = work else {
            return;
        };

        announce_current_revision(&shared);
        let item_id = work.task.item.id.clone();
        let thread_name = format!("neo-rimage-{item_id}");
        let worker_shared = Arc::clone(&shared);
        let spawn_result = thread::Builder::new().name(thread_name).spawn(move || {
            execute_work(worker_shared, work);
        });
        if let Err(error) = spawn_result {
            finish_work(
                Arc::clone(&shared),
                item_id,
                FinishedOutcome::Unavailable(error.to_string()),
                false,
            );
        }
    }
}

fn prepare_next_work(state: &mut State, now: TimestampMs) -> Option<DispatchWork> {
    if state.shutting_down
        || state.scheduler_paused
        || active_items(state) >= state.desired_concurrency
    {
        return None;
    }
    let slot_index = state
        .slots
        .iter()
        .take(state.desired_concurrency)
        .position(|slot| slot.item_id.is_none())?;

    let mut selected = None;
    let candidates = state.runnable_jobs.len();
    for _ in 0..candidates {
        let job_id = state.runnable_jobs.pop_front()?;
        let Some(job) = state.jobs.get(&job_id) else {
            continue;
        };
        if job.paused || job.cancel_requested || job.status.is_terminal() {
            if job.paused {
                state.runnable_jobs.push_back(job_id);
            }
            continue;
        }
        if let Some(item_index) = job
            .items
            .iter()
            .position(|item| item.status == ItemStatus::Queued)
        {
            if job
                .items
                .iter()
                .skip(item_index + 1)
                .any(|item| item.status == ItemStatus::Queued)
            {
                state.runnable_jobs.push_back(job_id.clone());
            }
            selected = Some((job_id, item_index));
            break;
        }
    }

    let (job_id, item_index) = selected?;
    let slot_id = state.slots[slot_index].id.clone();
    let cancellation = CancellationToken::new();
    let job = state.jobs.get_mut(&job_id)?;
    let (item_id, item_spec, attempt) = {
        let item = job.items.get_mut(item_index)?;
        transition_item(item, ItemStatus::Running);
        item.started_at = Some(now);
        item.finished_at = None;
        item.slot_id = Some(slot_id);
        item.cancellation = Some(cancellation.clone());
        item.stage = None;
        item.progress = None;
        item.warnings.clear();
        item.result = None;
        item.error = None;
        (item.spec.id.clone(), Arc::clone(&item.spec), item.attempt)
    };
    transition_job(job, JobStatus::Running);
    job.updated_at = now;
    state.slots[slot_index].item_id = Some(item_id);
    let task = ExecutionTask {
        job: Arc::clone(&job.spec),
        item: item_spec,
        attempt,
    };
    bump_revision(state);
    Some(DispatchWork { task, cancellation })
}

// This value is produced and consumed once on the same worker path. Keeping the
// domain outcome inline avoids a heap allocation for every completed image.
#[allow(clippy::large_enum_variant)]
enum FinishedOutcome {
    Engine(ExecutionOutcome),
    Panicked(String),
    Unavailable(String),
}

fn execute_work(shared: Arc<Shared>, work: DispatchWork) {
    let item_id = work.task.item.id.clone();
    let progress_shared = Arc::clone(&shared);
    let progress_item_id = item_id.clone();
    let reporter = ProgressReporter::new(move |event| {
        record_progress(&progress_shared, &progress_item_id, event);
    });
    let context = ExecutionContext {
        cancellation: work.cancellation,
        progress: reporter,
    };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        shared.executor.execute(&work.task, &context)
    }))
    .map(FinishedOutcome::Engine)
    .unwrap_or_else(|payload| FinishedOutcome::Panicked(panic_message(payload)));
    finish_work(shared, item_id, outcome, true);
}

fn record_progress(shared: &Shared, item_id: &ItemId, event: EngineProgressEvent) {
    let now = shared.clock.now_ms();
    // Fractional progress can arrive much faster than commands and snapshots.
    // It is deliberately lossy under contention; stage boundaries and warnings
    // still take the blocking path, so meaningful lifecycle updates are kept.
    let mut state = if matches!(&event, EngineProgressEvent::StageProgress { .. }) {
        match shared.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
        }
    } else {
        lock_state(&shared.state)
    };
    let Some(job_id) = job_id_for_item(&state, item_id) else {
        return;
    };
    let Some(job) = state.jobs.get_mut(&job_id) else {
        return;
    };
    let Some(item) = job.items.iter_mut().find(|item| &item.spec.id == item_id) else {
        return;
    };
    if !matches!(item.status, ItemStatus::Running | ItemStatus::Cancelling) {
        return;
    }

    let changed = match event {
        EngineProgressEvent::StageStarted { stage } => {
            let progress = ItemProgress {
                stage,
                measure: ProgressMeasure::Indeterminate,
            };
            if item.stage == Some(stage) && item.progress.as_ref() == Some(&progress) {
                false
            } else {
                item.stage = Some(stage);
                item.progress = Some(progress);
                true
            }
        }
        EngineProgressEvent::StageProgress { progress } => {
            if item.progress.as_ref() == Some(&progress) {
                false
            } else {
                item.stage = Some(progress.stage);
                item.progress = Some(progress);
                true
            }
        }
        EngineProgressEvent::WarningRaised { warning } => {
            item.warnings.push(warning);
            true
        }
        EngineProgressEvent::StageCompleted { stage, .. } => {
            if item.stage == Some(stage) {
                false
            } else {
                item.stage = Some(stage);
                true
            }
        }
    };
    if !changed {
        return;
    }
    job.updated_at = now;
    bump_revision(&mut state);
    let revision = state.revision;
    drop(state);
    notify_revision(shared, revision);
}

fn finish_work(shared: Arc<Shared>, item_id: ItemId, outcome: FinishedOutcome, reschedule: bool) {
    let now = shared.clock.now_ms();
    {
        let mut state = lock_state(&shared.state);
        let Some(job_id) = job_id_for_item(&state, &item_id) else {
            return;
        };

        let slot_id = {
            let Some(job) = state.jobs.get_mut(&job_id) else {
                return;
            };
            let cancel_requested = job.cancel_requested;
            let Some(item) = job.items.iter_mut().find(|item| item.spec.id == item_id) else {
                return;
            };
            if !matches!(item.status, ItemStatus::Running | ItemStatus::Cancelling) {
                return;
            }
            let slot_id = item.slot_id.take();
            item.cancellation = None;
            item.finished_at = Some(now);
            apply_finished_outcome(item, outcome, cancel_requested, &job_id, &item_id);
            let next_status = derive_job_status(job);
            transition_job(job, next_status);
            job.updated_at = now;
            slot_id
        };

        if let Some(slot_id) = slot_id {
            if let Some(slot) = state.slots.iter_mut().find(|slot| slot.id == slot_id) {
                slot.item_id = None;
            }
        }
        bump_revision(&mut state);
    }

    announce_current_revision(&shared);
    if reschedule {
        dispatch(shared);
    }
}
