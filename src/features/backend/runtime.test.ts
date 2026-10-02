import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  BackendCapabilities,
  BackendSnapshot,
  StateEventEnvelope,
} from "@/lib/ipc/contracts";

const mocks = vi.hoisted(() => ({
  getCapabilities: vi.fn(),
  getSnapshot: vi.fn(),
  listenToStateEvents: vi.fn(),
  unlisten: vi.fn(),
  toastError: vi.fn(),
  useEffect: vi.fn<(effect: () => void | (() => void)) => void>(),
}));

vi.mock("react", async (importOriginal) => ({
  ...await importOriginal<typeof import("react")>(),
  useEffect: mocks.useEffect,
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/lib/ipc")>(),
  backendClient: {
    getCapabilities: mocks.getCapabilities,
    getSnapshot: mocks.getSnapshot,
    listenToStateEvents: mocks.listenToStateEvents,
  },
}));

vi.mock("sonner", () => ({ toast: { error: mocks.toastError } }));

function deferred<Value>() {
  let resolve!: (value: Value) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<Value>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function snapshot(revision: number): BackendSnapshot {
  return {
    schemaVersion: 1,
    revision,
    generatedAt: 1_000,
    scheduler: {
      mode: "paused",
      desiredConcurrency: 1,
      effectiveConcurrency: 0,
      maxConcurrency: 2,
      activeItems: 0,
      queuedItems: 0,
    },
    jobs: [],
    workerSlots: [],
  };
}

function stateEvent(revision: number): StateEventEnvelope {
  return {
    schemaVersion: 1,
    revision,
    occurredAt: 1_000,
    correlationId: null,
    event: { kind: "snapshot_invalidated" },
  };
}

describe("backend runtime", () => {
  let runtime: typeof import("./runtime");
  let cleanup: (() => void) | null;

  beforeEach(async () => {
    vi.resetModules();
    vi.resetAllMocks();
    cleanup = null;
    mocks.getCapabilities.mockResolvedValue({ schemaVersion: 1 });
    mocks.getSnapshot.mockResolvedValue(snapshot(0));
    mocks.listenToStateEvents.mockResolvedValue(mocks.unlisten);
    mocks.useEffect.mockImplementation((effect) => {
      cleanup = effect() ?? null;
    });
    runtime = await import("./runtime");
  });

  it("coalesces refreshes while honoring the highest requested revision", async () => {
    const first = deferred<BackendSnapshot>();
    const second = deferred<BackendSnapshot>();
    mocks.getSnapshot
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    const lowerRequest = runtime.refreshBackendSnapshot(2);
    const higherRequest = runtime.refreshBackendSnapshot(5);
    expect(mocks.getSnapshot).toHaveBeenCalledOnce();

    first.resolve(snapshot(2));
    await vi.waitFor(() => expect(mocks.getSnapshot).toHaveBeenCalledTimes(2));
    second.resolve(snapshot(5));

    await expect(lowerRequest).resolves.toEqual(snapshot(5));
    await expect(higherRequest).resolves.toEqual(snapshot(5));
    expect(runtime.backendRuntimeState.lastAppliedRevision).toBe(5);
    expect(runtime.backendRuntimeState.syncStatus).toBe("ready");
  });

  it("never overwrites an applied snapshot with an older response", async () => {
    runtime.backendRuntimeState.snapshot = snapshot(9);
    runtime.backendRuntimeState.lastAppliedRevision = 9;
    const current = deferred<BackendSnapshot>();
    mocks.getSnapshot
      .mockResolvedValueOnce(snapshot(8))
      .mockReturnValueOnce(current.promise);

    const request = runtime.refreshBackendSnapshot();
    await vi.waitFor(() => expect(mocks.getSnapshot).toHaveBeenCalledTimes(2));
    expect(runtime.backendRuntimeState.snapshot?.revision).toBe(9);
    current.resolve(snapshot(9));
    await request;
  });

  it("clears a failed in-flight request so the next refresh can recover", async () => {
    const failure = new Error("offline");
    mocks.getSnapshot.mockRejectedValueOnce(failure);
    await expect(runtime.refreshBackendSnapshot(3)).rejects.toBe(failure);

    mocks.getSnapshot.mockResolvedValueOnce(snapshot(3));
    await expect(runtime.refreshBackendSnapshot()).resolves.toEqual(snapshot(3));
    expect(mocks.getSnapshot).toHaveBeenCalledTimes(2);
  });

  it("rejects an incompatible snapshot without applying it", async () => {
    mocks.getSnapshot.mockResolvedValueOnce({ ...snapshot(3), schemaVersion: 2 });
    await expect(runtime.refreshBackendSnapshot()).rejects.toThrow("unsupported schema version 2");
    expect(runtime.backendRuntimeState.snapshot).toBeNull();
    expect(runtime.backendRuntimeState.lastAppliedRevision).toBe(0);
  });

  it("coalesces capabilities and allows retry after a schema mismatch", async () => {
    const pending = deferred<BackendCapabilities>();
    mocks.getCapabilities.mockReturnValueOnce(pending.promise);
    const first = runtime.refreshBackendCapabilities();
    const second = runtime.refreshBackendCapabilities();
    const incompatible = { schemaVersion: 2 } as BackendCapabilities;
    const failures = Promise.allSettled([first, second]);
    pending.resolve(incompatible);
    expect((await failures).map((result) => result.status)).toEqual(["rejected", "rejected"]);
    expect(runtime.backendRuntimeState.capabilities).toBeNull();
    expect(mocks.getCapabilities).toHaveBeenCalledOnce();

    await runtime.refreshBackendCapabilities();
    expect(runtime.backendRuntimeState.capabilities?.schemaVersion).toBe(1);
  });

  it("registers the listener before loading the initial authoritative state", async () => {
    const registration = deferred<() => void>();
    mocks.listenToStateEvents.mockReturnValueOnce(registration.promise);
    runtime.useBackendRuntimeSync();
    expect(mocks.getSnapshot).not.toHaveBeenCalled();

    registration.resolve(mocks.unlisten);
    await vi.waitFor(() => expect(runtime.backendRuntimeState.syncStatus).toBe("ready"));
    expect(mocks.getSnapshot).toHaveBeenCalledOnce();
    expect(mocks.getCapabilities).toHaveBeenCalledOnce();
    cleanup?.();
    expect(mocks.unlisten).toHaveBeenCalledOnce();
  });

  it("cleans up a listener that finishes registering after disposal", async () => {
    const registration = deferred<() => void>();
    mocks.listenToStateEvents.mockReturnValueOnce(registration.promise);
    runtime.useBackendRuntimeSync();
    cleanup?.();
    registration.resolve(mocks.unlisten);

    await vi.waitFor(() => expect(mocks.unlisten).toHaveBeenCalledOnce());
    expect(mocks.getSnapshot).not.toHaveBeenCalled();
    expect(mocks.getCapabilities).not.toHaveBeenCalled();
  });

  it("ignores stale dirty hints and recovers a revision gap from a full snapshot", async () => {
    mocks.getSnapshot.mockResolvedValueOnce(snapshot(3));
    runtime.useBackendRuntimeSync();
    await vi.waitFor(() => expect(runtime.backendRuntimeState.lastAppliedRevision).toBe(3));
    const handleEvent = mocks.listenToStateEvents.mock.calls[0][0];
    handleEvent(stateEvent(3));
    handleEvent(stateEvent(2));
    expect(mocks.getSnapshot).toHaveBeenCalledOnce();

    const next = deferred<BackendSnapshot>();
    mocks.getSnapshot.mockReturnValueOnce(next.promise);
    handleEvent(stateEvent(7));
    expect(runtime.backendRuntimeState.syncStatus).toBe("needs_resync");
    next.resolve(snapshot(7));
    await vi.waitFor(() => expect(runtime.backendRuntimeState.lastAppliedRevision).toBe(7));
    expect(runtime.backendRuntimeState.syncStatus).toBe("ready");

    cleanup?.();
    handleEvent(stateEvent(8));
    expect(mocks.getSnapshot).toHaveBeenCalledTimes(2);
  });

  it("reports incompatible events without issuing a snapshot request", async () => {
    runtime.useBackendRuntimeSync();
    await vi.waitFor(() => expect(runtime.backendRuntimeState.syncStatus).toBe("ready"));
    const handleEvent = mocks.listenToStateEvents.mock.calls[0][0];
    handleEvent({ ...stateEvent(1), schemaVersion: 2 });
    await vi.waitFor(() => expect(mocks.toastError).toHaveBeenCalledOnce());
    expect(runtime.backendRuntimeState.syncStatus).toBe("needs_resync");
    expect(mocks.getSnapshot).toHaveBeenCalledOnce();
    cleanup?.();
  });

  it("reports listener registration failure as backend unavailability", async () => {
    mocks.listenToStateEvents.mockRejectedValueOnce(new Error("listener failed"));
    runtime.useBackendRuntimeSync();
    await vi.waitFor(() => expect(runtime.backendRuntimeState.syncStatus).toBe("unavailable"));
    expect(runtime.backendRuntimeState.lastError).toBe("listener failed");
    expect(mocks.toastError).toHaveBeenCalledWith("Backend unavailable", {
      description: "listener failed",
    });
    expect(mocks.getSnapshot).not.toHaveBeenCalled();
    cleanup?.();
  });
});
