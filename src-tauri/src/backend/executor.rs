use std::sync::Arc;

use crate::{
    domain::{EngineRequest, ErrorCategory},
    engine::{Engine, EngineContext, LocalEngine},
    jobs::{ExecutionContext, ExecutionOutcome, ExecutionTask, Executor},
};

/// Thin adapter that lets JobManager execute the Tauri-independent local
/// engine. It contains no scheduling or image-pipeline policy.
#[derive(Clone)]
pub struct LocalEngineExecutor {
    engine: Arc<dyn Engine>,
}

impl Default for LocalEngineExecutor {
    fn default() -> Self {
        Self::new(LocalEngine::new())
    }
}

impl LocalEngineExecutor {
    pub fn new(engine: impl Engine + 'static) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }

    pub fn from_shared(engine: Arc<dyn Engine>) -> Self {
        Self { engine }
    }
}

impl Executor for LocalEngineExecutor {
    fn execute(&self, task: &ExecutionTask, context: &ExecutionContext) -> ExecutionOutcome {
        let request = build_engine_request(task);
        let engine_context = EngineContext::new(&context.progress, &context.cancellation);
        match self.engine.execute(&request, &engine_context) {
            Ok(result) => ExecutionOutcome::Succeeded(result),
            Err(error) if error.category == ErrorCategory::Cancelled => ExecutionOutcome::Cancelled,
            Err(error) => ExecutionOutcome::Failed(error),
        }
    }
}

pub fn build_engine_request(task: &ExecutionTask) -> EngineRequest {
    EngineRequest {
        job_id: task.job.id.clone(),
        item_id: task.item.id.clone(),
        attempt: task.attempt,
        config_version: task.job.config_version,
        input_path: task.item.input_path.clone(),
        output: task.item.output.clone(),
        encoder: task.job.encoder.clone(),
        operations: task.job.operations.clone(),
        metadata: task.job.metadata.clone(),
    }
}
