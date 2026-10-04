import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BackendSnapshot, JobSnapshot, JobStatus } from "@/lib/ipc/contracts";

const mocks = vi.hoisted(() => ({
  backendClient: {
    setSchedulerPaused: vi.fn(),
    setWorkerCount: vi.fn(),
    removeJob: vi.fn(),
  },
  backendCommandState: {
    schedulerPending: false,
    workerCountPending: false,
    cleanupPending: false,
  },
  backendRuntimeState: { snapshot: null as BackendSnapshot | null },
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
  backendRuntimeState: mocks.backendRuntimeState,
  refreshBackendSnapshot: mocks.refreshBackendSnapshot,
}));

import { cleanupJobs, selectCleanupJobIds, setSchedulerPaused, setWorkerCount } from "./actions";

const SNAPSHOT = { revision: 7 } as BackendSnapshot;

describe("backend actions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.backendCommandState.schedulerPending = false;
    mocks.backendCommandState.workerCountPending = false;
    mocks.backendCommandState.cleanupPending = false;
    mocks.backendRuntimeState.snapshot = null;
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

function job(id: string, status: JobStatus, canRemove = true): JobSnapshot {
  return { id, status, controls: { canRemove } } as JobSnapshot;
}

describe("task cleanup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.backendCommandState.cleanupPending = false;
    mocks.backendRuntimeState.snapshot = null;
    mocks.refreshBackendSnapshot.mockResolvedValue(SNAPSHOT);
  });

  it("selects only removable terminal jobs for each menu group", () => {
    const jobs = [
      job("success", "succeeded"), job("partial", "partially_succeeded"),
      job("failed", "failed"), job("cancelled", "cancelled"),
      job("running", "running"), job("queued", "queued"), job("paused", "paused"),
      job("cancelling", "cancelling"), job("blocked", "failed", false),
    ];
    expect(selectCleanupJobIds(jobs, "succeeded")).toEqual(["success"]);
    expect(selectCleanupJobIds(jobs, "unsuccessful")).toEqual(["partial", "failed", "cancelled"]);
    expect(selectCleanupJobIds(jobs, "all")).toEqual(["success", "partial", "failed", "cancelled"]);
  });

  it("continues after individual failures and refreshes through the highest acknowledgement", async () => {
    const failure = new Error("transport failed");
    mocks.backendClient.removeJob
      .mockResolvedValueOnce({ revision: 9 })
      .mockRejectedValueOnce({ error: { code: "job.not_found" } })
      .mockRejectedValueOnce({ error: { code: "job.remove_not_allowed" } })
      .mockRejectedValueOnce(failure)
      .mockResolvedValueOnce({ revision: 12 });

    const result = await cleanupJobs(["a", "b", "c", "d", "e"]);

    expect(result).toEqual({
      completedIds: ["a", "b", "e"], skippedIds: ["c"],
      failures: [{ jobId: "d", error: failure }], minimumRevision: 12, refreshError: null,
    });
    expect(mocks.backendClient.removeJob.mock.calls.map(([command]) => command.jobId)).toEqual(["a", "b", "c", "d", "e"]);
    expect(mocks.backendClient.removeJob).toHaveBeenNthCalledWith(1, {
      schemaVersion: 2, correlationId: "correlation-test", jobId: "a",
    });
    expect(mocks.refreshBackendSnapshot).toHaveBeenCalledWith(12);
    expect(mocks.backendCommandState.cleanupPending).toBe(false);
  });

  it("serializes removals, captures the batch, and coalesces repeated clicks", async () => {
    let acknowledge!: (value: { revision: number }) => void;
    mocks.backendClient.removeJob.mockReturnValueOnce(new Promise((resolve) => { acknowledge = resolve; }))
      .mockResolvedValueOnce({ revision: 8 });
    const ids = ["a", "b", "a"];
    const request = cleanupJobs(ids);
    ids.push("late");
    expect(cleanupJobs(["late"])).toBe(request);
    expect(mocks.backendCommandState.cleanupPending).toBe(true);
    expect(mocks.backendClient.removeJob).toHaveBeenCalledTimes(1);
    acknowledge({ revision: 7 });
    expect((await request).completedIds).toEqual(["a", "b"]);
    expect(mocks.backendClient.removeJob).toHaveBeenCalledTimes(2);
  });

  it("rechecks current availability without optimistically deleting or adding records", async () => {
    const jobs = [job("a", "succeeded"), job("b", "failed"), job("late", "succeeded")];
    mocks.backendRuntimeState.snapshot = { jobs } as BackendSnapshot;
    mocks.backendClient.removeJob.mockImplementationOnce(async () => {
      jobs[1] = job("b", "running", false);
      return { revision: 10 };
    });
    const result = await cleanupJobs(["a", "b", "missing"]);
    expect(result.completedIds).toEqual(["a", "missing"]);
    expect(result.skippedIds).toEqual(["b"]);
    expect(mocks.backendClient.removeJob).toHaveBeenCalledTimes(1);
    expect(jobs.map(({ id }) => id)).toEqual(["a", "b", "late"]);
  });

  it("preserves removal results on refresh failure and releases pending state for retry", async () => {
    const failure = new Error("offline");
    mocks.backendClient.removeJob.mockResolvedValueOnce({ revision: 20 });
    mocks.refreshBackendSnapshot.mockRejectedValueOnce(failure);
    const result = await cleanupJobs(["a"]);
    expect(result.completedIds).toEqual(["a"]);
    expect(result.refreshError).toBe(failure);
    expect(result.minimumRevision).toBe(20);
    expect(mocks.backendCommandState.cleanupPending).toBe(false);
    mocks.backendClient.removeJob.mockResolvedValueOnce({ revision: 21 });
    expect((await cleanupJobs(["b"])).completedIds).toEqual(["b"]);
  });

  it("retries only failed IDs after a partial result", async () => {
    mocks.backendClient.removeJob.mockResolvedValueOnce({ revision: 1 })
      .mockRejectedValueOnce("temporary failure")
      .mockResolvedValueOnce({ revision: 2 });
    const result = await cleanupJobs(["a", "b"]);
    const retried = await cleanupJobs(result.failures.map(({ jobId }) => jobId));
    expect(retried.completedIds).toEqual(["b"]);
    expect(mocks.backendClient.removeJob.mock.calls.map(([command]) => command.jobId)).toEqual(["a", "b", "b"]);
  });
});
