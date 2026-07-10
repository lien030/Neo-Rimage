import {
  IPC_SCHEMA_VERSION,
  type CreateJobRequest,
  type MozJpegConfig,
  type Operation,
  type ResizeMode,
} from "@/lib/ipc/contracts";

import type { CreateTaskFormValues } from "./domain";

export interface TaskCacheInput {
  path: string;
  fileName: string;
}

export type CreateTaskValidationCode =
  | "inputs_required"
  | "input_path_required"
  | "encoder_unsupported"
  | "quality_invalid"
  | "chroma_quality_invalid"
  | "smoothing_invalid"
  | "chroma_subsample_invalid"
  | "resize_width_invalid"
  | "resize_height_invalid"
  | "output_directory_required"
  | "suffix_invalid"
  | "backup_requires_replace";

export class CreateTaskValidationError extends Error {
  constructor(public readonly code: CreateTaskValidationCode) {
    super(code);
    this.name = "CreateTaskValidationError";
  }
}

export function buildCreateJobRequest(
  draft: CreateTaskFormValues,
  taskCache: readonly TaskCacheInput[],
): CreateJobRequest {
  if (taskCache.length === 0) {
    throw new CreateTaskValidationError("inputs_required");
  }
  if (taskCache.some((input) => input.path.trim().length === 0)) {
    throw new CreateTaskValidationError("input_path_required");
  }
  if (draft.activeEncoder !== "mozjpeg") {
    throw new CreateTaskValidationError("encoder_unsupported");
  }

  const encoder: MozJpegConfig = {
    quality: requiredNumber(
      draft.mozjpeg.quality,
      1,
      100,
      "quality_invalid",
    ),
    chromaQuality: optionalNumber(
      draft.mozjpeg.chromaQuality,
      1,
      100,
      "chroma_quality_invalid",
    ),
    progressive: draft.mozjpeg.progressive,
    optimizeCoding: draft.mozjpeg.optimizeCoding,
    smoothing: requiredInteger(
      draft.mozjpeg.smoothing,
      0,
      100,
      "smoothing_invalid",
    ),
    colorSpace: draft.mozjpeg.colorSpace,
    trellisMultipass: draft.mozjpeg.trellisMultipass,
    chromaSubsample: optionalInteger(
      draft.mozjpeg.chromaSubsample,
      1,
      4,
      "chroma_subsample_invalid",
    ),
    quantizationTable: draft.mozjpeg.quantizationTable,
  };

  const operations: Operation[] = [];
  if (draft.resize.enabled) {
    const mode: ResizeMode = {
      kind: "exact",
      value: {
        width: requiredInteger(
          draft.resize.width,
          1,
          4_294_967_295,
          "resize_width_invalid",
        ),
        height: requiredInteger(
          draft.resize.height,
          1,
          4_294_967_295,
          "resize_height_invalid",
        ),
      },
    };
    operations.push({
      kind: "resize",
      config: {
        mode,
        filter: draft.resize.filter,
        allowUpscale: draft.resize.allowUpscale,
        allowDownscale: draft.resize.allowDownscale,
      },
    });
  }

  const outputDirectory = draft.output.outputDirectory.trim();
  if (draft.output.locationMode === "directory" && !outputDirectory) {
    throw new CreateTaskValidationError("output_directory_required");
  }
  if (
    /[\\/:*?"<>|\0]/u.test(draft.output.suffix) ||
    draft.output.suffix.includes("..")
  ) {
    throw new CreateTaskValidationError("suffix_invalid");
  }
  if (
    draft.output.collision !== "replace" &&
    (draft.output.sourceBackup || draft.output.existingOutputBackup)
  ) {
    throw new CreateTaskValidationError("backup_requires_replace");
  }

  return {
    schemaVersion: IPC_SCHEMA_VERSION,
    inputs: taskCache.map((input) => ({
      path: input.path,
      kind: "file",
      scanRecursively: false,
    })),
    operations,
    encoder: { kind: "mozjpeg", options: encoder },
    output: {
      location:
        draft.output.locationMode === "same_directory"
          ? { kind: "same_directory" }
          : { kind: "directory", path: outputDirectory },
      preserveStructure:
        draft.output.locationMode === "directory" &&
        draft.output.preserveStructure,
      suffix: draft.output.suffix,
      collision: draft.output.collision,
      sourceBackup: draft.output.sourceBackup ? "enabled" : "disabled",
      existingOutputBackup: draft.output.existingOutputBackup
        ? "enabled"
        : "disabled",
    },
    metadata: {
      embedded: draft.metadata.embedded,
      colorProfile: draft.metadata.colorProfile,
      report: draft.metadata.reportEnabled
        ? { kind: "json", path: draft.metadata.reportPath.trim() }
        : { kind: "disabled" },
    },
    inputAcceptance: draft.inputAcceptance,
    scheduling: null,
  };
}

export function createCorrelationId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `create-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function requiredNumber(
  value: string,
  minimum: number,
  maximum: number,
  code: CreateTaskValidationCode,
): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < minimum || parsed > maximum) {
    throw new CreateTaskValidationError(code);
  }
  return parsed;
}

function optionalNumber(
  value: string,
  minimum: number,
  maximum: number,
  code: CreateTaskValidationCode,
): number | null {
  return value.trim() === ""
    ? null
    : requiredNumber(value, minimum, maximum, code);
}

function requiredInteger(
  value: string,
  minimum: number,
  maximum: number,
  code: CreateTaskValidationCode,
): number {
  const parsed = requiredNumber(value, minimum, maximum, code);
  if (!Number.isInteger(parsed)) {
    throw new CreateTaskValidationError(code);
  }
  return parsed;
}

function optionalInteger(
  value: string,
  minimum: number,
  maximum: number,
  code: CreateTaskValidationCode,
): number | null {
  return value.trim() === ""
    ? null
    : requiredInteger(value, minimum, maximum, code);
}
