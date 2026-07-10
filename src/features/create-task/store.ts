import { proxy } from "valtio";

import type {
  CreateTaskFormValues,
  CreateTaskUiState,
} from "./domain";

export function createDefaultCreateTaskForm(): CreateTaskFormValues {
  return {
    inputs: [],
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
    avif: {
      quality: "50",
      alphaQuality: "",
      speed: "6",
      colorSpace: "ycbcr",
      alphaMode: "unassociated_clean",
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

export function resetCreateTaskDraft(): void {
  Object.assign(createTaskDraft, createDefaultCreateTaskForm());
  createTaskUiState.isDirty = false;
  createTaskUiState.globalError = null;
}
