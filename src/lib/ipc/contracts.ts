export const IPC_SCHEMA_VERSION = 1 as const;
export const JOB_CONFIG_VERSION = 1 as const;

export type AppError = { code: ErrorCode, category: ErrorCategory, messageKey: string, messageArgs: { [key in string]: string }, fallbackMessage: string, retryable: boolean, fieldErrors: Array<FieldError>, context: ErrorContext, diagnosticId: DiagnosticId | null, };

export type AvifAlphaMode = "unassociated_dirty" | "unassociated_clean" | "premultiplied";

export type AvifColorSpace = "ycbcr" | "rgb";

export type AvifConfig = { quality: number, alphaQuality: number | null, speed: number, colorSpace: AvifColorSpace, alphaMode: AvifAlphaMode, };

export type BackendCapabilities = { schemaVersion: number, jobConfigVersion: number, backendVersion: string, rimageVersion: string, rimageRevision: string | null, encoders: Array<EncoderCapability>, operations: Array<OperationCapability>, metadata: MetadataCapability, concurrency: ConcurrencyCapability, };

export type BackendNotice = { "kind": "capability_degraded", "payload": { reason_code: string, } } | { "kind": "temporary_files_cleaned", "payload": { removed: number, failed: number, } } | { "kind": "event_bridge_reconnected" } | { "kind": "shutdown_timed_out", "payload": { item_ids: Array<ItemId>, } };

export type BackendSnapshot = { schemaVersion: number, revision: Revision, generatedAt: TimestampMs, scheduler: SchedulerSnapshot, workerSlots: Array<WorkerSlotSnapshot>, jobs: Array<JobSnapshot>, };

export type BackupPolicy = "disabled" | "enabled";

export type CollisionPolicy = "fail" | "replace" | "auto_rename";

export type ColorProfilePolicy = "preserve_when_supported" | "convert_to_srgb" | "strip_after_conversion";

export type CommandAccepted<T> = { schemaVersion: number, correlationId: CorrelationId, revision: Revision, snapshot: T, };

export type CommandErrorEnvelope = { schemaVersion: number, correlationId: CorrelationId, error: AppError, };

export type ConcurrencyCapability = { default: number, minimum: number, maximum: number, };

export type CorrelationId = string;

export type CreateJobCommand = { correlationId: CorrelationId, request: CreateJobRequest, };

export type CreateJobRequest = { schemaVersion: number, inputs: Array<InputResource>, operations: Array<Operation>, encoder: EncoderConfig, output: OutputPolicy, metadata: MetadataPolicy, inputAcceptance: InputAcceptancePolicy, scheduling: SchedulingHint | null, };

export type CreateJobResponse = { schemaVersion: number, correlationId: CorrelationId, revision: Revision, job: JobSnapshot, items: Array<ItemSnapshot>, rejectedInputs: Array<RejectedInput>, };

export type DiagnosticId = string;

export type DitherOperation = { strength: number | null, };

export type EmbeddedMetadataPolicy = "preserve_when_supported" | "strip";

export type EncoderCapability = { kind: EncoderKind, available: boolean, outputExtensions: Array<string>, options: Array<OptionCapability>, limitations: Array<string>, };

export type EncoderConfig = { "kind": "mozjpeg", "options": MozJpegConfig } | { "kind": "jpeg", "options": JpegConfig } | { "kind": "avif", "options": AvifConfig } | { "kind": "oxipng", "options": OxiPngConfig } | { "kind": "webp", "options": WebPConfig } | { "kind": "jpeg_xl" } | { "kind": "png" } | { "kind": "farbfeld" } | { "kind": "ppm" } | { "kind": "qoi" };

export type EncoderKind = "mozjpeg" | "jpeg" | "avif" | "oxipng" | "webp" | "jpeg_xl" | "png" | "farbfeld" | "ppm" | "qoi";

export type EngineWarning = { code: string, stage: ProcessingStage | null, messageKey: string, messageArgs: { [key in string]: string }, fallbackMessage: string, };

export type ErrorCategory = "validation" | "protocol" | "input" | "processing" | "encoding" | "output" | "metadata" | "cancelled" | "backend" | "internal";

export type ErrorCode = string;

export type ErrorContext = { jobId: JobId | null, itemId: ItemId | null, stage: ProcessingStage | null, path: string | null, };

export type FieldError = { fieldPath: string, code: ErrorCode, messageKey: string, messageArgs: { [key in string]: string }, };

export type InputAcceptancePolicy = "reject_all" | "accept_valid";

export type InputResource = { path: string, kind: InputResourceKind, scanRecursively: boolean, };

export type InputResourceKind = "file" | "directory";

export type ItemControlAvailability = { canCancel: boolean, canRetry: boolean, };

export type ItemId = string;

export type ItemProgress = { stage: ProcessingStage, measure: ProgressMeasure, };

export type ItemResultSummary = { outputPath: string, inputBytes: number, outputBytes: number, durationMs: number, };

export type ItemSnapshot = { id: ItemId, jobId: JobId, revision: Revision, sequence: number, attempt: number, inputPath: string, outputPath: string | null, status: ItemStatus, stage: ProcessingStage | null, progress: ItemProgress | null, workerSlotId: WorkerSlotId | null, createdAt: TimestampMs, startedAt: TimestampMs | null, finishedAt: TimestampMs | null, controls: ItemControlAvailability, result: ItemResultSummary | null, error: AppError | null, warnings: Array<EngineWarning>, };

export type ItemStatus = "queued" | "running" | "cancelling" | "succeeded" | "failed" | "cancelled" | "skipped";

export type JobCommand = { schemaVersion: number, correlationId: CorrelationId, jobId: JobId, };

export type JobControlAvailability = { canPause: boolean, canResume: boolean, canCancel: boolean, canRetry: boolean, canRemove: boolean, };

export type JobCounts = { total: number, queued: number, running: number, cancelling: number, succeeded: number, failed: number, cancelled: number, skipped: number, };

export type JobDetailSnapshot = { schemaVersion: number, revision: Revision, job: JobSnapshot, items: Page<ItemSnapshot>, };

export type JobId = string;

export type JobProgressSnapshot = { completedItems: number, totalItems: number, activeItems: number, };

export type JobSnapshot = { id: JobId, revision: Revision, configVersion: number, encoder: EncoderKind, status: JobStatus, createdAt: TimestampMs, updatedAt: TimestampMs, counts: JobCounts, progress: JobProgressSnapshot, controls: JobControlAvailability, result: ResultSummary | null, error: AppError | null, };

export type JobStatus = "queued" | "running" | "paused" | "cancelling" | "succeeded" | "partially_succeeded" | "failed" | "cancelled";

export type JpegConfig = { quality: number, progressive: boolean, };

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]: JsonValue } | null;

export type MetadataCapability = { embeddedMetadata: boolean, iccProfiles: boolean, autoOrient: boolean, processingReport: boolean, };

export type MetadataPolicy = { embedded: EmbeddedMetadataPolicy, colorProfile: ColorProfilePolicy, report: ProcessingReportPolicy, };

export type MozJpegColorSpace = "ycbcr" | "rgb" | "grayscale";

export type MozJpegConfig = { quality: number, chromaQuality: number | null, progressive: boolean, optimizeCoding: boolean, smoothing: number, colorSpace: MozJpegColorSpace, trellisMultipass: boolean, chromaSubsample: number | null, quantizationTable: MozJpegQuantizationTable | null, };

export type MozJpegQuantizationTable = "ahumada_watson_peterson" | "annex_k" | "flat" | "klein_silverstein_carney" | "msssim" | "n_robidoux" | "psnr_hvs" | "peterson_ahumada_watson" | "watson_taylor_borthwick";

export type NoticeEventEnvelope = { schemaVersion: number, occurredAt: TimestampMs, notice: BackendNotice, };

export type Operation = { "kind": "resize", "config": ResizeOperation } | { "kind": "quantize", "config": QuantizeOperation } | { "kind": "dither", "config": DitherOperation } | { "kind": "premultiply_alpha" };

export type OperationCapability = { kind: OperationKind, available: boolean, options: Array<OptionCapability>, limitations: Array<string>, };

export type OperationKind = "resize" | "quantize" | "dither" | "premultiply_alpha";

export type OptionCapability = { path: string, valueType: OptionValueType, required: boolean, defaultValue: JsonValue | null, constraints: OptionConstraints, requires: Array<string>, conflictsWith: Array<string>, };

export type OptionConstraints = { minimum: number | null, maximum: number | null, step: number | null, allowedValues: Array<JsonValue>, };

export type OptionValueType = "boolean" | "integer" | "number" | "string" | "enum";

export type OutputLocation = { "kind": "same_directory" } | { "kind": "directory", "path": string };

export type OutputPolicy = { location: OutputLocation, preserveStructure: boolean, suffix: string, collision: CollisionPolicy, sourceBackup: BackupPolicy, existingOutputBackup: BackupPolicy, };

export type OxiPngConfig = { interlace: boolean, effort: number, };

export type Page<T> = { items: Array<T>, offset: number, limit: number, total: number, };

export type ProcessingReportPolicy = { "kind": "disabled" } | { "kind": "json", "path": string };

export type ProcessingStage = "preflight" | "inspect" | "decode" | "normalize" | "operations" | "encode" | "commit" | "metadata_finalize" | "complete";

export type ProgressMeasure = { "kind": "indeterminate" } | { "kind": "fraction", completed: number, total: number, };

export type QuantizeOperation = { quality: number, };

export type RejectedInput = { inputIndex: number, path: string, error: AppError, };

export type ResizeFilter = "nearest" | "bilinear" | "hamming" | "catmull_rom" | "mitchell" | "lanczos3";

export type ResizeMode = { "kind": "exact", "value": { width: number, height: number, } } | { "kind": "fit_width", "value": { width: number, } } | { "kind": "fit_height", "value": { height: number, } } | { "kind": "percentage", "value": { percent: number, } } | { "kind": "scale", "value": { factor: number, } };

export type ResizeOperation = { mode: ResizeMode, filter: ResizeFilter, allowUpscale: boolean, allowDownscale: boolean, };

export type ResultSummary = { totalInputBytes: number, totalOutputBytes: number, durationMs: number, succeeded: number, failed: number, cancelled: number, skipped: number, };

export type RetryItemsCommand = { schemaVersion: number, correlationId: CorrelationId, jobId: JobId, itemIds: Array<ItemId>, includeCancelled: boolean, };

export type Revision = number;

export type SchedulerMode = "running" | "paused" | "shutting_down";

export type SchedulerSnapshot = { mode: SchedulerMode, desiredConcurrency: number, effectiveConcurrency: number, maxConcurrency: number, activeItems: number, queuedItems: number, };

export type SchedulingHint = { requestedConcurrency: number | null, };

export type SetSchedulerPausedCommand = { schemaVersion: number, correlationId: CorrelationId, paused: boolean, };

export type SetWorkerCountCommand = { schemaVersion: number, correlationId: CorrelationId, desiredConcurrency: number, };

export type StateEvent = { "kind": "snapshot_invalidated" } | { "kind": "scheduler_changed", "payload": SchedulerSnapshot } | { "kind": "worker_slots_changed", "payload": Array<WorkerSlotSnapshot> } | { "kind": "job_changed", "payload": JobSnapshot } | { "kind": "item_changed", "payload": ItemSnapshot } | { "kind": "progress_changed", "payload": { job_id: JobId, item_id: ItemId, progress: ItemProgress, } } | { "kind": "backend_shutting_down", "payload": { grace_period_ms: number, } };

export type StateEventEnvelope = { schemaVersion: number, revision: Revision, occurredAt: TimestampMs, correlationId: CorrelationId | null, event: StateEvent, };

export type TimestampMs = number;

export type WebPConfig = { lossless: boolean, quality: number, slightLoss: number, exact: boolean, };

export type WorkerSlotId = string;

export type WorkerSlotSnapshot = { id: WorkerSlotId, status: WorkerSlotStatus, itemId: ItemId | null, inputPath: string | null, stage: ProcessingStage | null, progress: ItemProgress | null, };

export type WorkerSlotStatus = "idle" | "busy" | "draining";
