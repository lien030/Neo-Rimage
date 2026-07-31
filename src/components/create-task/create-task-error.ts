import { CreateTaskValidationError } from "@/features/create-task";

const VALIDATION_ERROR_KEYS = {
  inputs_required: "createTaskErrorInputsRequired",
  input_path_required: "createTaskErrorInputPath",
  encoder_unsupported: "createTaskErrorEncoderUnsupported",
  quality_invalid: "createTaskErrorQuality",
  chroma_quality_invalid: "createTaskErrorChromaQuality",
  smoothing_invalid: "createTaskErrorSmoothing",
  chroma_subsample_invalid: "createTaskErrorChromaSubsample",
  jpeg_quality_invalid: "createTaskErrorJpegQuality",
  avif_quality_invalid: "createTaskErrorAvifQuality",
  avif_alpha_quality_invalid: "createTaskErrorAvifAlphaQuality",
  avif_speed_invalid: "createTaskErrorAvifSpeed",
  oxipng_effort_invalid: "createTaskErrorOxiPngEffort",
  webp_quality_invalid: "createTaskErrorWebPQuality",
  webp_slight_loss_invalid: "createTaskErrorWebPSlightLoss",
  webp_slight_loss_requires_lossless:
    "createTaskErrorWebPSlightLossRequiresLossless",
  resize_width_invalid: "createTaskErrorResizeWidth",
  resize_height_invalid: "createTaskErrorResizeHeight",
  output_directory_required: "createTaskErrorOutputDirectory",
  suffix_invalid: "createTaskErrorSuffix",
  backup_requires_replace: "createTaskErrorBackupPolicy",
} satisfies Record<CreateTaskValidationError["code"], string>;

export function createTaskErrorMessage(
  error: unknown,
  translate: (key: string) => string,
): string {
  if (error instanceof CreateTaskValidationError) {
    return translate(VALIDATION_ERROR_KEYS[error.code]);
  }

  if (isRecord(error)) {
    const appError = isRecord(error.error) ? error.error : error;
    if (typeof appError.fallbackMessage === "string") {
      return appError.fallbackMessage;
    }
    if (typeof appError.message === "string") {
      return appError.message;
    }
  }

  if (error instanceof Error && error.message) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return translate("createTaskErrorUnknown");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
