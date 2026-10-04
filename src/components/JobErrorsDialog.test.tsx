import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

import i18n from "@/i18n/config";
import type { AppError, ItemSnapshot, JobDetailSnapshot, JobSnapshot } from "@/lib/ipc";
import { JobErrorsContent } from "./JobErrorsDialog";
import { loadJobErrors, type JobErrorProgress } from "./job-errors";

const mocks = vi.hoisted(() => ({ getJobSnapshot: vi.fn() }));
vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/lib/ipc")>(),
  backendClient: { getJobSnapshot: mocks.getJobSnapshot },
}));

const ERROR: AppError = {
  code: "input.decode_failed",
  category: "input",
  messageKey: "errors.decodeFailed",
  messageArgs: {},
  fallbackMessage: "Original codec diagnostic",
  retryable: false,
  fieldErrors: [],
  context: { jobId: "job-1", itemId: "item-1", stage: "decode", path: null },
  diagnosticId: "diagnostic-1",
};

function item(index: number, failed = false): ItemSnapshot {
  return {
    id: `item-${index}`, jobId: "job-1", revision: 3, sequence: index, attempt: 1,
    inputPath: `C:/images/file-${index}.png`, outputPath: `C:/images/file-${index}.jpg`,
    status: failed ? "failed" : "succeeded", queueReason: null, stage: "decode",
    progress: null, workerSlotId: null, createdAt: 1, startedAt: 2, finishedAt: 3,
    controls: { canCancel: false, canRetry: false }, result: null,
    error: failed ? ERROR : null, warnings: [],
  };
}

function page(offset: number, items: ItemSnapshot[], total: number): JobDetailSnapshot {
  return {
    schemaVersion: 2,
    revision: 3,
    job: { id: "job-1", error: null } as JobSnapshot,
    items: { offset, limit: 200, total, items },
  };
}

function deferred<Value>() {
  let resolve!: (value: Value) => void;
  const promise = new Promise<Value>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe("job error loading", () => {
  beforeEach(() => vi.resetAllMocks());

  it("scans successive 200-item pages and finds failures beyond the first and maximum page sizes", async () => {
    const progress: JobErrorProgress[] = [];
    mocks.getJobSnapshot.mockImplementation(async (_jobId, offset) => page(
      offset,
      Array.from({ length: Math.min(200, 1_001 - offset) }, (_, index) => item(offset + index, offset + index === 1_000)),
      1_001,
    ));
    await loadJobErrors("job-1", (value) => progress.push(value), new AbortController().signal);

    expect(mocks.getJobSnapshot.mock.calls).toEqual(
      [0, 200, 400, 600, 800, 1_000].map((offset) => ["job-1", offset, 200]),
    );
    expect(progress.map((value) => value.checkedItems)).toEqual([200, 400, 600, 800, 1_000, 1_001]);
    expect(progress[0].items).toEqual([]);
    expect(progress[progress.length - 1].items.map((value) => value.id)).toEqual(["item-1000"]);
  });

  it("reports empty details and preserves a job-level error", async () => {
    const detail = page(0, [], 0);
    detail.job.error = ERROR;
    mocks.getJobSnapshot.mockResolvedValue(detail);
    const progress = vi.fn();
    await loadJobErrors("job-1", progress, new AbortController().signal);
    expect(progress).toHaveBeenCalledExactlyOnceWith({
      items: [], checkedItems: 0, totalItems: 0, jobError: ERROR,
    });
  });

  it("keeps already loaded failures when a later query fails", async () => {
    const missingDetail = { ...item(0, true), error: null };
    const failure = new Error("connection lost");
    mocks.getJobSnapshot.mockResolvedValueOnce(page(0, [missingDetail], 2)).mockRejectedValueOnce(failure);
    const progress = vi.fn();
    await expect(loadJobErrors("job-1", progress, new AbortController().signal)).rejects.toBe(failure);
    expect(progress).toHaveBeenCalledExactlyOnceWith({
      items: [missingDetail], checkedItems: 1, totalItems: 2, jobError: null,
    });
  });

  it("ignores a late response after close and does not request another page", async () => {
    const pending = deferred<JobDetailSnapshot>();
    mocks.getJobSnapshot.mockReturnValue(pending.promise);
    const progress = vi.fn();
    const controller = new AbortController();
    const loading = loadJobErrors("job-1", progress, controller.signal);
    controller.abort();
    pending.resolve(page(0, [item(0, true)], 2));
    await loading;
    expect(progress).not.toHaveBeenCalled();
    expect(mocks.getJobSnapshot).toHaveBeenCalledOnce();
  });

  it("does not overwrite a refreshed or switched request with an older response", async () => {
    const oldPage = deferred<JobDetailSnapshot>();
    mocks.getJobSnapshot.mockReturnValueOnce(oldPage.promise).mockResolvedValueOnce(page(0, [item(2, true)], 1));
    const oldProgress = vi.fn();
    const currentProgress = vi.fn();
    const oldController = new AbortController();
    const oldLoading = loadJobErrors("job-1", oldProgress, oldController.signal);
    oldController.abort();
    await loadJobErrors("job-1", currentProgress, new AbortController().signal);
    oldPage.resolve(page(0, [item(0, true)], 1));
    await oldLoading;
    expect(oldProgress).not.toHaveBeenCalled();
    expect(currentProgress.mock.calls[0][0].items.map((value: ItemSnapshot) => value.id)).toEqual(["item-2"]);
  });

  it("rejects incompatible or incomplete responses without reporting false completion", async () => {
    for (const detail of [
      { ...page(0, [], 0), schemaVersion: 99 },
      page(0, [], 3),
      page(200, [item(0)], 3),
    ]) {
      mocks.getJobSnapshot.mockResolvedValueOnce(detail);
      const progress = vi.fn();
      await expect(loadJobErrors("job-1", progress, new AbortController().signal)).rejects.toHaveProperty("messageKey");
      expect(progress).not.toHaveBeenCalled();
    }
  });
});

describe("job error content", () => {
  beforeEach(async () => { await i18n.changeLanguage("en"); });

  it.each(["en", "zh", "ja"])("localizes errors and exposes structured diagnostics and full paths in %s", async (language) => {
    await i18n.changeLanguage(language);
    const failed = item(0, true);
    failed.inputPath = `C:/很长的目录/${"nested/".repeat(30)}damaged.png`;
    const markup = renderToStaticMarkup(<JobErrorsContent
      progress={{ items: [failed], checkedItems: 1, totalItems: 1, jobError: null }}
      loading={false} error={null}
    />);
    expect(markup).toContain(i18n.t("errors.decodeFailed"));
    expect(markup).toContain(failed.inputPath);
    expect(markup).toContain("damaged.png");
    expect(markup).toContain("input.decode_failed");
    expect(markup).toContain("Original codec diagnostic");
    expect(markup).toContain("diagnostic-1");
    expect(markup).toContain("<details");
    expect(markup).toContain("select-text whitespace-pre-wrap break-all");
    expect(markup).toContain(i18n.t("jobErrorsScanProgress", { checked: 1, total: 1 }));
  });

  it("keeps partial details visible with a reload hint instead of claiming completion", () => {
    const markup = renderToStaticMarkup(<JobErrorsContent
      progress={{ items: [item(0, true)], checkedItems: 200, totalItems: 201, jobError: null }}
      loading={false} error={new Error("connection lost")}
    />);
    expect(markup).toContain('role="alert"');
    expect(markup).toContain("connection lost");
    expect(markup).toContain(i18n.t("jobErrorsPartialHint"));
    expect(markup).toContain("file-0.png");
    expect(markup).not.toContain(i18n.t("jobErrorsEmpty"));
  });

  it("handles missing errors and unknown backend translations", () => {
    const missing = { ...item(0, true), error: null };
    const unknown = { ...item(1, true), error: { ...ERROR, messageKey: "errors.futureCodec" } };
    const markup = renderToStaticMarkup(<JobErrorsContent
      progress={{ items: [missing, unknown], checkedItems: 2, totalItems: 2, jobError: null }}
      loading={false} error={null}
    />);
    expect(markup).toContain(i18n.t("jobErrorsMissingDetail"));
    expect(markup).toContain("Original codec diagnostic");
    expect(markup).not.toContain("errors.futureCodec");
  });

  it("distinguishes initial loading from an empty completed result", () => {
    expect(renderToStaticMarkup(<JobErrorsContent progress={null} loading error={null} />))
      .toContain(i18n.t("jobErrorsLoading"));
    const markup = renderToStaticMarkup(<JobErrorsContent
      progress={{ items: [], checkedItems: 0, totalItems: 0, jobError: null }}
      loading={false} error={null}
    />);
    expect(markup).toContain(i18n.t("jobErrorsEmpty"));
    expect(markup).not.toContain(i18n.t("jobErrorsLoading"));
  });
});
