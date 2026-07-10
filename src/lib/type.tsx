export type LanguageType = "zh" | "en" | "ja";

export interface AppState {
  running: boolean;
  recursiveFolders: boolean;
  isShowCreateTask: boolean;
  isShowDragDrop: boolean;
}


export enum EncoderType {
  Avif,
  Farbfeld,
  Mozjpeg,
  Jpeg,
  Jpeg_xl,
  Oxipng,
  Png,
  Ppm,
  Qoi,
  Webp
}

export enum ResizeFilterType {
  Nearest,
  Bilinear,
  Hamming,
  CatmullRom,
  Mitchell,
  Lanczos3
}

export interface TaskStore {
  taskList: Task[];
  taskCache: TaskCacheType[];
}

export interface Task extends TaskConfig {
  id: string;
  status: TaskStatusType;
}

export interface TaskConfig {
  filePath: string;
  fileName: string;
  encoder: EncoderType;
  encodeConfig?: EncodeConfig;
  resize: boolean;
  resizeConfig?: ResizeConfig;
  suffix: string;
  recursive: boolean;
  backup: boolean;
  outputFilePath?: string;
}


export interface TaskCacheType {
  path: string;
  fileName: string;
}

export interface ResizeConfig {
  firstly: boolean;
  width: number;
  height: number;
  filter: ResizeFilterType;
}

export interface EncodeConfig {
  mozjpeg?: MozjpegConfig;
}

export interface ProcessWorker {
  id: string;
  status: WorkerStatusType;
  task: Task | null;
}

export enum WorkerStatusType {
  Idle,
  Busy,
  Done
}

export enum TaskStatusType {
  Idle,
  Processing,
  Done,
  Error
}

export interface MozjpegConfig {
  quality: number;
  progressive: boolean;
  // progressive = baseline
  // https://github.com/SalOne22/rimage/blob/4d157b38f0709b76ad9d8b740f8c3e1166d6cc4f/src/cli/pipeline.rs#L270
  optimizeCoding: boolean;
  smoothing: number;
  colorSpace: MozjpegColorSpaceType;
  trellisMultipass: boolean;
}

export enum MozjpegColorSpaceType {
  JCS_UNKNOWN, // error/unspecified
  JCS_GRAYSCALE, // monochrome
  JCS_RGB, // red/green/blue as specified by the RGB_RED, RGB_GREEN, RGB_BLUE, and RGB_PIXELSIZE macros
  JCS_YCbCr, // Y/Cb/Cr (also known as YUV)
  JCS_CMYK, // C/M/Y/K
  JCS_YCCK, // Y/Cb/Cr/K
  JCS_EXT_RGB, // red/green/blue
  JCS_EXT_RGBX, // red/green/blue/x
  JCS_EXT_BGR, // blue/green/red
  JCS_EXT_BGRX, // blue/green/red/x
  JCS_EXT_XBGR, // x/blue/green/red
  JCS_EXT_XRGB, // x/red/green/blue
  JCS_EXT_RGBA, // red/green/blue/alpha
  JCS_EXT_BGRA, // blue/green/red/alpha
  JCS_EXT_ABGR, // alpha/blue/green/red
  JCS_EXT_ARGB, // alpha/red/green/blue
  JCS_RGB565 // 5-bit red/6-bit green/5-bit blue
}

export interface JpegConfig {
  quality: number;
  progressive: boolean;
}

export class DefaultJpegConfig implements JpegConfig {
  quality = 75;
  progressive = true;

  static default(): DefaultJpegConfig {
    return new DefaultJpegConfig();
  }
}
