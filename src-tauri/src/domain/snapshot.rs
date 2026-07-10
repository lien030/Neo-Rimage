use super::{
    AppError, EncoderKind, EngineWarning, ItemId, ItemProgress, JobId, ProcessingStage, Revision,
    TimestampMs, WorkerSlotId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Cancelling,
    Succeeded,
    PartiallySucceeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::PartiallySucceeded | Self::Failed | Self::Cancelled
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use JobStatus::*;
        matches!(
            (self, next),
            (Queued, Running | Cancelling | Failed | Cancelled)
                | (
                    Running,
                    Paused | Cancelling | Succeeded | PartiallySucceeded | Failed
                )
                | (
                    Paused,
                    Running | Cancelling | Succeeded | PartiallySucceeded | Failed
                )
                | (
                    Cancelling,
                    Succeeded | PartiallySucceeded | Failed | Cancelled
                )
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
}

impl ItemStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use ItemStatus::*;
        matches!(
            (self, next),
            (Queued, Running | Failed | Cancelled | Skipped)
                | (
                    Running,
                    Cancelling | Succeeded | Failed | Cancelled | Skipped
                )
                | (Cancelling, Succeeded | Failed | Cancelled)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerMode {
    Running,
    Paused,
    ShuttingDown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerSlotStatus {
    Idle,
    Busy,
    Draining,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendSnapshot {
    pub schema_version: u16,
    pub revision: Revision,
    pub generated_at: TimestampMs,
    pub scheduler: SchedulerSnapshot,
    pub worker_slots: Vec<WorkerSlotSnapshot>,
    pub jobs: Vec<JobSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerSnapshot {
    pub mode: SchedulerMode,
    pub desired_concurrency: u16,
    pub effective_concurrency: u16,
    pub max_concurrency: u16,
    pub active_items: u32,
    pub queued_items: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerSlotSnapshot {
    pub id: WorkerSlotId,
    pub status: WorkerSlotStatus,
    pub item_id: Option<ItemId>,
    pub stage: Option<ProcessingStage>,
    pub progress: Option<ItemProgress>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub id: JobId,
    pub revision: Revision,
    pub config_version: u16,
    pub encoder: EncoderKind,
    pub status: JobStatus,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
    pub counts: JobCounts,
    pub progress: JobProgressSnapshot,
    pub controls: JobControlAvailability,
    pub result: Option<ResultSummary>,
    pub error: Option<AppError>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobCounts {
    pub total: u32,
    pub queued: u32,
    pub running: u32,
    pub cancelling: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub skipped: u32,
}

impl JobCounts {
    pub fn terminal(&self) -> u32 {
        self.succeeded
            .saturating_add(self.failed)
            .saturating_add(self.cancelled)
            .saturating_add(self.skipped)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressSnapshot {
    pub completed_items: u32,
    pub total_items: u32,
    pub active_items: u32,
}

impl JobProgressSnapshot {
    pub fn ratio(&self) -> Option<f64> {
        (self.total_items > 0)
            .then(|| (self.completed_items as f64 / self.total_items as f64).clamp(0.0, 1.0))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobControlAvailability {
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_cancel: bool,
    pub can_retry: bool,
    pub can_remove: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemControlAvailability {
    pub can_cancel: bool,
    pub can_retry: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSnapshot {
    pub id: ItemId,
    pub job_id: JobId,
    pub revision: Revision,
    pub sequence: u32,
    pub attempt: u32,
    pub input_path: String,
    pub output_path: Option<String>,
    pub status: ItemStatus,
    pub stage: Option<ProcessingStage>,
    pub progress: Option<ItemProgress>,
    pub worker_slot_id: Option<WorkerSlotId>,
    pub created_at: TimestampMs,
    pub started_at: Option<TimestampMs>,
    pub finished_at: Option<TimestampMs>,
    pub controls: ItemControlAvailability,
    pub result: Option<ItemResultSummary>,
    pub error: Option<AppError>,
    #[serde(default)]
    pub warnings: Vec<EngineWarning>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResultSummary {
    pub output_path: String,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultSummary {
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    pub duration_ms: u64,
    pub succeeded: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub skipped: u32,
}

impl ResultSummary {
    pub fn bytes_saved(&self) -> i128 {
        self.total_input_bytes as i128 - self.total_output_bytes as i128
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetailSnapshot {
    pub schema_version: u16,
    pub revision: Revision,
    pub job: JobSnapshot,
    pub items: Page<ItemSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub offset: u32,
    pub limit: u32,
    pub total: u32,
}

#[cfg(test)]
mod tests {
    use super::{ItemStatus, JobStatus};
    use serde_json::json;

    #[test]
    fn public_status_tags_are_stable() {
        assert_eq!(
            serde_json::to_value(JobStatus::PartiallySucceeded).unwrap(),
            json!("partially_succeeded")
        );
        assert_eq!(
            serde_json::to_value(ItemStatus::Cancelling).unwrap(),
            json!("cancelling")
        );
        assert_eq!(
            serde_json::to_value(ItemStatus::Cancelled).unwrap(),
            json!("cancelled")
        );
    }

    #[test]
    fn terminal_states_cannot_transition_back() {
        assert!(JobStatus::Running.can_transition_to(JobStatus::Succeeded));
        assert!(JobStatus::Paused.can_transition_to(JobStatus::Succeeded));
        assert!(JobStatus::Paused.can_transition_to(JobStatus::PartiallySucceeded));
        assert!(!JobStatus::Succeeded.can_transition_to(JobStatus::Running));
        assert!(ItemStatus::Running.can_transition_to(ItemStatus::Cancelling));
        assert!(ItemStatus::Running.can_transition_to(ItemStatus::Skipped));
        assert!(!ItemStatus::Failed.can_transition_to(ItemStatus::Queued));
    }
}
