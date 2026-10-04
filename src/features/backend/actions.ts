import {
  backendClient,
  generateCorrelationId,
  IPC_SCHEMA_VERSION,
  type BackendSnapshot,
  type JobSnapshot,
  type Revision,
} from "@/lib/ipc";
import {
  backendCommandState,
  backendRuntimeState,
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

export type TaskCleanupGroup = "succeeded" | "unsuccessful" | "all";
type CleanupJob = Pick<JobSnapshot, "id" | "status"> & {
  readonly controls: Pick<JobSnapshot["controls"], "canRemove">;
};

export function isFinishedJob(job: Pick<JobSnapshot, "status">): boolean {
  return ["succeeded", "partially_succeeded", "failed", "cancelled"].includes(job.status);
}

export function selectCleanupJobIds(
  jobs: readonly CleanupJob[],
  group: TaskCleanupGroup,
): string[] {
  return jobs.filter((job) => job.controls.canRemove && isFinishedJob(job) && (
    group === "all" || (group === "succeeded" ? job.status === "succeeded" : job.status !== "succeeded")
  )).map((job) => job.id);
}

export interface TaskCleanupResult {
  completedIds: string[];
  skippedIds: string[];
  failures: { jobId: string; error: unknown }[];
  minimumRevision: Revision;
  refreshError: unknown | null;
}

let cleanupRequest: Promise<TaskCleanupResult> | null = null;

export function cleanupJobs(jobIds: readonly string[]): Promise<TaskCleanupResult> {
  if (cleanupRequest) return cleanupRequest;

  // Capture the confirmed batch: jobs that finish later are never added to it.
  const confirmedIds = [...new Set(jobIds)];
  backendCommandState.cleanupPending = true;
  cleanupRequest = (async () => {
    const result: TaskCleanupResult = {
      completedIds: [], skippedIds: [], failures: [], minimumRevision: 0, refreshError: null,
    };
    for (const jobId of confirmedIds) {
      const jobs = backendRuntimeState.snapshot?.jobs;
      const current = jobs?.find((job) => job.id === jobId);
      if (jobs && !current) {
        result.completedIds.push(jobId);
        continue;
      }
      if (current && (!isFinishedJob(current) || !current.controls.canRemove)) {
        result.skippedIds.push(jobId);
        continue;
      }
      try {
        const response = await backendClient.removeJob({
          schemaVersion: IPC_SCHEMA_VERSION,
          correlationId: generateCorrelationId("request"),
          jobId,
        });
        result.minimumRevision = Math.max(result.minimumRevision, response.revision);
        result.completedIds.push(jobId);
      } catch (error: unknown) {
        const envelope = error as { error?: { code?: string }; code?: string } | null;
        const code = envelope?.error?.code ?? envelope?.code;
        if (code === "job.not_found") result.completedIds.push(jobId);
        else if (code === "job.remove_not_allowed") result.skippedIds.push(jobId);
        else result.failures.push({ jobId, error });
      }
    }
    try {
      // Backend snapshots remain authoritative even after partial or uncertain failures.
      await refreshBackendSnapshot(result.minimumRevision);
    } catch (error: unknown) {
      result.refreshError = error;
    }
    return result;
  })().finally(() => {
    backendCommandState.cleanupPending = false;
    cleanupRequest = null;
  });
  return cleanupRequest;
}
