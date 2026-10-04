import {
  backendClient,
  DEFAULT_ITEM_PAGE_SIZE,
  IPC_SCHEMA_VERSION,
  type AppError,
  type ItemSnapshot,
} from "@/lib/ipc";

export interface JobErrorProgress {
  items: ItemSnapshot[];
  checkedItems: number;
  totalItems: number;
  jobError: AppError | null;
}

/** The detail API pages all inputs, so errors can occur after successful pages. */
export async function loadJobErrors(
  jobId: string,
  onProgress: (progress: JobErrorProgress) => void,
  signal: AbortSignal,
): Promise<void> {
  let offset = 0;
  const failures: ItemSnapshot[] = [];

  while (!signal.aborted) {
    const detail = await backendClient.getJobSnapshot(
      jobId,
      offset,
      DEFAULT_ITEM_PAGE_SIZE,
    );
    // Tauri queries cannot be cancelled in flight. Ignore their late response.
    if (signal.aborted) return;
    if (detail.schemaVersion !== IPC_SCHEMA_VERSION) {
      throw { messageKey: "errors.schemaVersionUnsupported" };
    }
    const page = detail.items;
    if (
      detail.job.id !== jobId ||
      page.offset !== offset ||
      (page.items.length === 0 && offset < page.total)
    ) {
      throw { messageKey: "jobErrorsPageIncomplete" };
    }

    failures.push(
      ...page.items.filter((item) => item.status === "failed" || item.error !== null),
    );
    offset += page.items.length;
    onProgress({
      items: [...failures],
      checkedItems: offset,
      totalItems: page.total,
      jobError: detail.job.error,
    });
    if (offset >= page.total) return;
  }
}
