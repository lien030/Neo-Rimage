import {
  backendClient,
  IPC_SCHEMA_VERSION,
  type BackendSnapshot,
} from "@/lib/ipc";
import {
  backendCommandState,
  refreshBackendSnapshot,
} from "./runtime";

function correlationId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `request-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export async function setSchedulerPaused(
  paused: boolean,
): Promise<BackendSnapshot> {
  if (backendCommandState.schedulerPending) {
    return refreshBackendSnapshot();
  }

  backendCommandState.schedulerPending = true;
  try {
    await backendClient.setSchedulerPaused({
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: correlationId(),
      paused,
    });
    return await refreshBackendSnapshot();
  } finally {
    backendCommandState.schedulerPending = false;
  }
}

export async function setWorkerCount(
  desiredConcurrency: number,
): Promise<BackendSnapshot> {
  if (backendCommandState.workerCountPending) {
    return refreshBackendSnapshot();
  }

  backendCommandState.workerCountPending = true;
  try {
    await backendClient.setWorkerCount({
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: correlationId(),
      desiredConcurrency,
    });
    return await refreshBackendSnapshot();
  } finally {
    backendCommandState.workerCountPending = false;
  }
}
