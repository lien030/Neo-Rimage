/** Local presentation state; it contains no backend lifecycle facts. */
export interface CreateTaskUiState {
  isOpen: boolean;
  isSubmitting: boolean;
  isDirty: boolean;
  globalError: string | null;
}
