use std::thread;

use tauri::{AppHandle, Emitter, State};

use crate::{
    backend::{command_error, manager_command_error, BackendService},
    domain::{
        AppError, BackendCapabilities, BackendSnapshot, CommandAccepted, CommandErrorEnvelope,
        CorrelationId, CreateJobCommand, CreateJobResponse, ErrorCategory, ItemId, JobCommand,
        JobCommandResponse, JobDetailSnapshot, JobId, RetryItemsCommand, ScanInputsCommand,
        ScanInputsResponse, SchedulerCommandResponse, SetSchedulerPausedCommand,
        SetWorkerCountCommand, StateEvent, StateEventEnvelope, WorkerCountCommandResponse,
        IPC_SCHEMA_VERSION, STATE_EVENT_NAME,
    },
    jobs::{ManagerError, RetryMode},
};

pub fn spawn_revision_bridge(app: AppHandle, service: BackendService) {
    let subscription = service.manager().subscribe_revisions();
    thread::Builder::new()
        .name("neo-rimage-event-bridge".to_owned())
        .spawn(move || {
            while subscription.recv().is_ok() {
                let (revision, occurred_at) = service.manager().observation_revision();
                let envelope =
                    StateEventEnvelope::new(revision, occurred_at, StateEvent::SnapshotInvalidated);
                // Events are bounded dirty hints. A webview that misses one can
                // always recover from get_backend_snapshot.
                let _ = app.emit(STATE_EVENT_NAME, envelope);
            }
        })
        .expect("failed to start the backend event bridge");
}

#[tauri::command]
pub fn get_backend_capabilities(service: State<'_, BackendService>) -> BackendCapabilities {
    service.capabilities()
}

#[tauri::command]
pub fn get_backend_snapshot(service: State<'_, BackendService>) -> BackendSnapshot {
    service.manager().snapshot()
}

#[tauri::command]
pub fn get_job_snapshot(
    service: State<'_, BackendService>,
    job_id: String,
) -> Result<JobDetailSnapshot, CommandErrorEnvelope> {
    let job_id = JobId::new(job_id);
    let correlation_id = CorrelationId::new(format!("get-job-{job_id}"));
    service
        .manager()
        .job_detail_snapshot(&job_id, 0, u32::MAX)
        .map_err(|error| manager_command_error(correlation_id, error))
}

#[tauri::command]
pub async fn scan_inputs(
    service: State<'_, BackendService>,
    command: ScanInputsCommand,
) -> Result<ScanInputsResponse, CommandErrorEnvelope> {
    let service = service.inner().clone();
    let correlation_id = command.correlation_id.clone();
    tauri::async_runtime::spawn_blocking(move || service.scan_inputs(command))
        .await
        .map_err(|_| {
            command_error(
                correlation_id,
                AppError::new(
                    "backend.scan_join_failed",
                    ErrorCategory::Internal,
                    "errors.backendScanFailed",
                    "The backend could not finish scanning the selected inputs.",
                ),
            )
        })?
}

#[tauri::command]
pub async fn create_job(
    service: State<'_, BackendService>,
    command: CreateJobCommand,
) -> Result<CreateJobResponse, CommandErrorEnvelope> {
    let service = service.inner().clone();
    let correlation_id = command.correlation_id.clone();
    // Input discovery and canonicalization are blocking filesystem work. Keep
    // them off Tauri's async command executor even though submission itself is
    // an in-memory manager operation.
    tauri::async_runtime::spawn_blocking(move || service.create_job(command))
        .await
        .map_err(|_| {
            command_error(
                correlation_id,
                AppError::new(
                    "backend.create_join_failed",
                    ErrorCategory::Internal,
                    "errors.backendCreateFailed",
                    "The backend could not finish accepting the job.",
                ),
            )
        })?
}

#[tauri::command]
pub fn pause_job(
    service: State<'_, BackendService>,
    command: JobCommand,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    run_job_command(&service, command, |service, job_id| {
        service.manager().pause_job(job_id)
    })
}

#[tauri::command]
pub fn resume_job(
    service: State<'_, BackendService>,
    command: JobCommand,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    run_job_command(&service, command, |service, job_id| {
        service.manager().resume_job(job_id)
    })
}

#[tauri::command]
pub fn cancel_job(
    service: State<'_, BackendService>,
    command: JobCommand,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    run_job_command(&service, command, |service, job_id| {
        service.manager().cancel_job(job_id)
    })
}

#[tauri::command]
pub fn retry_job_items(
    service: State<'_, BackendService>,
    command: RetryItemsCommand,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    ensure_schema(command.schema_version, &command.correlation_id)?;
    let job_id = command.job_id.clone();
    retry_selection(
        &service,
        &command.correlation_id,
        &job_id,
        &command.item_ids,
        command.include_cancelled,
    )?;
    accepted_job(&service, command.correlation_id, &job_id)
}

#[tauri::command]
pub fn remove_job(
    service: State<'_, BackendService>,
    command: JobCommand,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    ensure_schema(command.schema_version, &command.correlation_id)?;
    // Capture the final job view before deletion. The response envelope uses
    // the later clearing revision, while its payload describes what was
    // removed and can no longer be queried from the manager.
    let snapshot = service
        .manager()
        .job_snapshot(&command.job_id)
        .map_err(|error| manager_command_error(command.correlation_id.clone(), error))?;
    let cleared = service
        .manager()
        .clear_terminal_jobs(Some(std::slice::from_ref(&command.job_id)));
    if cleared.cleared_job_ids.is_empty() {
        return Err(command_error(
            command.correlation_id,
            AppError::new(
                "job.remove_not_allowed",
                ErrorCategory::Validation,
                "errors.jobRemoveNotAllowed",
                "Only terminal jobs can be removed.",
            ),
        ));
    }
    let backend = service.manager().snapshot();
    Ok(CommandAccepted {
        schema_version: IPC_SCHEMA_VERSION,
        correlation_id: command.correlation_id,
        revision: backend.revision,
        snapshot,
    })
}

#[tauri::command]
pub fn set_scheduler_paused(
    service: State<'_, BackendService>,
    command: SetSchedulerPausedCommand,
) -> Result<SchedulerCommandResponse, CommandErrorEnvelope> {
    ensure_schema(command.schema_version, &command.correlation_id)?;
    service
        .manager()
        .set_scheduler_paused(command.paused)
        .map_err(|error| manager_command_error(command.correlation_id.clone(), error))?;
    let backend = service.manager().snapshot();
    Ok(CommandAccepted {
        schema_version: IPC_SCHEMA_VERSION,
        correlation_id: command.correlation_id,
        revision: backend.revision,
        snapshot: backend.scheduler,
    })
}

#[tauri::command]
pub fn set_worker_count(
    service: State<'_, BackendService>,
    command: SetWorkerCountCommand,
) -> Result<WorkerCountCommandResponse, CommandErrorEnvelope> {
    ensure_schema(command.schema_version, &command.correlation_id)?;
    service
        .manager()
        .set_desired_concurrency(usize::from(command.desired_concurrency))
        .map_err(|error| manager_command_error(command.correlation_id.clone(), error))?;
    let backend = service.manager().snapshot();
    Ok(CommandAccepted {
        schema_version: IPC_SCHEMA_VERSION,
        correlation_id: command.correlation_id,
        revision: backend.revision,
        snapshot: backend.worker_slots,
    })
}

fn run_job_command(
    service: &BackendService,
    command: JobCommand,
    action: impl FnOnce(&BackendService, &JobId) -> Result<(), ManagerError>,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    ensure_schema(command.schema_version, &command.correlation_id)?;
    action(service, &command.job_id)
        .map_err(|error| manager_command_error(command.correlation_id.clone(), error))?;
    accepted_job(service, command.correlation_id, &command.job_id)
}

fn retry_selection(
    service: &BackendService,
    correlation_id: &CorrelationId,
    job_id: &JobId,
    item_ids: &[ItemId],
    include_cancelled: bool,
) -> Result<(), CommandErrorEnvelope> {
    let result = match item_ids {
        [] => service
            .manager()
            .retry_job(
                job_id,
                if include_cancelled {
                    RetryMode::FailedAndCancelled
                } else {
                    RetryMode::FailedOnly
                },
            )
            .map(|_| ()),
        [item_id] => {
            let belongs_to_job = service
                .manager()
                .job_detail_snapshot(job_id, 0, u32::MAX)
                .map_err(|error| manager_command_error(correlation_id.clone(), error))?
                .items
                .items
                .iter()
                .any(|item| item.id == *item_id);
            if !belongs_to_job {
                return Err(command_error(
                    correlation_id.clone(),
                    AppError::new(
                        "item.job_mismatch",
                        ErrorCategory::Validation,
                        "errors.itemJobMismatch",
                        "The selected item does not belong to the requested job.",
                    ),
                ));
            }
            service.manager().retry_item(item_id)
        }
        _ => {
            return Err(command_error(
                correlation_id.clone(),
                AppError::new(
                    "job.batch_item_retry_unavailable",
                    ErrorCategory::Validation,
                    "errors.batchRetryUnavailable",
                    "Retrying a selected group of items is not available yet.",
                ),
            ));
        }
    };
    result.map_err(|error| manager_command_error(correlation_id.clone(), error))
}

fn accepted_job(
    service: &BackendService,
    correlation_id: CorrelationId,
    job_id: &JobId,
) -> Result<JobCommandResponse, CommandErrorEnvelope> {
    let snapshot = service
        .manager()
        .job_snapshot(job_id)
        .map_err(|error| manager_command_error(correlation_id.clone(), error))?;
    let revision = snapshot.revision;
    Ok(CommandAccepted {
        schema_version: IPC_SCHEMA_VERSION,
        correlation_id,
        revision,
        snapshot,
    })
}

fn ensure_schema(
    received: u16,
    correlation_id: &CorrelationId,
) -> Result<(), CommandErrorEnvelope> {
    if received == IPC_SCHEMA_VERSION {
        return Ok(());
    }
    Err(command_error(
        correlation_id.clone(),
        AppError::new(
            "protocol.schema_version_unsupported",
            ErrorCategory::Protocol,
            "errors.schemaVersionUnsupported",
            "The request schema version is not supported by this backend.",
        )
        .with_message_arg("received", received.to_string())
        .with_message_arg("supported", IPC_SCHEMA_VERSION.to_string()),
    ))
}
