import {
  generateCorrelationId,
  IPC_SCHEMA_VERSION,
  type AvifConfig,
  type CreateJobRequest,
  type EncoderConfig,
  type EncoderKind,
  type JpegConfig,
  type MetadataPolicy,
  type MozJpegConfig,
  type Operation,
  type OutputPolicy,
  type OxiPngConfig,
  type ResizeMode,
  type WebPConfig,
} from "@/lib/ipc";

import type {
  CreateTaskFormValues,
  MetadataDraftValue,
  OutputDraftValue,
  ResizeDraftValue,
} from "./domain";
import type { SelectedInputFile } from "./input-files";

const MAX_IMAGE_DIMENSION = 4_294_967_295;

export type CreateTaskValidationCode =
  | "inputs_required"
  | "input_path_required"
  | "encoder_unsupported"
  | "quality_invalid"
  | "chroma_quality_invalid"
  | "smoothing_invalid"
  | "chroma_subsample_invalid"
  | "jpeg_quality_invalid"
  | "avif_quality_invalid"
  | "avif_alpha_quality_invalid"
  | "avif_speed_invalid"
  | "oxipng_effort_invalid"
  | "webp_quality_invalid"
  | "webp_slight_loss_invalid"
  | "webp_slight_loss_requires_lossless"
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
  selectedInputs: readonly SelectedInputFile[],
  availableEncoders: readonly EncoderKind[],
): CreateJobRequest {
  // This is the form-to-IPC boundary: draft strings remain editable until all
  // values have been validated and converted into the versioned DTO below.
  if (selectedInputs.length === 0) {
    throw new CreateTaskValidationError("inputs_required");
  }
  if (selectedInputs.some((input) => input.path.trim().length === 0)) {
    throw new CreateTaskValidationError("input_path_required");
  }
  if (!availableEncoders.includes(draft.activeEncoder)) {
    throw new CreateTaskValidationError("encoder_unsupported");
  }

  const encoder = buildEncoderConfig(draft);
  const operations = buildOperations(draft.resize);
  const output = buildOutputPolicy(draft.output);

  return {
    schemaVersion: IPC_SCHEMA_VERSION,
    inputs: selectedInputs.map((input) => ({
      path: input.path,
      kind: "file",
      scanRecursively: false,
    })),
    operations,
    encoder,
    output,
    metadata: buildMetadataPolicy(draft.metadata),
    inputAcceptance: draft.inputAcceptance,
    scheduling: null,
  };
}

function buildOperations(resize: ResizeDraftValue): Operation[] {
  if (!resize.enabled) {
    return [];
  }

  // Only exact dimensions are exposed by the current form. Keeping the
  // conversion explicit prevents unused draft fields from leaking over IPC.
  const mode: ResizeMode = {
    kind: "exact",
    value: {
      width: requiredInteger(
        resize.width,
        1,
        MAX_IMAGE_DIMENSION,
        "resize_width_invalid",
      ),
      height: requiredInteger(
        resize.height,
        1,
        MAX_IMAGE_DIMENSION,
        "resize_height_invalid",
      ),
    },
  };

  return [
    {
      kind: "resize",
      config: {
        mode,
        filter: resize.filter,
        allowUpscale: resize.allowUpscale,
        allowDownscale: resize.allowDownscale,
      },
    },
  ];
}

function buildOutputPolicy(output: OutputDraftValue): OutputPolicy {
  const outputDirectory = output.outputDirectory.trim();
  if (output.locationMode === "directory" && !outputDirectory) {
    throw new CreateTaskValidationError("output_directory_required");
  }
  if (/[\\/:*?"<>|\0]/u.test(output.suffix) || output.suffix.includes("..")) {
    throw new CreateTaskValidationError("suffix_invalid");
  }
  if (
    output.collision !== "replace" &&
    (output.sourceBackup || output.existingOutputBackup)
  ) {
    throw new CreateTaskValidationError("backup_requires_replace");
  }

  return {
    location:
      output.locationMode === "same_directory"
        ? { kind: "same_directory" }
        : { kind: "directory", path: outputDirectory },
    preserveStructure:
      output.locationMode === "directory" && output.preserveStructure,
    suffix: output.suffix,
    collision: output.collision,
    sourceBackup: output.sourceBackup ? "enabled" : "disabled",
    existingOutputBackup: output.existingOutputBackup
      ? "enabled"
      : "disabled",
  };
}

function buildMetadataPolicy(metadata: MetadataDraftValue): MetadataPolicy {
  return {
    embedded: metadata.embedded,
    colorProfile: metadata.colorProfile,
    report: metadata.reportEnabled
      ? { kind: "json", path: metadata.reportPath.trim() }
      : { kind: "disabled" },
  };
}

function buildEncoderConfig(draft: CreateTaskFormValues): EncoderConfig {
  switch (draft.activeEncoder) {
    case "mozjpeg":
      return { kind: "mozjpeg", options: buildMozJpegConfig(draft) };
    case "jpeg": {
      const options: JpegConfig = {
        quality: requiredNumber(
          draft.jpeg.quality,
          1,
          100,
          "jpeg_quality_invalid",
        ),
        progressive: draft.jpeg.progressive,
      };
      return { kind: "jpeg", options };
    }
    case "avif": {
      const options: AvifConfig = {
        quality: requiredNumber(
          draft.avif.quality,
          1,
          100,
          "avif_quality_invalid",
        ),
        alphaQuality: optionalNumber(
          draft.avif.alphaQuality,
          1,
          100,
          "avif_alpha_quality_invalid",
        ),
        speed: requiredInteger(
          draft.avif.speed,
          1,
          10,
          "avif_speed_invalid",
        ),
        colorSpace: draft.avif.colorSpace,
        alphaMode: draft.avif.alphaMode,
      };
      return { kind: "avif", options };
    }
    case "oxipng": {
      const options: OxiPngConfig = {
        interlace: draft.oxipng.interlace,
        effort: requiredInteger(
          draft.oxipng.effort,
          0,
          6,
          "oxipng_effort_invalid",
        ),
      };
      return { kind: "oxipng", options };
    }
    case "webp": {
      const slightLoss = requiredInteger(
        draft.webp.slightLoss,
        0,
        100,
        "webp_slight_loss_invalid",
      );
      if (!draft.webp.lossless && slightLoss > 0) {
        throw new CreateTaskValidationError(
          "webp_slight_loss_requires_lossless",
        );
      }
      const options: WebPConfig = {
        lossless: draft.webp.lossless,
        quality: requiredNumber(
          draft.webp.quality,
          1,
          100,
          "webp_quality_invalid",
        ),
        slightLoss,
        exact: draft.webp.exact,
      };
      return { kind: "webp", options };
    }
    case "jpeg_xl":
      return { kind: "jpeg_xl" };
    case "png":
      return { kind: "png" };
    case "farbfeld":
      return { kind: "farbfeld" };
    case "ppm":
      return { kind: "ppm" };
    case "qoi":
      return { kind: "qoi" };
  }
}

function buildMozJpegConfig(draft: CreateTaskFormValues): MozJpegConfig {
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
  return encoder;
}

export function createCorrelationId(): string {
  return generateCorrelationId("create");
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
