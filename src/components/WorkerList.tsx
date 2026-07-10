import { useWorkerList } from "@/lib/State";
import { ProcessWorker, WorkerStatusType } from "@/lib/type";

export default function WorkerList() {
  const workers = useWorkerList();

  return (
    <div className="w-full grow max-h-[438px] px-2 rounded-lg bg-background/50">
      <div className="w-full h-full pt-2 flex flex-col overflow-y-auto gap-2">
        {workers.map((worker) => (
          <WorkerCard key={worker.id} {...worker} />
        ))}
      </div>
    </div>
  );
}

function WorkerCard(info: ProcessWorker) {
  const lampColor = () => {
    switch (info.status) {
      case WorkerStatusType.Idle:
        return "bg-green-400";
      case WorkerStatusType.Busy:
        return "bg-yellow-400";
      case WorkerStatusType.Done:
        return "bg-green-400";
    }
  };

  return (
    <div className="h-10 min-h-10 w-full grid grid-cols-[36px_auto] border border-zinc-300 bg-muted-foreground/5 rounded-sm">
      <figure className="flex justify-center items-center">
        <div className={`h-2 w-2 rounded-full ${lampColor()}`} />
      </figure>
      <div className="flex items-center">
        {info.task ? info.task.fileName : "💤"}
      </div>
    </div>
  );
}

