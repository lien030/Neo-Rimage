import type {
  AvifAlphaMode,
  AvifColorSpace,
  EncoderKind,
  InputAcceptancePolicy,
  MozJpegColorSpace,
  MozJpegQuantizationTable,
  ResizeFilter,
} from "@/lib/ipc/contracts";

export interface InputDraftValue {
  localId: string;
  path: string;
  kind: "file" | "directory" | null;
  scanRecursively: boolean;
}

export interface ResizeDraftValue {
  enabled: boolean;
  mode: "exact" | "fit_width" | "fit_height" | "percentage" | "scale";
  width: string;
  height: string;
  percent: string;
  factor: string;
  filter: ResizeFilter;
  allowUpscale: boolean;
  allowDownscale: boolean;
}

export interface MozJpegDraftValue {
  quality: string;
  chromaQuality: string;
  progressive: boolean;
  optimizeCoding: boolean;
  smoothing: string;
  colorSpace: MozJpegColorSpace;
  trellisMultipass: boolean;
  chromaSubsample: string;
  quantizationTable: MozJpegQuantizationTable | null;
}

export interface AvifDraftValue {
  quality: string;
  alphaQuality: string;
  speed: string;
  colorSpace: AvifColorSpace;
  alphaMode: AvifAlphaMode;
}

export interface OutputDraftValue {
  locationMode: "same_directory" | "directory";
  outputDirectory: string;
  preserveStructure: boolean;
  suffix: string;
  collision: "fail" | "replace" | "auto_rename";
  sourceBackup: boolean;
  existingOutputBackup: boolean;
}

export interface MetadataDraftValue {
  embedded: "preserve_when_supported" | "strip";
  colorProfile:
    | "preserve_when_supported"
    | "convert_to_srgb"
    | "strip_after_conversion";
  reportEnabled: boolean;
  reportPath: string;
}

/**
 * Editable and temporarily invalid values. This must never be stored as a
 * backend task snapshot or sent over IPC without validation/conversion.
 */
export interface CreateTaskFormValues {
  inputs: InputDraftValue[];
  activeEncoder: EncoderKind;
  mozjpeg: MozJpegDraftValue;
  avif: AvifDraftValue;
  resize: ResizeDraftValue;
  output: OutputDraftValue;
  metadata: MetadataDraftValue;
  inputAcceptance: InputAcceptancePolicy;
}
