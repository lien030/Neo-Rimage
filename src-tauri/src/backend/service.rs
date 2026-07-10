use std::sync::Arc;

use crate::{
    domain::{
        BackendCapabilities, CommandErrorEnvelope, ConcurrencyCapability, CreateJobCommand,
        CreateJobResponse, IPC_SCHEMA_VERSION, JOB_CONFIG_VERSION,
    },
    engine::{engine_capabilities, RIMAGE_SOURCE_REVISION, RIMAGE_SOURCE_VERSION},
    jobs::JobManager,
};

use super::{
    command_error, manager_command_error, manager_error, LocalEngineExecutor, NormalizedJob,
    RequestNormalizer,
};

#[derive(Clone)]
pub struct BackendService {
    normalizer: RequestNormalizer,
    manager: JobManager,
}

impl BackendService {
    pub fn new(maximum_concurrency: usize) -> Result<Self, crate::domain::AppError> {
        Self::with_normalizer(maximum_concurrency, RequestNormalizer::default())
    }

    pub fn with_normalizer(
        maximum_concurrency: usize,
        normalizer: RequestNormalizer,
    ) -> Result<Self, crate::domain::AppError> {
        let executor = Arc::new(LocalEngineExecutor::default());
        let manager = JobManager::new(executor, maximum_concurrency).map_err(manager_error)?;
        Ok(Self {
            normalizer,
            manager,
        })
    }

    pub fn manager(&self) -> &JobManager {
        &self.manager
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        let engine = engine_capabilities();
        let scheduler = self.manager.snapshot().scheduler;
        BackendCapabilities {
            schema_version: IPC_SCHEMA_VERSION,
            job_config_version: JOB_CONFIG_VERSION,
            backend_version: env!("CARGO_PKG_VERSION").to_owned(),
            rimage_version: RIMAGE_SOURCE_VERSION.to_owned(),
            rimage_revision: Some(RIMAGE_SOURCE_REVISION.to_owned()),
            encoders: engine.encoders,
            operations: engine.operations,
            metadata: engine.metadata,
            concurrency: ConcurrencyCapability {
                default: 1,
                minimum: 1,
                maximum: scheduler.max_concurrency,
            },
        }
    }

    pub fn normalize_request(
        &self,
        request: crate::domain::CreateJobRequest,
    ) -> Result<NormalizedJob, crate::domain::AppError> {
        self.normalizer.normalize(request)
    }

    pub fn create_job(
        &self,
        command: CreateJobCommand,
    ) -> Result<CreateJobResponse, CommandErrorEnvelope> {
        let correlation_id = command.correlation_id;
        let normalized = self
            .normalizer
            .normalize(command.request)
            .map_err(|error| command_error(correlation_id.clone(), error))?;
        let rejected_inputs = normalized.rejected_inputs;
        let job_id = self
            .manager
            .submit(normalized.submission)
            .map_err(|error| manager_command_error(correlation_id.clone(), error))?;
        let detail = self
            .manager
            .job_detail_snapshot(&job_id, 0, u32::MAX)
            .map_err(|error| manager_command_error(correlation_id.clone(), error))?;

        Ok(CreateJobResponse {
            schema_version: IPC_SCHEMA_VERSION,
            correlation_id,
            revision: detail.revision,
            job: detail.job,
            items: detail.items.items,
            rejected_inputs,
        })
    }
}
