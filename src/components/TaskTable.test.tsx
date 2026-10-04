import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

import i18n from "@/i18n/config";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { BackendSyncStatus, JobSnapshot, JobStatus } from "@/lib/ipc";

import TaskTable from "./TaskTable";

const mocks = vi.hoisted(() => ({ useBackendRuntimeState: vi.fn() }));

vi.mock("@/features/backend", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/features/backend")>(),
  ...mocks,
}));

function job(status: JobStatus, totalItems = 1, completedItems = 0): JobSnapshot {
  return {
    id: `job-${status}`,
    revision: 1,
    configVersion: 1,
    encoder: "mozjpeg",
    status,
    createdAt: 1_000,
    updatedAt: 1_000,
    counts: {
      total: totalItems, queued: 0, waitingForMemory: 0, running: 0, cancelling: 0,
      succeeded: completedItems, failed: 0, cancelled: 0, skipped: 0,
    },
    progress: { completedItems, totalItems, activeItems: 0 },
    controls: {
      canPause: false, canResume: false, canCancel: false,
      canRetry: false, canRemove: false,
    },
    result: null,
    error: null,
  };
}

function renderTable(
  jobs: JobSnapshot[] | null = [],
  syncStatus: BackendSyncStatus = "ready",
  lastError: string | null = null,
) {
  mocks.useBackendRuntimeState.mockReturnValue({
    snapshot: jobs === null ? null : { jobs }, syncStatus, lastError,
  });
  return renderToStaticMarkup(
    <TooltipProvider>
      <TaskTable />
    </TooltipProvider>,
  );
}

describe("TaskTable", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
  });

  it("keeps the fixed four-column layout and scrollable sticky header", () => {
    const markup = renderTable([job("queued")]);
    const headers = markup.match(/<th\b[^>]*>/g) ?? [];
    const cells = markup.match(/<td\b[^>]*>/g) ?? [];

    expect(headers).toHaveLength(4);
    expect(cells).toHaveLength(4);
    for (const columns of [headers, cells]) {
      expect(columns[0]).toContain("w-8 px-1");
      expect(columns[0]).toContain("width:32px;min-width:32px;max-width:32px");
      expect(columns[1]).toContain("min-width:160px");
      expect(columns[2]).toContain("min-width:130px");
      expect(columns[3]).toContain("min-width:140px");
    }
    expect(markup).toContain("min-width:462px");
    expect(markup).toContain("h-full overflow-auto");
    expect(markup).toContain("sticky top-0 z-10 bg-background drop-shadow");
    expect(markup).toContain("File");
    expect(markup).toContain("Options");
    expect(markup).toContain("Progress");
    expect(markup).toContain("mozjpeg · job-queued");
    expect(markup).toContain("1 item</p>");
  });

  it.each([
    ["ready", null, "No backend jobs."],
    ["syncing", null, "Synchronizing backend jobs…"],
    ["unavailable", "Backend unavailable", "Backend unavailable"],
  ] as const)("renders the %s empty state", (syncStatus, lastError, message) => {
    const markup = renderTable(null, syncStatus, lastError);
    expect(markup).toContain('colSpan="4"');
    expect(markup).toContain(message);
  });

  it.each([
    ["queued", "bg-gray-400", "Queued"],
    ["running", "bg-yellow-400", "Running"],
    ["paused", "bg-blue-400", "Paused"],
    ["cancelling", "bg-orange-400", "Cancelling"],
    ["succeeded", "bg-green-400", "Succeeded"],
    ["partially_succeeded", "bg-amber-500", "Partially Succeeded"],
    ["failed", "bg-red-400", "Failed"],
    ["cancelled", "bg-zinc-500", "Cancelled"],
  ] as const)("renders the %s status", (status, lampClass, label) => {
    const markup = renderTable([job(status)]);
    expect(markup).toContain(`h-2 w-2 rounded-full ${lampClass}`);
    expect(markup).toContain(`>${label}</p>`);
  });

  it("preserves progress and moves backend errors behind a details button", () => {
    const failed = job("failed");
    failed.error = {
      code: "output_failed", category: "output", messageKey: "outputFailed",
      messageArgs: {}, fallbackMessage: "Cannot write output", retryable: false,
      fieldErrors: [], diagnosticId: null,
      context: { jobId: failed.id, itemId: null, stage: null, path: null },
    };
    const markup = renderTable([
      job("queued", 0), job("running", 3, 1), job("succeeded", 1, 2), failed,
    ]);
    expect(markup).toContain(">—</p>");
    expect(markup).toContain("33% · 1/3");
    expect(markup).toContain("100% · 2/1");
    expect(markup).toContain("3 items</p>");
    expect(markup).toContain(i18n.t("viewJobErrorsFor", { jobId: failed.id }));
    expect(markup).toContain(`>${i18n.t("jobStatus.failed")}</button>`);
    expect(markup).not.toContain("Cannot write output");
  });

  it.each([
    ["zh", "进度", "暂无任务。", "排队中", "3 项"],
    ["ja", "進捗", "タスクはありません。", "待機中", "3 件"],
  ])("renders headings, empty states, counts and statuses in %s", async (
    language, progress, empty, status, count,
  ) => {
    await i18n.changeLanguage(language);
    expect(renderTable()).toContain(empty);
    const markup = renderTable([job("queued", 3)]);
    expect(markup).toContain(progress);
    expect(markup).toContain(status);
    expect(markup).toContain(count);
    expect(markup).not.toContain(">Progress</th>");
    expect(markup).not.toContain("3 items</p>");
  });

  it.each(["en", "zh", "ja"])("renders queued memory waits without changing columns in %s", async (language) => {
    await i18n.changeLanguage(language);
    const waiting = job("queued", 3);
    waiting.counts.queued = 3;
    waiting.counts.waitingForMemory = 2;
    const markup = renderTable([waiting]);
    expect(markup).toContain(i18n.t("waitingForMemory", { count: 2 }));
    expect(markup.match(/<td\b[^>]*>/g)).toHaveLength(4);
    expect(renderTable([job("queued")])).not.toContain(i18n.t("waitingForMemory", { count: 0 }));
  });

  it.each(["en", "zh", "ja"])("offers localized error details from failed item counts in %s", async (language) => {
    await i18n.changeLanguage(language);
    const failed = job("partially_succeeded", 3, 3);
    failed.counts.failed = 2;
    const markup = renderTable([failed]);
    expect(markup).toContain(`>${i18n.t("jobFailedItems", { count: 2 })}</button>`);
    expect(markup).not.toContain(`>${i18n.t("viewJobErrors")}</button>`);
    expect(markup).toContain(i18n.t("viewJobErrorsFor", { jobId: failed.id }));
    expect(markup.match(/<td\b[^>]*>/g)).toHaveLength(4);
    expect(renderTable([job("succeeded")])).not.toContain(i18n.t("viewJobErrors"));
  });

  it("allows inspecting failures while the rest of a job is still running", () => {
    const running = job("running", 3, 1);
    running.counts.failed = 1;
    const markup = renderTable([running]);
    expect(markup).toContain(i18n.t("viewJobErrorsFor", { jobId: running.id }));
    expect(markup).not.toContain('disabled=""');
  });
});
