import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import i18n from "@/i18n/config";

import App from "./App";

vi.mock("@/features/backend", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/features/backend")>(),
  useBackendRuntimeSync: vi.fn(),
  useBackendCommandState: () => ({ schedulerPending: false, workerCountPending: false }),
  useBackendRuntimeState: () => ({
    capabilities: null,
    syncStatus: "ready",
    lastError: null,
    lastErrorDetails: null,
    snapshot: {
      jobs: [],
      workerSlots: [{
        id: "worker-1", status: "idle", itemId: null,
        inputPath: null, stage: null, progress: null,
      }],
      scheduler: {
        mode: "paused", effectiveConcurrency: 1,
        desiredConcurrency: 1, maxConcurrency: 4,
      },
    },
  }),
}));

describe("homepage translations", () => {
  it.each([
    ["en", "Progress", "No backend jobs.", "Idle", "GO"],
    ["zh", "进度", "暂无任务。", "空闲", "开始"],
    ["ja", "進捗", "タスクはありません。", "待機中", "開始"],
  ])("renders the complete empty homepage in %s", async (language, progress, empty, idle, start) => {
    await i18n.changeLanguage(language);
    const markup = renderToStaticMarkup(<TooltipProvider><App /></TooltipProvider>);
    const text = markup.replace(/<[^>]+>/g, " ");
    for (const message of [progress, empty, idle, start]) expect(text).toContain(message);
    if (language !== "en") {
      expect(text).not.toMatch(/\b(Progress|Idle|No backend jobs|SYNC|STOP|GO)\b/);
    }
    expect(markup).toContain(i18n.t("increaseWorkers"));
    expect(markup).toContain(i18n.t("decreaseWorkers"));
    expect(markup).toContain(i18n.t("createTask"));
  });
});
