import { useTranslation } from "react-i18next";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { formatBackendError, useBackendRuntimeState } from "@/features/backend";
import type { JobStatus } from "@/lib/ipc";

type ObservedBackendState = ReturnType<typeof useBackendRuntimeState>;
type ObservedJobSnapshot = NonNullable<
  ObservedBackendState["snapshot"]
>["jobs"][number];

const STATUS_LAMP_CLASSES: Record<JobStatus, string> = {
  queued: "bg-gray-400",
  running: "bg-yellow-400",
  paused: "bg-blue-400",
  cancelling: "bg-orange-400",
  succeeded: "bg-green-400",
  partially_succeeded: "bg-amber-500",
  failed: "bg-red-400",
  cancelled: "bg-zinc-500",
};

function jobProgress(job: ObservedJobSnapshot) {
  if (job.progress.totalItems === 0) {
    return "—";
  }

  const percent = Math.round(
    (job.progress.completedItems / job.progress.totalItems) * 100,
  );
  return `${Math.min(percent, 100)}% · ${job.progress.completedItems}/${job.progress.totalItems}`;
}

export default function TaskTable() {
  const { t } = useTranslation();
  const backend = useBackendRuntimeState();
  const jobs = backend.snapshot?.jobs ?? [];
  const emptyMessage =
    backend.syncStatus === "ready"
      ? t("noJobs")
      : backend.lastError
        ? formatBackendError(backend.lastErrorDetails ?? backend.lastError, t)
        : t("synchronizingJobs");

  return (
    <div className="min-h-0 w-full flex-1 overflow-hidden rounded-lg border bg-background">
      <Table
        className="select-none"
        containerClassName="h-full overflow-auto"
        style={{ minWidth: 462 }}
      >
        <TableHeader className="sticky top-0 z-10 bg-background drop-shadow">
          <TableRow>
            <TableHead
              className="w-8 px-1"
              style={{ width: 32, minWidth: 32, maxWidth: 32 }}
            />
            <TableHead style={{ minWidth: 160 }}>{t("file")}</TableHead>
            <TableHead style={{ minWidth: 130 }}>{t("progress")}</TableHead>
            <TableHead style={{ minWidth: 140 }}>{t("options")}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {jobs.length ? (
            jobs.map((job) => (
              <TableRow key={job.id}>
                <TableCell
                  className="w-8 px-1"
                  style={{ width: 32, minWidth: 32, maxWidth: 32 }}
                >
                  <figure className="flex justify-center items-center">
                    <div
                      className={`h-2 w-2 rounded-full ${STATUS_LAMP_CLASSES[job.status]}`}
                    />
                  </figure>
                </TableCell>
                <TableCell style={{ minWidth: 160 }}>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="min-w-0">
                        <p className="whitespace-nowrap text-ellipsis w-full overflow-hidden">
                          {job.encoder} · {job.id}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {t("itemCount", { count: job.counts.total })}
                        </p>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      <p>{job.id}</p>
                    </TooltipContent>
                  </Tooltip>
                </TableCell>
                <TableCell style={{ minWidth: 130 }}>
                  <p className="text-sm tabular-nums">{jobProgress(job)}</p>
                </TableCell>
                <TableCell style={{ minWidth: 140 }}>
                  <div className="min-w-0">
                    <p className="text-sm">{t("jobStatus." + job.status)}</p>
                    {job.error && (
                      <p className="text-xs text-red-500 truncate">
                        {formatBackendError(job.error, t)}
                      </p>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            ))
          ) : (
            <TableRow>
              <TableCell colSpan={4} className="h-24 text-center">
                {emptyMessage}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
