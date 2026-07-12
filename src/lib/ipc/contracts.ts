export const IPC_SCHEMA_VERSION = 1 as const;
export const JOB_CONFIG_VERSION = 1 as const;

export type JobId = string;
export type ItemId = string;
export type WorkerSlotId = string;
export type CorrelationId = string;
export type DiagnosticId = string;
export type Revision = number;
export type TimestampMs = number;

export type InputResourceKind = "file" | "directory";
export type InputAcceptancePolicy = "reject_all" | "accept_valid";

export interface InputResource {
  path: string;
  kind: InputResourceKind;
  scanRecursively: boolean;
}

export type ResizeFilter =
  | "nearest"
  | "bilinear"
  | "hamming"
  | "catmull_rom"
  | "mitchell"
  | "lanczos3";

export type ResizeMode =
  | { kind: "exact"; value: { width: number; height: number } }
  | { kind: "fit_width"; value: { width: number } }
  | { kind: "fit_height"; value: { height: number } }
  | { kind: "percentage"; value: { percent: number } }
  | { kind: "scale"; value: { factor: number } };

export interface ResizeOperation {
  mode: ResizeMode;
  filter: ResizeFilter;
  allowUpscale: boolean;
  allowDownscale: boolean;
}

export interface QuantizeOperation {
  quality: number;
}

export interface DitherOperation {
  strength: number | null;
}

export type Operation =
  | { kind: "resize"; config: ResizeOperation }
  | { kind: "quantize"; config: QuantizeOperation }
  | { kind: "dither"; config: DitherOperation }
  | { kind: "premultiply_alpha" };

export type EncoderKind =
  | "mozjpeg"
  | "jpeg"
  | "avif"
  | "oxipng"
  | "webp"
  | "jpeg_xl"
  | "png"
  | "farbfeld"
  | "ppm"
  | "qoi";

export type MozJpegColorSpace = "ycbcr" | "rgb" | "grayscale";
export type MozJpegQuantizationTable =
  | "ahumada_watson_peterson"
  | "annex_k"
  | "flat"
  | "klein_silverstein_carney"
  | "msssim"
  | "n_robidoux"
  | "psnr_hvs"
  | "peterson_ahumada_watson"
  | "watson_taylor_borthwick";

export interface MozJpegConfig {
  quality: number;
  chromaQuality: number | null;
  progressive: boolean;
  optimizeCoding: boolean;
  smoothing: number;
  colorSpace: MozJpegColorSpace;
  trellisMultipass: boolean;
  chromaSubsample: number | null;
  quantizationTable: MozJpegQuantizationTable | null;
}

export interface JpegConfig {
  quality: number;
  progressive: boolean;
}

export type AvifColorSpace = "ycbcr" | "rgb";
export type AvifAlphaMode =
  | "unassociated_dirty"
  | "unassociated_clean"
  | "premultiplied";

export interface AvifConfig {
  quality: number;
  alphaQuality: number | null;
  speed: number;
  colorSpace: AvifColorSpace;
  alphaMode: AvifAlphaMode;
}

export interface OxiPngConfig {
  interlace: boolean;
  effort: number;
}

export interface WebPConfig {
  lossless: boolean;
  quality: number;
  slightLoss: number;
  exact: boolean;
}

export type EncoderConfig =
  | { kind: "mozjpeg"; options: MozJpegConfig }
  | { kind: "jpeg"; options: JpegConfig }
  | { kind: "avif"; options: AvifConfig }
  | { kind: "oxipng"; options: OxiPngConfig }
  | { kind: "webp"; options: WebPConfig }
  | { kind: "jpeg_xl" }
  | { kind: "png" }
  | { kind: "farbfeld" }
  | { kind: "ppm" }
  | { kind: "qoi" };

export type OutputLocation =
  | { kind: "same_directory" }
  | { kind: "directory"; path: string };
export type CollisionPolicy = "fail" | "replace" | "auto_rename";
export type BackupPolicy = "disabled" | "enabled";

export interface OutputPolicy {
  location: OutputLocation;
  preserveStructure: boolean;
  suffix: string;
  collision: CollisionPolicy;
  sourceBackup: BackupPolicy;
  existingOutputBackup: BackupPolicy;
}

export type EmbeddedMetadataPolicy = "preserve_when_supported" | "strip";
export type ColorProfilePolicy =
  | "preserve_when_supported"
  | "convert_to_srgb"
  | "strip_after_conversion";
export type ProcessingReportPolicy =
  | { kind: "disabled" }
  | { kind: "json"; path: string };

export interface MetadataPolicy {
  embedded: EmbeddedMetadataPolicy;
  colorProfile: ColorProfilePolicy;
  report: ProcessingReportPolicy;
}

export interface SchedulingHint {
  requestedConcurrency: number | null;
}

export interface CreateJobRequest {
  schemaVersion: number;
  inputs: InputResource[];
  operations: Operation[];
  encoder: EncoderConfig;
  output: OutputPolicy;
  metadata: MetadataPolicy;
  inputAcceptance: InputAcceptancePolicy;
  scheduling: SchedulingHint | null;
}

export type ProcessingStage =
  | "preflight"
  | "inspect"
  | "decode"
  | "normalize"
  | "operations"
  | "encode"
  | "commit"
  | "metadata_finalize"
  | "complete";

export type ProgressMeasure =
  | { kind: "indeterminate" }
  | { kind: "fraction"; completed: number; total: number };

export interface ItemProgress {
  stage: ProcessingStage;
  measure: ProgressMeasure;
}

export interface EngineWarning {
  code: string;
  stage: ProcessingStage | null;
  messageKey: string;
  messageArgs: Record<string, string>;
  fallbackMessage: string;
}

export type ErrorCategory =
  | "validation"
  | "protocol"
  | "input"
  | "processing"
  | "encoding"
  | "output"
  | "metadata"
  | "cancelled"
  | "backend"
  | "internal";

export interface ErrorContext {
  jobId: JobId | null;
  itemId: ItemId | null;
  stage: ProcessingStage | null;
  path: string | null;
}

export interface FieldError {
  fieldPath: string;
  code: string;
  messageKey: string;
  messageArgs: Record<string, string>;
}

export interface AppError {
  code: string;
  category: ErrorCategory;
  messageKey: string;
  messageArgs: Record<string, string>;
  fallbackMessage: string;
  retryable: boolean;
  fieldErrors: FieldError[];
  context: ErrorContext;
  diagnosticId: DiagnosticId | null;
}

export interface CommandErrorEnvelope {
  schemaVersion: number;
  correlationId: CorrelationId;
  error: AppError;
}

export type JobStatus =
  | "queued"
  | "running"
  | "paused"
  | "cancelling"
  | "succeeded"
  | "partially_succeeded"
  | "failed"
  | "cancelled";

export type ItemStatus =
  | "queued"
  | "running"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "skipped";

export type SchedulerMode = "running" | "paused" | "shutting_down";
export type WorkerSlotStatus = "idle" | "busy" | "draining";

export interface SchedulerSnapshot {
  readonly mode: SchedulerMode;
  readonly desiredConcurrency: number;
  readonly effectiveConcurrency: number;
  readonly maxConcurrency: number;
  readonly activeItems: number;
  readonly queuedItems: number;
}

export interface WorkerSlotSnapshot {
  readonly id: WorkerSlotId;
  readonly status: WorkerSlotStatus;
  readonly itemId: ItemId | null;
  readonly inputPath: string | null;
  readonly stage: ProcessingStage | null;
  readonly progress: ItemProgress | null;
}

export interface JobCounts {
  readonly total: number;
  readonly queued: number;
  readonly running: number;
  readonly cancelling: number;
  readonly succeeded: number;
  readonly failed: number;
  readonly cancelled: number;
  readonly skipped: number;
}

export interface JobProgressSnapshot {
  readonly completedItems: number;
  readonly totalItems: number;
  readonly activeItems: number;
}

export interface JobControlAvailability {
  readonly canPause: boolean;
  readonly canResume: boolean;
  readonly canCancel: boolean;
  readonly canRetry: boolean;
  readonly canRemove: boolean;
}

export interface ItemControlAvailability {
  readonly canCancel: boolean;
  readonly canRetry: boolean;
}

export interface ResultSummary {
  readonly totalInputBytes: number;
  readonly totalOutputBytes: number;
  readonly durationMs: number;
  readonly succeeded: number;
  readonly failed: number;
  readonly cancelled: number;
  readonly skipped: number;
}

export interface ItemResultSummary {
  readonly outputPath: string;
  readonly inputBytes: number;
  readonly outputBytes: number;
  readonly durationMs: number;
}

export interface JobSnapshot {
  readonly id: JobId;
  readonly revision: Revision;
  readonly configVersion: number;
  readonly encoder: EncoderKind;
  readonly status: JobStatus;
  readonly createdAt: TimestampMs;
  readonly updatedAt: TimestampMs;
  readonly counts: JobCounts;
  readonly progress: JobProgressSnapshot;
  readonly controls: JobControlAvailability;
  readonly result: ResultSummary | null;
  readonly error: AppError | null;
}

export interface ItemSnapshot {
  readonly id: ItemId;
  readonly jobId: JobId;
  readonly revision: Revision;
  readonly sequence: number;
  readonly attempt: number;
  readonly inputPath: string;
  readonly outputPath: string | null;
  readonly status: ItemStatus;
  readonly stage: ProcessingStage | null;
  readonly progress: ItemProgress | null;
  readonly workerSlotId: WorkerSlotId | null;
  readonly createdAt: TimestampMs;
  readonly startedAt: TimestampMs | null;
  readonly finishedAt: TimestampMs | null;
  readonly controls: ItemControlAvailability;
  readonly result: ItemResultSummary | null;
  readonly error: AppError | null;
  readonly warnings: readonly EngineWarning[];
}

export interface BackendSnapshot {
  readonly schemaVersion: number;
  readonly revision: Revision;
  readonly generatedAt: TimestampMs;
  readonly scheduler: SchedulerSnapshot;
  readonly workerSlots: readonly WorkerSlotSnapshot[];
  readonly jobs: readonly JobSnapshot[];
}

export interface Page<T> {
  readonly items: readonly T[];
  readonly offset: number;
  readonly limit: number;
  readonly total: number;
}

export interface JobDetailSnapshot {
  readonly schemaVersion: number;
  readonly revision: Revision;
  readonly job: JobSnapshot;
  readonly items: Page<ItemSnapshot>;
}

export interface CreateJobCommand {
  correlationId: CorrelationId;
  request: CreateJobRequest;
}

export interface RejectedInput {
  readonly inputIndex: number;
  readonly path: string;
  readonly error: AppError;
}

export interface CreateJobResponse {
  readonly schemaVersion: number;
  readonly correlationId: CorrelationId;
  readonly revision: Revision;
  readonly job: JobSnapshot;
  readonly items: readonly ItemSnapshot[];
  readonly rejectedInputs: readonly RejectedInput[];
}

export interface JobCommand {
  schemaVersion: number;
  correlationId: CorrelationId;
  jobId: JobId;
}

export interface RetryItemsCommand extends JobCommand {
  itemIds: ItemId[];
  includeCancelled: boolean;
}

export interface SetSchedulerPausedCommand {
  schemaVersion: number;
  correlationId: CorrelationId;
  paused: boolean;
}

export interface SetWorkerCountCommand {
  schemaVersion: number;
  correlationId: CorrelationId;
  desiredConcurrency: number;
}

export interface CommandAccepted<T> {
  readonly schemaVersion: number;
  readonly correlationId: CorrelationId;
  readonly revision: Revision;
  readonly snapshot: T;
}

export type StateEvent =
  | { kind: "snapshot_invalidated" }
  | { kind: "scheduler_changed"; payload: SchedulerSnapshot }
  | { kind: "worker_slots_changed"; payload: WorkerSlotSnapshot[] }
  | { kind: "job_changed"; payload: JobSnapshot }
  | { kind: "item_changed"; payload: ItemSnapshot }
  | {
      kind: "progress_changed";
      payload: {
        jobId: JobId;
        itemId: ItemId;
        progress: ItemProgress;
      };
    }
  | { kind: "backend_shutting_down"; payload: { gracePeriodMs: number } };

export interface StateEventEnvelope {
  readonly schemaVersion: number;
  readonly revision: Revision;
  readonly occurredAt: TimestampMs;
  readonly correlationId: CorrelationId | null;
  readonly event: StateEvent;
}

export type BackendNotice =
  | { kind: "capability_degraded"; payload: { reasonCode: string } }
  | {
      kind: "temporary_files_cleaned";
      payload: { removed: number; failed: number };
    }
  | { kind: "event_bridge_reconnected" }
  | { kind: "shutdown_timed_out"; payload: { itemIds: ItemId[] } };

export interface NoticeEventEnvelope {
  readonly schemaVersion: number;
  readonly occurredAt: TimestampMs;
  readonly notice: BackendNotice;
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export type OptionValueType =
  | "boolean"
  | "integer"
  | "number"
  | "string"
  | "enum";

export interface OptionConstraints {
  readonly minimum: number | null;
  readonly maximum: number | null;
  readonly step: number | null;
  readonly allowedValues: readonly JsonValue[];
}

export interface OptionCapability {
  readonly path: string;
  readonly valueType: OptionValueType;
  readonly required: boolean;
  readonly defaultValue: JsonValue | null;
  readonly constraints: OptionConstraints;
  readonly requires: readonly string[];
  readonly conflictsWith: readonly string[];
}

export interface EncoderCapability {
  readonly kind: EncoderKind;
  readonly available: boolean;
  readonly outputExtensions: readonly string[];
  readonly options: readonly OptionCapability[];
  readonly limitations: readonly string[];
}

export type OperationKind =
  | "resize"
  | "quantize"
  | "dither"
  | "premultiply_alpha";

export interface OperationCapability {
  readonly kind: OperationKind;
  readonly available: boolean;
  readonly options: readonly OptionCapability[];
  readonly limitations: readonly string[];
}

export interface MetadataCapability {
  readonly embeddedMetadata: boolean;
  readonly iccProfiles: boolean;
  readonly autoOrient: boolean;
  readonly processingReport: boolean;
}

export interface ConcurrencyCapability {
  readonly default: number;
  readonly minimum: number;
  readonly maximum: number;
}

export interface BackendCapabilities {
  readonly schemaVersion: number;
  readonly jobConfigVersion: number;
  readonly backendVersion: string;
  readonly rimageVersion: string;
  readonly rimageRevision: string | null;
  readonly encoders: readonly EncoderCapability[];
  readonly operations: readonly OperationCapability[];
  readonly metadata: MetadataCapability;
  readonly concurrency: ConcurrencyCapability;
}
