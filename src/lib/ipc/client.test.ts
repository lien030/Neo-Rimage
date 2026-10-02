import { describe, expect, it } from "vitest";

import {
  BACKEND_COMMANDS,
  BACKEND_STATE_EVENT,
  BackendClient,
  createJobCommand,
  type IpcTransport,
} from "./client";
import {
  IPC_SCHEMA_VERSION,
  type CreateJobRequest,
  type JobCommand,
  type RetryItemsCommand,
  type SetSchedulerPausedCommand,
  type SetWorkerCountCommand,
  type StateEventEnvelope,
} from "./contracts";

interface Invocation {
  command: string;
  args: Record<string, unknown> | undefined;
}

class RecordingTransport implements IpcTransport {
  readonly invocations: Invocation[] = [];
  listenedEvent: string | null = null;
  private eventHandler: ((payload: unknown) => void) | null = null;

  invoke<T>(
    command: string,
    args?: Record<string, unknown>,
  ): Promise<T> {
    this.invocations.push({ command, args });
    return Promise.resolve({} as T);
  }

  listen<T>(
    event: string,
    handler: (payload: T) => void,
  ): Promise<() => void> {
    this.listenedEvent = event;
    this.eventHandler = (payload) => handler(payload as T);
    return Promise.resolve(() => {
      this.eventHandler = null;
    });
  }

  emit(payload: unknown): void {
    this.eventHandler?.(payload);
  }
}

const CREATE_JOB_REQUEST: CreateJobRequest = {
  schemaVersion: IPC_SCHEMA_VERSION,
  inputs: [],
  operations: [],
  encoder: { kind: "png" },
  output: {
    location: { kind: "same_directory" },
    preserveStructure: false,
    suffix: "-optimized",
    collision: "fail",
    sourceBackup: "disabled",
    existingOutputBackup: "disabled",
  },
  metadata: {
    embedded: "preserve_when_supported",
    colorProfile: "preserve_when_supported",
    report: { kind: "disabled" },
  },
  inputAcceptance: "reject_all",
  scheduling: null,
};

describe("BackendClient", () => {
  it("keeps input scan arguments in the versioned command envelope", async () => {
    const transport = new RecordingTransport();
    const client = new BackendClient(transport);
    const command = {
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: "scan-test",
      paths: ["/images"],
      scanRecursively: true,
    };
    await client.scanInputs(command);
    expect(transport.invocations).toEqual([{ command: BACKEND_COMMANDS.scanInputs, args: { command } }]);
  });

  it("uses the exact read command names and argument casing", async () => {
    const transport = new RecordingTransport();
    const client = new BackendClient(transport);

    await client.getCapabilities();
    await client.getSnapshot();
    await client.getJobSnapshot("job-1");

    expect(transport.invocations).toEqual([
      { command: BACKEND_COMMANDS.getCapabilities, args: undefined },
      { command: BACKEND_COMMANDS.getSnapshot, args: undefined },
      { command: BACKEND_COMMANDS.getJobSnapshot, args: { jobId: "job-1" } },
    ]);
  });

  it("keeps command DTOs nested under the command argument", async () => {
    const transport = new RecordingTransport();
    const client = new BackendClient(transport);
    const jobCommand: JobCommand = {
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: "correlation-1",
      jobId: "job-1",
    };
    const retryCommand: RetryItemsCommand = {
      ...jobCommand,
      itemIds: ["item-1"],
      includeCancelled: true,
    };
    const schedulerCommand: SetSchedulerPausedCommand = {
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: "correlation-2",
      paused: true,
    };
    const workerCommand: SetWorkerCountCommand = {
      schemaVersion: IPC_SCHEMA_VERSION,
      correlationId: "correlation-3",
      desiredConcurrency: 4,
    };
    const createCommand = createJobCommand(
      "correlation-create",
      CREATE_JOB_REQUEST,
    );

    await client.createJob(createCommand);
    await client.pauseJob(jobCommand);
    await client.resumeJob(jobCommand);
    await client.cancelJob(jobCommand);
    await client.retryJobItems(retryCommand);
    await client.removeJob(jobCommand);
    await client.setSchedulerPaused(schedulerCommand);
    await client.setWorkerCount(workerCommand);

    expect(transport.invocations).toEqual([
      { command: BACKEND_COMMANDS.createJob, args: { command: createCommand } },
      { command: BACKEND_COMMANDS.pauseJob, args: { command: jobCommand } },
      { command: BACKEND_COMMANDS.resumeJob, args: { command: jobCommand } },
      { command: BACKEND_COMMANDS.cancelJob, args: { command: jobCommand } },
      {
        command: BACKEND_COMMANDS.retryJobItems,
        args: { command: retryCommand },
      },
      { command: BACKEND_COMMANDS.removeJob, args: { command: jobCommand } },
      {
        command: BACKEND_COMMANDS.setSchedulerPaused,
        args: { command: schedulerCommand },
      },
      {
        command: BACKEND_COMMANDS.setWorkerCount,
        args: { command: workerCommand },
      },
    ]);
  });

  it("subscribes to the backend state channel without reshaping payloads", async () => {
    const transport = new RecordingTransport();
    const client = new BackendClient(transport);
    let received: StateEventEnvelope | null = null;
    const event = { revision: 3 } as StateEventEnvelope;

    await client.listenToStateEvents((payload) => {
      received = payload;
    });
    transport.emit(event);

    expect(transport.listenedEvent).toBe(BACKEND_STATE_EVENT);
    expect(received).toBe(event);
  });
});

describe("createJobCommand", () => {
  it("normalizes the nested request schema version", () => {
    const staleRequest = { ...CREATE_JOB_REQUEST, schemaVersion: 0 };

    expect(createJobCommand("correlation-1", staleRequest)).toEqual({
      correlationId: "correlation-1",
      request: { ...CREATE_JOB_REQUEST, schemaVersion: IPC_SCHEMA_VERSION },
    });
  });
});
