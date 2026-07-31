import { proxy } from "valtio";

import type {
  CreateTaskFormValues,
  CreateTaskUiState,
} from "./domain";
import type { SelectedInputFile } from "./input-files";

export function createDefaultCreateTaskForm(): CreateTaskFormValues {
  return {
    activeEncoder: "mozjpeg",
    mozjpeg: {
      quality: "75",
      chromaQuality: "",
      progressive: true,
      optimizeCoding: true,
      smoothing: "0",
      colorSpace: "ycbcr",
      trellisMultipass: false,
      chromaSubsample: "",
      quantizationTable: null,
    },
    jpeg: {
      quality: "80",
      progressive: false,
    },
    avif: {
      quality: "50",
      alphaQuality: "",
      speed: "6",
      colorSpace: "ycbcr",
      alphaMode: "unassociated_clean",
    },
    oxipng: {
      interlace: false,
      effort: "2",
    },
    webp: {
      lossless: false,
      quality: "75",
      slightLoss: "0",
      exact: false,
    },
    resize: {
      enabled: false,
      mode: "exact",
      width: "100",
      height: "100",
      percent: "100",
      factor: "1",
      filter: "lanczos3",
      allowUpscale: false,
      allowDownscale: true,
    },
    output: {
      locationMode: "same_directory",
      outputDirectory: "",
      preserveStructure: false,
      suffix: "-optimized",
      collision: "fail",
      sourceBackup: false,
      existingOutputBackup: false,
    },
    metadata: {
      embedded: "preserve_when_supported",
      colorProfile: "preserve_when_supported",
      reportEnabled: false,
      reportPath: "",
    },
    inputAcceptance: "reject_all",
  };
}

export const createTaskDraft = proxy<CreateTaskFormValues>(
  createDefaultCreateTaskForm(),
);

export const createTaskInputState = proxy<{ files: SelectedInputFile[] }>({
  files: [],
});

export const createTaskUiState = proxy<CreateTaskUiState>({
  isOpen: false,
  activeSection: "encoder",
  isSubmitting: false,
  isDirty: false,
  globalError: null,
});

export function markCreateTaskDirty(): void {
  createTaskUiState.isDirty = true;
  createTaskUiState.globalError = null;
}

/** Keeps draft mutations and their UI bookkeeping on the same state boundary. */
export function updateCreateTaskDraft(
  update: (draft: CreateTaskFormValues) => void,
): void {
  update(createTaskDraft);
  markCreateTaskDirty();
}

export function resetCreateTaskDraft(): void {
  Object.assign(createTaskDraft, createDefaultCreateTaskForm());
  createTaskUiState.isDirty = false;
  createTaskUiState.globalError = null;
}

export function resetCreateTaskInputs(): void {
  createTaskInputState.files = [];
}
