import type {
  AvifAlphaMode,
  AvifColorSpace,
  CollisionPolicy,
  ColorProfilePolicy,
  EmbeddedMetadataPolicy,
  EncoderKind,
  InputAcceptancePolicy,
  MozJpegColorSpace,
  MozJpegQuantizationTable,
  OutputLocation,
  ResizeFilter,
  ResizeMode,
} from "@/lib/ipc/contracts";

export interface ResizeDraftValue {
  enabled: boolean;
  mode: ResizeMode["kind"];
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

export interface JpegDraftValue {
  quality: string;
  progressive: boolean;
}

export interface AvifDraftValue {
  quality: string;
  alphaQuality: string;
  speed: string;
  colorSpace: AvifColorSpace;
  alphaMode: AvifAlphaMode;
}

export interface OxiPngDraftValue {
  interlace: boolean;
  effort: string;
}

export interface WebPDraftValue {
  lossless: boolean;
  quality: string;
  slightLoss: string;
  exact: boolean;
}

export interface OutputDraftValue {
  locationMode: OutputLocation["kind"];
  outputDirectory: string;
  preserveStructure: boolean;
  suffix: string;
  collision: CollisionPolicy;
  sourceBackup: boolean;
  existingOutputBackup: boolean;
}

export interface MetadataDraftValue {
  embedded: EmbeddedMetadataPolicy;
  colorProfile: ColorProfilePolicy;
  reportEnabled: boolean;
  reportPath: string;
}

/**
 * Editable and temporarily invalid values. This must never be stored as a
 * backend task snapshot or sent over IPC without validation/conversion.
 */
export interface CreateTaskFormValues {
  activeEncoder: EncoderKind;
  mozjpeg: MozJpegDraftValue;
  jpeg: JpegDraftValue;
  avif: AvifDraftValue;
  oxipng: OxiPngDraftValue;
  webp: WebPDraftValue;
  resize: ResizeDraftValue;
  output: OutputDraftValue;
  metadata: MetadataDraftValue;
  inputAcceptance: InputAcceptancePolicy;
}
