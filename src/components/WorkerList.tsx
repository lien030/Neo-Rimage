import { useBackendRuntimeState } from "@/lib/State";
import type {
  ItemProgress,
  WorkerSlotSnapshot,
  WorkerSlotStatus,
} from "@/lib/ipc";

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

function progressText(progress: ItemProgress | null) {
  if (!progress) {
    return null;
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
    <div className="w-full grow max-h-[438px] px-2 rounded-lg bg-background/50">
      <div className="w-full h-full pt-2 flex flex-col overflow-y-auto gap-2">
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
  const activity = progressText(worker.progress);
  const itemLabel = worker.itemId
    ? `#${worker.itemId.slice(0, 8)}`
    : worker.status === "draining"
      ? "Draining"
      : "Idle";

  return (
    <div className="min-h-12 w-full grid grid-cols-[36px_auto] border border-zinc-300 bg-muted-foreground/5 rounded-sm">
      <figure className="flex justify-center items-center">
        <div className={`h-2 w-2 rounded-full ${lampColor(worker.status)}`} />
      </figure>
      <div className="min-w-0 py-1 flex flex-col justify-center">
        <p className="text-sm truncate">{itemLabel}</p>
        {activity && (
          <p className="text-xs text-muted-foreground truncate">{activity}</p>
        )}
      </div>
    </div>
  );
}
