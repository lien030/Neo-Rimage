import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import i18n from "@/i18n/config";
import type { BackendSyncStatus, ProcessingStage, WorkerSlotSnapshot, WorkerSlotStatus } from "@/lib/ipc";

import WorkerList from "./WorkerList";

const mocks = vi.hoisted(() => ({ useBackendRuntimeState: vi.fn() }));

vi.mock("@/features/backend", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/features/backend")>(),
  ...mocks,
}));

function worker(status: WorkerSlotStatus, stage: ProcessingStage | null = null): WorkerSlotSnapshot {
  return {
    id: "worker-1",
    status,
    inputPath: null,
    itemId: null,
    stage,
    progress: null,
  };
}

function renderWorkers(
  workers: WorkerSlotSnapshot[],
  syncStatus: BackendSyncStatus = "ready",
) {
  mocks.useBackendRuntimeState.mockReturnValue({
    snapshot: { workerSlots: workers },
    syncStatus,
    lastError: null,
    lastErrorDetails: null,
  });
  return renderToStaticMarkup(<TooltipProvider><WorkerList /></TooltipProvider>);
}

describe("WorkerList translations", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
  });

  it.each(["en", "zh", "ja"])("localizes every worker status and empty state in %s", async (language) => {
    await i18n.changeLanguage(language);
    expect(renderWorkers([])).toContain(i18n.t("noWorkers"));
    expect(renderWorkers([], "syncing")).toContain(i18n.t("synchronizingWorkers"));
    for (const status of ["idle", "busy", "draining"] as const) {
      expect(renderWorkers([worker(status)])).toContain(i18n.t("workerStatus." + status));
    }
  });

  it.each(["en", "zh", "ja"])("localizes every processing stage in %s", async (language) => {
    await i18n.changeLanguage(language);
    for (const stage of [
      "preflight", "inspect", "decode", "normalize", "operations", "encode",
      "commit", "metadata_finalize", "complete",
    ] as const) {
      expect(renderWorkers([worker("busy", stage)])).toContain(i18n.t("processingStage." + stage));
    }
  });

  it("updates existing worker text after a language change while preserving file names and progress", async () => {
    const active = worker("busy", "decode");
    active.inputPath = "C:\\images\\photo.png";
    active.progress = { stage: "decode", measure: { kind: "fraction", completed: 1, total: 2 } };
    expect(renderWorkers([active])).toContain("Decoding · 50%");
    await i18n.changeLanguage("zh");
    const markup = renderWorkers([active]);
    expect(markup).toContain("photo.png");
    expect(markup).toContain("解码中 · 50%");
    expect(markup).not.toContain("Decoding");
    active.progress.measure = { kind: "fraction", completed: 0, total: 0 };
    expect(renderWorkers([active])).not.toContain("NaN");
  });
});
