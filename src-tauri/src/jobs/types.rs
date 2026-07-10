use std::fmt;
use std::sync::mpsc::{Receiver, RecvError, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use crate::domain::{ItemId, ItemSpec, ItemStatus, JobId, JobSpec, JobStatus, Revision};

/// A normalized, immutable batch accepted by the manager.
///
/// The adapter/validation layer may pre-allocate IDs. If an ID is empty, the
/// manager fills it before storing the job so execution never observes a
/// mutable or incomplete identity.
#[derive(Clone, Debug, PartialEq)]
pub struct JobSubmission {
    pub job: JobSpec,
    pub items: Vec<ItemSpec>,
}

impl JobSubmission {
    pub fn new(job: JobSpec, items: Vec<ItemSpec>) -> Self {
        Self { job, items }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryMode {
    FailedOnly,
    FailedAndCancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearResult {
    pub cleared_job_ids: Vec<JobId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShutdownReport {
    pub graceful: bool,
    pub unfinished_item_ids: Vec<ItemId>,
}

/// Bounded dirty-notification stream for async adapters.
///
/// Notifications may be coalesced. Consumers must fetch a full snapshot and
/// compare its revision instead of treating this receiver as an event log.
pub struct RevisionSubscription {
    pub(crate) receiver: Receiver<Revision>,
}

impl RevisionSubscription {
    pub fn recv(&self) -> Result<Revision, RecvError> {
        self.receiver.recv()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Revision, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    pub fn try_recv(&self) -> Result<Revision, TryRecvError> {
        self.receiver.try_recv()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagerError {
    EmptyJob,
    JobNotFound(JobId),
    ItemNotFound(ItemId),
    DuplicateJobId(JobId),
    DuplicateItemId(ItemId),
    ItemJobMismatch {
        item_id: ItemId,
        job_id: JobId,
    },
    InvalidAction {
        job_id: JobId,
        state: JobStatus,
        action: &'static str,
    },
    InvalidItemAction {
        item_id: ItemId,
        state: ItemStatus,
        action: &'static str,
    },
    ConcurrencyOutOfRange {
        requested: usize,
        minimum: usize,
        maximum: usize,
    },
    ShuttingDown,
    TimedOut,
}

impl fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyJob => formatter.write_str("a job must contain at least one item"),
            Self::JobNotFound(job_id) => write!(formatter, "job {job_id} was not found"),
            Self::ItemNotFound(item_id) => write!(formatter, "item {item_id} was not found"),
            Self::DuplicateJobId(job_id) => write!(formatter, "job {job_id} already exists"),
            Self::DuplicateItemId(item_id) => write!(formatter, "item {item_id} is duplicated"),
            Self::ItemJobMismatch { item_id, job_id } => {
                write!(formatter, "item {item_id} does not belong to job {job_id}")
            }
            Self::InvalidAction {
                job_id,
                state,
                action,
            } => write!(
                formatter,
                "cannot {action} job {job_id} while it is {state:?}"
            ),
            Self::InvalidItemAction {
                item_id,
                state,
                action,
            } => write!(
                formatter,
                "cannot {action} item {item_id} while it is {state:?}"
            ),
            Self::ConcurrencyOutOfRange {
                requested,
                minimum,
                maximum,
            } => write!(
                formatter,
                "concurrency {requested} is outside the supported range {minimum}..={maximum}"
            ),
            Self::ShuttingDown => formatter.write_str("the job manager is shutting down"),
            Self::TimedOut => formatter.write_str("the operation timed out"),
        }
    }
}

impl std::error::Error for ManagerError {}
