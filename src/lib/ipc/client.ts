import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  IPC_SCHEMA_VERSION,
  type BackendCapabilities,
  type BackendSnapshot,
  type CommandAccepted,
  type CreateJobCommand,
  type CreateJobResponse,
  type JobCommand,
  type JobDetailSnapshot,
  type JobId,
  type JobSnapshot,
  type RetryItemsCommand,
  type ScanInputsCommand,
  type ScanInputsResponse,
  type SchedulerSnapshot,
  type SetSchedulerPausedCommand,
  type SetWorkerCountCommand,
  type StateEventEnvelope,
  type WorkerSlotSnapshot,
} from "./contracts";

export const BACKEND_STATE_EVENT = "backend://state-delta" as const;
export const BACKEND_NOTICE_EVENT = "backend://notice" as const;

export const BACKEND_COMMANDS = {
  getCapabilities: "get_backend_capabilities",
  getSnapshot: "get_backend_snapshot",
  getJobSnapshot: "get_job_snapshot",
  createJob: "create_job",
  scanInputs: "scan_inputs",
  pauseJob: "pause_job",
  resumeJob: "resume_job",
  cancelJob: "cancel_job",
  retryJobItems: "retry_job_items",
  removeJob: "remove_job",
  setSchedulerPaused: "set_scheduler_paused",
  setWorkerCount: "set_worker_count",
} as const;

export interface IpcTransport {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(
    event: string,
    handler: (payload: T) => void,
  ): Promise<UnlistenFn>;
}

const tauriTransport: IpcTransport = {
  invoke: <T>(command: string, args?: Record<string, unknown>) =>
    invoke<T>(command, args),
  listen: <T>(event: string, handler: (payload: T) => void) =>
    listen<T>(event, ({ payload }) => handler(payload)),
};

export class BackendClient {
  constructor(private readonly transport: IpcTransport = tauriTransport) {}

  getCapabilities(): Promise<BackendCapabilities> {
    return this.transport.invoke(BACKEND_COMMANDS.getCapabilities);
  }

  getSnapshot(): Promise<BackendSnapshot> {
    return this.transport.invoke(BACKEND_COMMANDS.getSnapshot);
  }

  getJobSnapshot(jobId: JobId): Promise<JobDetailSnapshot> {
    return this.transport.invoke(BACKEND_COMMANDS.getJobSnapshot, { jobId });
  }

  createJob(command: CreateJobCommand): Promise<CreateJobResponse> {
    return this.transport.invoke(BACKEND_COMMANDS.createJob, { command });
  }

  scanInputs(command: ScanInputsCommand): Promise<ScanInputsResponse> {
    return this.transport.invoke(BACKEND_COMMANDS.scanInputs, { command });
  }

  pauseJob(command: JobCommand): Promise<CommandAccepted<JobSnapshot>> {
    return this.transport.invoke(BACKEND_COMMANDS.pauseJob, { command });
  }

  resumeJob(command: JobCommand): Promise<CommandAccepted<JobSnapshot>> {
    return this.transport.invoke(BACKEND_COMMANDS.resumeJob, { command });
  }

  cancelJob(command: JobCommand): Promise<CommandAccepted<JobSnapshot>> {
    return this.transport.invoke(BACKEND_COMMANDS.cancelJob, { command });
  }

  retryJobItems(
    command: RetryItemsCommand,
  ): Promise<CommandAccepted<JobSnapshot>> {
    return this.transport.invoke(BACKEND_COMMANDS.retryJobItems, { command });
  }

  removeJob(command: JobCommand): Promise<CommandAccepted<JobSnapshot>> {
    return this.transport.invoke(BACKEND_COMMANDS.removeJob, { command });
  }

  setSchedulerPaused(
    command: SetSchedulerPausedCommand,
  ): Promise<CommandAccepted<SchedulerSnapshot>> {
    return this.transport.invoke(BACKEND_COMMANDS.setSchedulerPaused, {
      command,
    });
  }

  setWorkerCount(
    command: SetWorkerCountCommand,
  ): Promise<CommandAccepted<WorkerSlotSnapshot[]>> {
    return this.transport.invoke(BACKEND_COMMANDS.setWorkerCount, { command });
  }

  listenToStateEvents(
    handler: (event: StateEventEnvelope) => void,
  ): Promise<UnlistenFn> {
    return this.transport.listen(BACKEND_STATE_EVENT, handler);
  }
}

export function createJobCommand(
  correlationId: string,
  request: CreateJobCommand["request"],
): CreateJobCommand {
  return {
    correlationId,
    // Normalize the version at the final IPC boundary so callers cannot send
    // a stale version copied from long-lived editable state.
    request: { ...request, schemaVersion: IPC_SCHEMA_VERSION },
  };
}

export const backendClient = new BackendClient();
