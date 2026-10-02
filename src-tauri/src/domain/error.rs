use super::{CorrelationId, DiagnosticId, ItemId, JobId, ProcessingStage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Validation,
    Protocol,
    Input,
    Processing,
    Encoding,
    Output,
    Metadata,
    Cancelled,
    Backend,
    Internal,
}

/// Stable, forward-compatible machine code. UI behavior must not parse messages.
#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ErrorCode(pub String);

impl ErrorCode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for ErrorCode {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorContext {
    pub job_id: Option<JobId>,
    pub item_id: Option<ItemId>,
    pub stage: Option<ProcessingStage>,
    pub path: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    pub field_path: String,
    pub code: ErrorCode,
    pub message_key: String,
    #[serde(default)]
    pub message_args: BTreeMap<String, String>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub message_key: String,
    #[serde(default)]
    pub message_args: BTreeMap<String, String>,
    pub fallback_message: String,
    pub retryable: bool,
    #[serde(default)]
    pub field_errors: Vec<FieldError>,
    #[serde(default)]
    pub context: ErrorContext,
    pub diagnostic_id: Option<DiagnosticId>,
}

impl AppError {
    pub fn new(
        code: impl Into<ErrorCode>,
        category: ErrorCategory,
        message_key: impl Into<String>,
        fallback_message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category,
            message_key: message_key.into(),
            message_args: BTreeMap::new(),
            fallback_message: fallback_message.into(),
            retryable: false,
            field_errors: Vec::new(),
            context: ErrorContext::default(),
            diagnostic_id: None,
        }
    }

    pub fn validation(code: impl Into<ErrorCode>, field_errors: Vec<FieldError>) -> Self {
        let mut error = Self::new(
            code,
            ErrorCategory::Validation,
            "errors.validation",
            "The request contains invalid fields.",
        );
        error.field_errors = field_errors;
        error
    }

    pub fn cancelled(stage: Option<ProcessingStage>) -> Self {
        let mut error = Self::new(
            "task.cancelled",
            ErrorCategory::Cancelled,
            "errors.taskCancelled",
            "The task was cancelled.",
        );
        error.context.stage = stage;
        error
    }

    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn with_message_arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.message_args.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.fallback_message)
    }
}

impl std::error::Error for AppError {}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorEnvelope {
    pub schema_version: u16,
    pub correlation_id: CorrelationId,
    pub error: AppError,
}
