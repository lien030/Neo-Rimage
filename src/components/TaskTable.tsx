import { useMemo } from "react";
import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
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
import { useBackendRuntimeState } from "@/lib/State";
import type { JobStatus } from "@/lib/ipc";

type ObservedBackendState = ReturnType<typeof useBackendRuntimeState>;
type ObservedJobSnapshot = NonNullable<
  ObservedBackendState["snapshot"]
>["jobs"][number];

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: readonly TData[];
  emptyMessage: string;
}

function lampColor(status: JobStatus) {
  switch (status) {
    case "queued":
      return "bg-gray-400";
    case "running":
      return "bg-yellow-400";
    case "paused":
      return "bg-blue-400";
    case "cancelling":
      return "bg-orange-400";
    case "succeeded":
      return "bg-green-400";
    case "partially_succeeded":
      return "bg-amber-500";
    case "failed":
      return "bg-red-400";
    case "cancelled":
      return "bg-zinc-500";
  }
}

function statusLabel(status: JobStatus) {
  return status
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function jobProgress(job: {
  readonly progress: {
    readonly completedItems: number;
    readonly totalItems: number;
  };
}) {
  if (job.progress.totalItems === 0) {
    return "—";
  }

  const percent = Math.round(
    (job.progress.completedItems / job.progress.totalItems) * 100,
  );
  return `${Math.min(percent, 100)}% · ${job.progress.completedItems}/${job.progress.totalItems}`;
}

function DataTable<TData, TValue>({
  columns,
  data,
  emptyMessage,
}: DataTableProps<TData, TValue>) {
  const table = useReactTable({
    data: [...data],
    columns,
    getCoreRowModel: getCoreRowModel(),
    defaultColumn: {
      enableResizing: true,
      minSize: 100,
    },
  });

  return (
    <Table className="select-none">
      <TableHeader className="sticky top-0 bg-background drop-shadow">
        {table.getHeaderGroups().map((headerGroup) => (
          <TableRow key={headerGroup.id}>
            {headerGroup.headers.map((header) => (
              <TableHead
                key={header.id}
                style={{
                  width: header.column.getSize(),
                  minWidth: header.column.getSize(),
                }}
              >
                {header.isPlaceholder
                  ? null
                  : flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
              </TableHead>
            ))}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.length ? (
          table.getRowModel().rows.map((row) => (
            <TableRow key={row.id}>
              {row.getVisibleCells().map((cell) => (
                <TableCell
                  key={cell.id}
                  style={{
                    width: cell.column.getSize(),
                    minWidth: cell.column.getSize(),
                    maxWidth: cell.column.getSize(),
                  }}
                >
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </TableCell>
              ))}
            </TableRow>
          ))
        ) : (
          <TableRow>
            <TableCell colSpan={columns.length} className="h-24 text-center">
              {emptyMessage}
            </TableCell>
          </TableRow>
        )}
      </TableBody>
    </Table>
  );
}

export default function TaskTable() {
  const { t } = useTranslation();
  const backend = useBackendRuntimeState();
  const jobs = backend.snapshot ? [...backend.snapshot.jobs] : [];

  const columns = useMemo<ColumnDef<ObservedJobSnapshot>[]>(
    () => [
      {
        accessorKey: "status",
        header: "",
        minSize: 32,
        maxSize: 32,
        cell: ({ row }) => (
          <figure className="flex justify-center items-center">
            <div
              className={`h-2 w-2 rounded-full ${lampColor(row.original.status)}`}
            />
          </figure>
        ),
      },
      {
        accessorKey: "id",
        header: t("file"),
        minSize: 160,
        maxSize: 220,
        cell: ({ row }) => {
          const job = row.original;
          return (
            <Tooltip>
              <TooltipTrigger asChild>
                <div className="min-w-0">
                  <p className="whitespace-nowrap text-ellipsis w-full overflow-hidden">
                    {job.encoder} · {job.id}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {job.counts.total} item{job.counts.total === 1 ? "" : "s"}
                  </p>
                </div>
              </TooltipTrigger>
              <TooltipContent>
                <p>{job.id}</p>
              </TooltipContent>
            </Tooltip>
          );
        },
      },
      {
        id: "progress",
        header: "Progress",
        minSize: 130,
        maxSize: 150,
        cell: ({ row }) => (
          <p className="text-sm tabular-nums">{jobProgress(row.original)}</p>
        ),
      },
      {
        id: "state",
        header: t("options"),
        minSize: 140,
        maxSize: 170,
        cell: ({ row }) => (
          <div className="min-w-0">
            <p className="text-sm">{statusLabel(row.original.status)}</p>
            {row.original.error && (
              <p className="text-xs text-red-500 truncate">
                {row.original.error.fallbackMessage}
              </p>
            )}
          </div>
        ),
      },
    ],
    [t],
  );

  const emptyMessage =
    backend.syncStatus === "ready"
      ? "No backend jobs."
      : backend.lastError ?? "Synchronizing backend jobs…";

  return (
    <div className="w-full h-[488px] border rounded-lg bg-background overflow-hidden">
      <DataTable columns={columns} data={jobs} emptyMessage={emptyMessage} />
    </div>
  );
}
