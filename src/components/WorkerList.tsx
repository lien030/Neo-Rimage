import { useTranslation } from "react-i18next";

import { formatBackendError, useBackendRuntimeState } from "@/features/backend";
import type {
  ItemProgress,
  ProcessingStage,
  WorkerSlotSnapshot,
  WorkerSlotStatus,
} from "@/lib/ipc";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { inputFileName } from "./worker-display";

const STATUS_LAMP_CLASSES: Record<WorkerSlotStatus, string> = {
  idle: "bg-green-400",
  busy: "bg-yellow-400",
  draining: "bg-orange-400",
};

function formatWorkerActivity(
  progress: ItemProgress | null,
  stage: ProcessingStage | null,
  translate: (key: string) => string,
) {
  if (!progress) {
    return stage ? translate("processingStage." + stage) : null;
  }

  if (progress.measure.kind === "fraction" && progress.measure.total > 0) {
    const percent = Math.round(
      (progress.measure.completed / progress.measure.total) * 100,
    );
    return `${translate("processingStage." + progress.stage)} · ${Math.min(percent, 100)}%`;
  }

  return translate("processingStage." + progress.stage);
}

function workerItemLabel(
  worker: WorkerSlotSnapshot,
  translate: (key: string) => string,
): string {
  if (worker.inputPath) {
    return inputFileName(worker.inputPath);
  }
  if (worker.itemId) {
    return `#${worker.itemId.slice(0, 8)}`;
  }
  return translate("workerStatus." + worker.status);
}

export default function WorkerList() {
  const { t } = useTranslation();
  const backend = useBackendRuntimeState();
  const workers = backend.snapshot?.workerSlots ?? [];

  return (
    <div className="min-h-0 w-full flex-1 rounded-lg bg-background/50 px-2">
      <div className="flex h-full min-h-0 w-full flex-col gap-2 overflow-y-auto py-2">
        {workers.length === 0 && (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            {backend.syncStatus === "ready"
              ? t("noWorkers")
              : backend.lastError
                ? formatBackendError(backend.lastErrorDetails ?? backend.lastError, t)
                : t("synchronizingWorkers")}
          </div>
        )}
        {workers.map((worker) => (
          <WorkerCard key={worker.id} worker={worker} />
        ))}
      </div>
    </div>
  );
}

function WorkerCard({ worker }: { worker: WorkerSlotSnapshot }) {
  const { t } = useTranslation();
  const activity = formatWorkerActivity(worker.progress, worker.stage, t);
  const itemLabel = workerItemLabel(worker, t);

  return (
    <div className="grid min-h-12 w-full grid-cols-[36px_minmax(0,1fr)] rounded-sm border border-zinc-300 bg-muted-foreground/5">
      <figure className="flex items-center justify-center">
        <div
          className={`h-2 w-2 rounded-full ${STATUS_LAMP_CLASSES[worker.status]}`}
          title={t("workerStatus." + worker.status)}
        />
      </figure>
      <div className="flex min-w-0 flex-col justify-center overflow-hidden py-1">
        {worker.inputPath ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="truncate text-sm">{itemLabel}</p>
            </TooltipTrigger>
            <TooltipContent side="left" className="max-w-sm break-all">
              <p>{worker.inputPath}</p>
            </TooltipContent>
          </Tooltip>
        ) : (
          <p className="truncate text-sm">{itemLabel}</p>
        )}
        {activity && (
          <p className="truncate text-xs text-muted-foreground">{activity}</p>
        )}
      </div>
    </div>
  );
}
