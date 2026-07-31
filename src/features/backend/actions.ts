import {
  backendClient,
  generateCorrelationId,
  IPC_SCHEMA_VERSION,
  type BackendSnapshot,
  type Revision,
} from "@/lib/ipc";
import {
  backendCommandState,
  refreshBackendSnapshot,
} from "./runtime";

type PendingCommand = "schedulerPending" | "workerCountPending";

interface RevisionedCommandResponse {
  readonly revision: Revision;
}

async function runBackendCommand(
  pendingCommand: PendingCommand,
  send: () => Promise<RevisionedCommandResponse>,
): Promise<BackendSnapshot> {
  if (backendCommandState[pendingCommand]) {
    return refreshBackendSnapshot();
  }

  backendCommandState[pendingCommand] = true;
  try {
    const response = await send();

    // The acknowledgement may arrive before the corresponding snapshot is
    // observable, so wait for at least the acknowledged backend revision.
    return await refreshBackendSnapshot(response.revision);
  } finally {
    backendCommandState[pendingCommand] = false;
  }
}

export async function setSchedulerPaused(
  paused: boolean,
): Promise<BackendSnapshot> {
  return runBackendCommand("schedulerPending", () =>
    backendClient.setSchedulerPaused({
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: generateCorrelationId("request"),
      paused,
    }),
  );
}

export async function setWorkerCount(
  desiredConcurrency: number,
): Promise<BackendSnapshot> {
  return runBackendCommand("workerCountPending", () =>
    backendClient.setWorkerCount({
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: generateCorrelationId("request"),
      desiredConcurrency,
    }),
  );
}
