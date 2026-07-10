use crate::{
    domain::{AppError, CommandErrorEnvelope, CorrelationId, ErrorCategory, ErrorContext},
    jobs::ManagerError,
};

pub fn manager_error(error: ManagerError) -> AppError {
    match error {
        ManagerError::EmptyJob => AppError::new(
            "job.empty",
            ErrorCategory::Validation,
            "errors.jobEmpty",
            "A job must contain at least one supported input file.",
        ),
        ManagerError::JobNotFound(job_id) => AppError::new(
            "job.not_found",
            ErrorCategory::Validation,
            "errors.jobNotFound",
            "The requested job no longer exists.",
        )
        .with_context(ErrorContext {
            job_id: Some(job_id),
            ..ErrorContext::default()
        }),
        ManagerError::ItemNotFound(item_id) => AppError::new(
            "item.not_found",
            ErrorCategory::Validation,
            "errors.itemNotFound",
            "The requested item no longer exists.",
        )
        .with_context(ErrorContext {
            item_id: Some(item_id),
            ..ErrorContext::default()
        }),
        ManagerError::DuplicateJobId(job_id) => AppError::new(
            "job.duplicate_id",
            ErrorCategory::Internal,
            "errors.internalJobIdentity",
            "A generated job identity collided with an existing job.",
        )
        .with_context(ErrorContext {
            job_id: Some(job_id),
            ..ErrorContext::default()
        }),
        ManagerError::DuplicateItemId(item_id) => AppError::new(
            "item.duplicate_id",
            ErrorCategory::Internal,
            "errors.internalItemIdentity",
            "A generated item identity collided with an existing item.",
        )
        .with_context(ErrorContext {
            item_id: Some(item_id),
            ..ErrorContext::default()
        }),
        ManagerError::ItemJobMismatch { item_id, job_id } => AppError::new(
            "item.job_mismatch",
            ErrorCategory::Internal,
            "errors.internalItemOwnership",
            "An item was associated with the wrong job.",
        )
        .with_context(ErrorContext {
            job_id: Some(job_id),
            item_id: Some(item_id),
            ..ErrorContext::default()
        }),
        ManagerError::InvalidAction {
            job_id,
            state,
            action,
        } => AppError::new(
            "job.action_not_allowed",
            ErrorCategory::Validation,
            "errors.jobActionNotAllowed",
            "The requested job action is not allowed in its current state.",
        )
        .with_context(ErrorContext {
            job_id: Some(job_id),
            ..ErrorContext::default()
        })
        .with_message_arg("state", format!("{state:?}").to_lowercase())
        .with_message_arg("action", action),
        ManagerError::InvalidItemAction {
            item_id,
            state,
            action,
        } => AppError::new(
            "item.action_not_allowed",
            ErrorCategory::Validation,
            "errors.itemActionNotAllowed",
            "The requested item action is not allowed in its current state.",
        )
        .with_context(ErrorContext {
            item_id: Some(item_id),
            ..ErrorContext::default()
        })
        .with_message_arg("state", format!("{state:?}").to_lowercase())
        .with_message_arg("action", action),
        ManagerError::ConcurrencyOutOfRange {
            requested,
            minimum,
            maximum,
        } => AppError::new(
            "scheduler.concurrency_out_of_range",
            ErrorCategory::Validation,
            "errors.concurrencyOutOfRange",
            "The requested concurrency is outside the supported range.",
        )
        .with_message_arg("requested", requested.to_string())
        .with_message_arg("minimum", minimum.to_string())
        .with_message_arg("maximum", maximum.to_string()),
        ManagerError::ShuttingDown => AppError::new(
            "backend.shutting_down",
            ErrorCategory::Backend,
            "errors.backendShuttingDown",
            "The backend is shutting down and cannot accept this action.",
        ),
        ManagerError::TimedOut => AppError::new(
            "backend.operation_timed_out",
            ErrorCategory::Backend,
            "errors.backendOperationTimedOut",
            "The backend did not complete the operation before its deadline.",
        )
        .with_retryable(true),
    }
}

pub fn command_error(correlation_id: CorrelationId, error: AppError) -> CommandErrorEnvelope {
    CommandErrorEnvelope {
        schema_version: crate::domain::IPC_SCHEMA_VERSION,
        correlation_id,
        error,
    }
}

pub fn manager_command_error(
    correlation_id: CorrelationId,
    error: ManagerError,
) -> CommandErrorEnvelope {
    command_error(correlation_id, manager_error(error))
}
