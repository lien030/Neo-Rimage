export type CreateTaskSection = "inputs" | "encoder" | "operations" | "output";

/** Local presentation state; it contains no backend lifecycle facts. */
export interface CreateTaskUiState {
  isOpen: boolean;
  activeSection: CreateTaskSection;
  isSubmitting: boolean;
  isDirty: boolean;
  globalError: string | null;
}
