import { useBackendRuntimeState } from "@/lib/State";
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

function lampColor(status: WorkerSlotStatus) {
  switch (status) {
    case "idle":
      return "bg-green-400";
    case "busy":
      return "bg-yellow-400";
    case "draining":
      return "bg-orange-400";
  }
}

function progressText(
  progress: ItemProgress | null,
  stage: ProcessingStage | null,
) {
  if (!progress) {
    return stage;
  }

  if (progress.measure.kind === "fraction" && progress.measure.total > 0) {
    const percent = Math.round(
      (progress.measure.completed / progress.measure.total) * 100,
    );
    return `${progress.stage} · ${Math.min(percent, 100)}%`;
  }

  return progress.stage;
}

export default function WorkerList() {
  const backend = useBackendRuntimeState();
  const workers = backend.snapshot?.workerSlots ?? [];

  return (
    <div className="min-h-0 w-full flex-1 rounded-lg bg-background/50 px-2">
      <div className="flex h-full min-h-0 w-full flex-col gap-2 overflow-y-auto py-2">
        {workers.length === 0 && (
          <div className="h-full flex items-center justify-center text-sm text-muted-foreground">
            {backend.syncStatus === "ready"
              ? "No worker slots"
              : backend.lastError ?? "Synchronizing workers…"}
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
  const activity = progressText(worker.progress, worker.stage);
  const itemLabel = worker.inputPath
    ? inputFileName(worker.inputPath)
    : worker.itemId
      ? `#${worker.itemId.slice(0, 8)}`
      : worker.status === "draining"
        ? "Draining"
        : "Idle";

  return (
    <div className="grid min-h-12 w-full grid-cols-[36px_minmax(0,1fr)] rounded-sm border border-zinc-300 bg-muted-foreground/5">
      <figure className="flex justify-center items-center">
        <div className={`h-2 w-2 rounded-full ${lampColor(worker.status)}`} />
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
          <p className="text-xs text-muted-foreground truncate">{activity}</p>
        )}
      </div>
    </div>
  );
}
