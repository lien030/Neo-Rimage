//! Read-only projection of scheduler records into stable domain snapshots.
//!
//! Keeping projection here leaves `manager.rs` focused on state mutation and
//! dispatch. Every public snapshot is still derived while the manager holds a
//! single state lock, so its revision and all nested records are consistent.

use std::path::Path;

use crate::domain::{
    BackendSnapshot, ItemControlAvailability, ItemResultSummary, ItemSnapshot, ItemStatus,
    JobControlAvailability, JobProgressSnapshot, JobSnapshot, JobStatus, ResultSummary, Revision,
    SchedulerMode, SchedulerSnapshot, TimestampMs, WorkerSlotSnapshot, WorkerSlotStatus,
    IPC_SCHEMA_VERSION,
};

use super::{
    active_items, count_items, item_is_retryable, queued_items, u16_len, u32_len, ItemRecord,
    JobRecord, SlotRecord, State,
};

pub(super) fn snapshot_state(state: &State, generated_at: TimestampMs) -> BackendSnapshot {
    let jobs = state
        .job_order
        .iter()
        .filter_map(|job_id| state.jobs.get(job_id))
        .map(|job| snapshot_job(job, state.revision))
        .collect();
    let worker_slots = state
        .slots
        .iter()
        .enumerate()
        .filter(|(index, slot)| *index < state.desired_concurrency || slot.item_id.is_some())
        .map(|(index, slot)| snapshot_slot(state, index, slot))
        .collect();

    BackendSnapshot {
        schema_version: IPC_SCHEMA_VERSION,
        revision: state.revision,
        generated_at,
        scheduler: SchedulerSnapshot {
            mode: if state.shutting_down {
                SchedulerMode::ShuttingDown
            } else if state.scheduler_paused {
                SchedulerMode::Paused
            } else {
                SchedulerMode::Running
            },
            desired_concurrency: u16_len(state.desired_concurrency),
            effective_concurrency: if state.shutting_down {
                0
            } else {
                u16_len(state.desired_concurrency)
            },
            max_concurrency: u16_len(state.maximum_concurrency),
            active_items: u32_len(active_items(state)),
            queued_items: u32_len(queued_items(state)),
        },
        worker_slots,
        jobs,
    }
}

fn snapshot_slot(state: &State, index: usize, slot: &SlotRecord) -> WorkerSlotSnapshot {
    let item = slot.item_id.as_ref().and_then(|item_id| {
        state
            .jobs
            .values()
            .flat_map(|job| &job.items)
            .find(|item| &item.spec.id == item_id)
    });
    WorkerSlotSnapshot {
        id: slot.id.clone(),
        status: match (&slot.item_id, index < state.desired_concurrency) {
            (Some(_), false) => WorkerSlotStatus::Draining,
            (Some(_), true) => WorkerSlotStatus::Busy,
            (None, _) => WorkerSlotStatus::Idle,
        },
        item_id: slot.item_id.clone(),
        input_path: item.map(|item| path_text(&item.spec.input_path)),
        stage: item.and_then(|item| item.stage),
        progress: item.and_then(|item| item.progress.clone()),
    }
}

pub(super) fn snapshot_job(job: &JobRecord, revision: Revision) -> JobSnapshot {
    let counts = count_items(&job.items);
    JobSnapshot {
        id: job.spec.id.clone(),
        revision,
        config_version: job.spec.config_version,
        encoder: job.spec.encoder.kind(),
        status: job.status,
        created_at: job.spec.created_at,
        updated_at: job.updated_at,
        progress: JobProgressSnapshot {
            completed_items: counts.terminal(),
            total_items: counts.total,
            active_items: counts.running.saturating_add(counts.cancelling),
        },
        controls: job_controls(job),
        result: job.status.is_terminal().then(|| aggregate_result(job)),
        error: None,
        counts,
    }
}

pub(super) fn snapshot_item(item: &ItemRecord, revision: Revision) -> ItemSnapshot {
    ItemSnapshot {
        id: item.spec.id.clone(),
        job_id: item.spec.job_id.clone(),
        revision,
        sequence: item.spec.sequence,
        attempt: item.attempt,
        input_path: path_text(&item.spec.input_path),
        output_path: Some(path_text(&item.spec.output.output_path)),
        status: item.status,
        stage: item.stage,
        progress: item.progress.clone(),
        worker_slot_id: item.slot_id.clone(),
        created_at: item.created_at,
        started_at: item.started_at,
        finished_at: item.finished_at,
        controls: ItemControlAvailability {
            can_cancel: matches!(item.status, ItemStatus::Queued | ItemStatus::Running),
            can_retry: item_is_retryable(item, true),
        },
        result: item.result.as_ref().map(|result| ItemResultSummary {
            output_path: path_text(&result.output_path),
            input_bytes: result.input_bytes,
            output_bytes: result.output_bytes,
            duration_ms: result.duration_ms,
        }),
        error: item.error.clone(),
        warnings: item.warnings.clone(),
    }
}

fn job_controls(job: &JobRecord) -> JobControlAvailability {
    let can_retry = job.items.iter().any(|item| item_is_retryable(item, true));
    JobControlAvailability {
        can_pause: job.status == JobStatus::Running,
        can_resume: job.status == JobStatus::Paused,
        can_cancel: !job.status.is_terminal() && job.status != JobStatus::Cancelling,
        can_retry: job.status.is_terminal() && can_retry,
        can_remove: job.status.is_terminal(),
    }
}

fn aggregate_result(job: &JobRecord) -> ResultSummary {
    let counts = count_items(&job.items);
    let mut result = ResultSummary {
        succeeded: counts.succeeded,
        failed: counts.failed,
        cancelled: counts.cancelled,
        skipped: counts.skipped,
        ..ResultSummary::default()
    };
    for item in &job.items {
        if let Some(item_result) = &item.result {
            result.total_input_bytes = result
                .total_input_bytes
                .saturating_add(item_result.input_bytes);
            result.total_output_bytes = result
                .total_output_bytes
                .saturating_add(item_result.output_bytes);
        }
    }
    result.duration_ms = job.updated_at.0.saturating_sub(job.spec.created_at.0);
    result
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
