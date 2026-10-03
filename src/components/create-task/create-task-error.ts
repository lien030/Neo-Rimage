import type { TOptions } from "i18next";

import { formatBackendError } from "@/features/backend";
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
  translate: (key: string, options?: TOptions) => string,
): string {
  if (error instanceof CreateTaskValidationError) {
    return translate(VALIDATION_ERROR_KEYS[error.code]);
  }

  return formatBackendError(error, translate, "createTaskErrorUnknown");
}
