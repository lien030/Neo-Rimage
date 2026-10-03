import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ scanInputs: vi.fn() }));

vi.mock("@/lib/ipc", () => ({
  backendClient: { scanInputs: mocks.scanInputs },
  generateCorrelationId: () => "scan-test",
  IPC_SCHEMA_VERSION: 2,
}));

import { collectImageInputs, mergeSelectedInputs } from "./input-files";

describe("input discovery", () => {
  beforeEach(() => vi.resetAllMocks());

  it("uses one versioned backend request for files and directories", async () => {
    const inputs = [{ path: "C:/images/photo.ppm", fileName: "photo.ppm" }];
    mocks.scanInputs.mockResolvedValue({ schemaVersion: 2, inputs, rejectedInputs: [] });

    await expect(collectImageInputs(["C:/images", "C:/images/photo.ppm"], true)).resolves.toEqual(inputs);
    expect(mocks.scanInputs).toHaveBeenCalledOnce();
    expect(mocks.scanInputs).toHaveBeenCalledWith({
      schemaVersion: 2,
      correlationId: "scan-test",
      paths: ["C:/images", "C:/images/photo.ppm"],
      scanRecursively: true,
    });
  });

  it("defaults to a non-recursive scan and preserves backend ordering", async () => {
    const inputs = [{ path: "/images/z.png", fileName: "z.png" }, { path: "/images/a.png", fileName: "a.png" }];
    mocks.scanInputs.mockResolvedValue({ schemaVersion: 2, inputs, rejectedInputs: [] });
    await expect(collectImageInputs(["/images"])).resolves.toEqual(inputs);
    expect(mocks.scanInputs.mock.calls[0][0].scanRecursively).toBe(false);
  });

  it("rejects incompatible scan responses", async () => {
    mocks.scanInputs.mockResolvedValue({ schemaVersion: 3, inputs: [], rejectedInputs: [] });
    await expect(collectImageInputs([])).rejects.toThrow("unsupported schema version 3");
  });

  it("merges canonical paths without duplicates or reordering existing inputs", () => {
    const first = { path: "/images/first.ppm", fileName: "first.ppm" };
    const second = { path: "/images/second.ppm", fileName: "second.ppm" };
    expect(mergeSelectedInputs([first], [first, second, second])).toEqual([first, second]);
  });
});

describe("mergeSelectedInputs", () => {
  it("preserves order while removing paths already selected", () => {
    const selected = [{ path: "a.png", fileName: "a.png" }];
    const incoming = [
      { path: "a.png", fileName: "duplicate.png" },
      { path: "b.png", fileName: "b.png" },
    ];

    expect(mergeSelectedInputs(selected, incoming)).toEqual([
      { path: "a.png", fileName: "a.png" },
      { path: "b.png", fileName: "b.png" },
    ]);
  });

  it("deduplicates repeated paths within the incoming list", () => {
    const incoming = [
      { path: "a.png", fileName: "a.png" },
      { path: "a.png", fileName: "duplicate.png" },
    ];

    expect(mergeSelectedInputs([], incoming)).toEqual([
      { path: "a.png", fileName: "a.png" },
    ]);
  });
});
