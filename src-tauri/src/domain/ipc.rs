use super::{
    AppError, BackendCapabilities, BackendSnapshot, CorrelationId, CreateJobRequest, ItemId,
    ItemProgress, ItemSnapshot, JobDetailSnapshot, JobId, JobSnapshot, Revision, SchedulerSnapshot,
    TimestampMs, WorkerSlotSnapshot, IPC_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

pub const STATE_EVENT_NAME: &str = "backend://state-delta";
pub const NOTICE_EVENT_NAME: &str = "backend://notice";

pub mod command_names {
    pub const GET_BACKEND_CAPABILITIES: &str = "get_backend_capabilities";
    pub const GET_BACKEND_SNAPSHOT: &str = "get_backend_snapshot";
    pub const GET_JOB_SNAPSHOT: &str = "get_job_snapshot";
    pub const CREATE_JOB: &str = "create_job";
    pub const PAUSE_JOB: &str = "pause_job";
    pub const RESUME_JOB: &str = "resume_job";
    pub const CANCEL_JOB: &str = "cancel_job";
    pub const RETRY_JOB_ITEMS: &str = "retry_job_items";
    pub const REMOVE_JOB: &str = "remove_job";
    pub const SET_SCHEDULER_PAUSED: &str = "set_scheduler_paused";
    pub const SET_WORKER_COUNT: &str = "set_worker_count";
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobCommand {
    pub correlation_id: CorrelationId,
    pub request: CreateJobRequest,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobResponse {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub revision: Revision,
    pub job: JobSnapshot,
    pub items: Vec<ItemSnapshot>,
    #[serde(default)]
    pub rejected_inputs: Vec<RejectedInput>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedInput {
    pub input_index: u32,
    pub path: String,
    pub error: AppError,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobCommand {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub job_id: JobId,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryItemsCommand {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub job_id: JobId,
    pub item_ids: Vec<ItemId>,
    #[serde(default)]
    pub include_cancelled: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSchedulerPausedCommand {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub paused: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWorkerCountCommand {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub desired_concurrency: u16,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAccepted<T> {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub revision: Revision,
    pub snapshot: T,
}

pub type JobCommandResponse = CommandAccepted<JobSnapshot>;
pub type SchedulerCommandResponse = CommandAccepted<SchedulerSnapshot>;
pub type WorkerCountCommandResponse = CommandAccepted<Vec<WorkerSlotSnapshot>>;

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateEventEnvelope {
    pub schema_version: u16,
    pub revision: Revision,
    pub occurred_at: TimestampMs,
    pub correlation_id: Option<CorrelationId>,
    pub event: StateEvent,
}

impl StateEventEnvelope {
    pub fn new(revision: Revision, occurred_at: TimestampMs, event: StateEvent) -> Self {
        Self {
            schema_version: IPC_SCHEMA_VERSION,
            revision,
            occurred_at,
            correlation_id: None,
            event,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum StateEvent {
    SnapshotInvalidated,
    SchedulerChanged(SchedulerSnapshot),
    WorkerSlotsChanged(Vec<WorkerSlotSnapshot>),
    JobChanged(JobSnapshot),
    ItemChanged(ItemSnapshot),
    ProgressChanged {
        job_id: JobId,
        item_id: ItemId,
        progress: ItemProgress,
    },
    BackendShuttingDown {
        grace_period_ms: u64,
    },
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeEventEnvelope {
    pub schema_version: u16,
    pub occurred_at: TimestampMs,
    pub notice: BackendNotice,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum BackendNotice {
    CapabilityDegraded { reason_code: String },
    TemporaryFilesCleaned { removed: u32, failed: u32 },
    EventBridgeReconnected,
    ShutdownTimedOut { item_ids: Vec<ItemId> },
}

/// Compile-time grouping for the formal v1 query payloads.
#[allow(dead_code)]
fn _query_contracts(
    _capabilities: BackendCapabilities,
    _backend: BackendSnapshot,
    _job: JobDetailSnapshot,
) {
}

#[cfg(test)]
mod tests {
    use super::{StateEvent, StateEventEnvelope};
    use crate::domain::{
        ItemId, ItemProgress, JobId, ProcessingStage, ProgressMeasure, Revision, TimestampMs,
    };
    use serde_json::json;

    #[test]
    fn event_envelope_has_version_revision_and_string_kind() {
        let event = StateEventEnvelope::new(
            Revision(9),
            TimestampMs(100),
            StateEvent::ProgressChanged {
                job_id: JobId::from("job-1"),
                item_id: ItemId::from("item-1"),
                progress: ItemProgress {
                    stage: ProcessingStage::Encode,
                    measure: ProgressMeasure::Indeterminate,
                },
            },
        );

        let value = serde_json::to_value(event).expect("serialize event");
        assert_eq!(value["schemaVersion"], json!(1));
        assert_eq!(value["revision"], json!(9));
        assert_eq!(value["event"]["kind"], json!("progress_changed"));
    }
}
