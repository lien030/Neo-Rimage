//! Maps executor termination into authoritative item state.
//!
//! The scheduler owns slot release and subsequent dispatch; this module owns
//! only result validation, terminal item transitions, and error enrichment.

use std::any::Any;

use crate::domain::{
    AppError, ErrorCategory, ErrorContext, ItemId, ItemStatus, JobId, ProcessingStage,
};

use super::{transition_item, ExecutionOutcome, FinishedOutcome, ItemRecord};

pub(super) fn apply_finished_outcome(
    item: &mut ItemRecord,
    outcome: FinishedOutcome,
    job_cancel_requested: bool,
    job_id: &JobId,
    item_id: &ItemId,
) {
    match outcome {
        FinishedOutcome::Engine(ExecutionOutcome::Succeeded(result)) => {
            // A buggy executor must not be able to complete a different item or
            // an earlier retry attempt with a plausible-looking result.
            let identity_matches = result.job_id == *job_id
                && result.item_id == *item_id
                && result.attempt == item.attempt;
            if !identity_matches {
                fail_item_with_internal_error(
                    item,
                    "executor.result_identity_mismatch",
                    "errors.executorResultIdentityMismatch",
                    "executor result identity did not match the dispatched item".to_owned(),
                    job_id,
                    item_id,
                );
                return;
            }

            transition_item(item, ItemStatus::Succeeded);
            item.stage = Some(ProcessingStage::Complete);
            for warning in &result.warnings {
                if !item.warnings.contains(warning) {
                    item.warnings.push(warning.clone());
                }
            }
            item.result = Some(result);
            item.error = None;
        }
        FinishedOutcome::Engine(ExecutionOutcome::Failed(mut error)) => {
            transition_item(item, ItemStatus::Failed);
            fill_error_context(&mut error, job_id, item_id, item.stage);
            item.result = None;
            item.error = Some(error);
        }
        FinishedOutcome::Engine(ExecutionOutcome::Cancelled) => {
            if job_cancel_requested || item.status == ItemStatus::Cancelling {
                transition_item(item, ItemStatus::Cancelled);
                item.result = None;
                item.error = None;
            } else {
                fail_item_with_internal_error(
                    item,
                    "executor.unexpected_cancelled",
                    "errors.executorUnexpectedCancelled",
                    "executor returned cancelled without a manager cancellation request".to_owned(),
                    job_id,
                    item_id,
                );
            }
        }
        FinishedOutcome::Engine(ExecutionOutcome::Skipped) => {
            transition_item(item, ItemStatus::Skipped);
            item.result = None;
            item.error = None;
        }
        FinishedOutcome::Panicked(message) => fail_item_with_internal_error(
            item,
            "executor.panicked",
            "errors.executorPanicked",
            message,
            job_id,
            item_id,
        ),
        FinishedOutcome::Unavailable(message) => fail_item_with_internal_error(
            item,
            "executor.unavailable",
            "errors.executorUnavailable",
            message,
            job_id,
            item_id,
        ),
    }
}

fn fail_item_with_internal_error(
    item: &mut ItemRecord,
    code: &'static str,
    message_key: &'static str,
    fallback_message: String,
    job_id: &JobId,
    item_id: &ItemId,
) {
    transition_item(item, ItemStatus::Failed);
    item.result = None;
    item.error = Some(internal_executor_error(
        code,
        message_key,
        fallback_message,
        job_id,
        item_id,
        item.stage,
    ));
}

pub(super) fn fill_error_context(
    error: &mut AppError,
    job_id: &JobId,
    item_id: &ItemId,
    stage: Option<ProcessingStage>,
) {
    if error.context.job_id.is_none() {
        error.context.job_id = Some(job_id.clone());
    }
    if error.context.item_id.is_none() {
        error.context.item_id = Some(item_id.clone());
    }
    if error.context.stage.is_none() {
        error.context.stage = stage;
    }
}

fn internal_executor_error(
    code: &'static str,
    message_key: &'static str,
    fallback_message: String,
    job_id: &JobId,
    item_id: &ItemId,
    stage: Option<ProcessingStage>,
) -> AppError {
    AppError::new(code, ErrorCategory::Internal, message_key, fallback_message).with_context(
        ErrorContext {
            job_id: Some(job_id.clone()),
            item_id: Some(item_id.clone()),
            stage,
            path: None,
        },
    )
}

pub(super) fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "executor panicked with a non-string payload".to_owned()
    }
}
