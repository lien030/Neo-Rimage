import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BackendSnapshot } from "@/lib/ipc/contracts";

const mocks = vi.hoisted(() => ({
  backendClient: {
    setSchedulerPaused: vi.fn(),
    setWorkerCount: vi.fn(),
  },
  backendCommandState: {
    schedulerPending: false,
    workerCountPending: false,
  },
  generateCorrelationId: vi.fn(() => "correlation-test"),
  refreshBackendSnapshot: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  backendClient: mocks.backendClient,
  generateCorrelationId: mocks.generateCorrelationId,
  IPC_SCHEMA_VERSION: 2,
}));

vi.mock("./runtime", () => ({
  backendCommandState: mocks.backendCommandState,
  refreshBackendSnapshot: mocks.refreshBackendSnapshot,
}));

import { setSchedulerPaused, setWorkerCount } from "./actions";

const SNAPSHOT = { revision: 7 } as BackendSnapshot;

describe("backend actions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.backendCommandState.schedulerPending = false;
    mocks.backendCommandState.workerCountPending = false;
    mocks.refreshBackendSnapshot.mockResolvedValue(SNAPSHOT);
  });

  it("refreshes through the acknowledged scheduler revision", async () => {
    mocks.backendClient.setSchedulerPaused.mockResolvedValue({ revision: 7 });

    await expect(setSchedulerPaused(true)).resolves.toBe(SNAPSHOT);

    expect(mocks.generateCorrelationId).toHaveBeenCalledWith("request");
    expect(mocks.backendClient.setSchedulerPaused).toHaveBeenCalledWith({
      schemaVersion: 2,
      correlationId: "correlation-test",
      paused: true,
    });
    expect(mocks.refreshBackendSnapshot).toHaveBeenCalledWith(7);
    expect(mocks.backendCommandState.schedulerPending).toBe(false);
  });

  it("refreshes through the acknowledged worker-count revision", async () => {
    mocks.backendClient.setWorkerCount.mockResolvedValue({ revision: 9 });

    await expect(setWorkerCount(4)).resolves.toBe(SNAPSHOT);

    expect(mocks.backendClient.setWorkerCount).toHaveBeenCalledWith({
      schemaVersion: 2,
      correlationId: "correlation-test",
      desiredConcurrency: 4,
    });
    expect(mocks.refreshBackendSnapshot).toHaveBeenCalledWith(9);
    expect(mocks.backendCommandState.workerCountPending).toBe(false);
  });

  it("coalesces a duplicate command into a regular snapshot refresh", async () => {
    mocks.backendCommandState.schedulerPending = true;

    await expect(setSchedulerPaused(false)).resolves.toBe(SNAPSHOT);

    expect(mocks.backendClient.setSchedulerPaused).not.toHaveBeenCalled();
    expect(mocks.refreshBackendSnapshot).toHaveBeenCalledWith();
  });

  it("always clears pending state when a command fails", async () => {
    const failure = new Error("backend failed");
    mocks.backendClient.setWorkerCount.mockRejectedValue(failure);

    await expect(setWorkerCount(2)).rejects.toBe(failure);

    expect(mocks.refreshBackendSnapshot).not.toHaveBeenCalled();
    expect(mocks.backendCommandState.workerCountPending).toBe(false);
  });
});
