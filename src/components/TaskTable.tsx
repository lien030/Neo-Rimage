import { type CSSProperties, useMemo } from "react";
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
import { useBackendRuntimeState } from "@/features/backend";
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

const EMPTY_JOBS: readonly ObservedJobSnapshot[] = [];

interface SizedColumn {
  id: string;
  getSize: () => number;
  columnDef: { minSize?: number };
}

function columnLayout(column: SizedColumn): {
  className?: string;
  style: CSSProperties;
} {
  const isStatusColumn = column.id === "status";
  const fixedWidth = isStatusColumn ? column.getSize() : undefined;

  return {
    className: isStatusColumn ? "w-8 px-1" : undefined,
    style: {
      width: fixedWidth,
      minWidth: column.columnDef.minSize,
      maxWidth: fixedWidth,
    },
  };
}

function statusLabel(status: JobStatus) {
  return status
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function jobProgress(job: ObservedJobSnapshot) {
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
  // TanStack Table treats data identity as a change signal. Keep it stable
  // across the table's own internal state updates to avoid a render loop.
  const stableData = useMemo(() => [...data], [data]);
  const table = useReactTable({
    data: stableData,
    columns,
    getCoreRowModel: getCoreRowModel(),
    defaultColumn: {
      enableResizing: true,
      minSize: 100,
    },
  });
  const minimumTableWidth = table
    .getAllLeafColumns()
    .reduce(
      (width, column) => width + (column.columnDef.minSize ?? 0),
      0,
    );
  const rows = table.getRowModel().rows;

  return (
    <Table
      className="select-none"
      containerClassName="h-full overflow-auto"
      style={{ minWidth: minimumTableWidth }}
    >
      <TableHeader className="sticky top-0 z-10 bg-background drop-shadow">
        {table.getHeaderGroups().map((headerGroup) => (
          <TableRow key={headerGroup.id}>
            {headerGroup.headers.map((header) => {
              const layout = columnLayout(header.column);

              return (
                <TableHead
                  key={header.id}
                  className={layout.className}
                  style={layout.style}
                >
                  {header.isPlaceholder
                    ? null
                    : flexRender(
                        header.column.columnDef.header,
                        header.getContext(),
                      )}
                </TableHead>
              );
            })}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {rows.length ? (
          rows.map((row) => (
            <TableRow key={row.id}>
              {row.getVisibleCells().map((cell) => {
                const layout = columnLayout(cell.column);

                return (
                  <TableCell
                    key={cell.id}
                    className={layout.className}
                    style={layout.style}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                );
              })}
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
  const jobs = backend.snapshot?.jobs ?? EMPTY_JOBS;

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
              className={`h-2 w-2 rounded-full ${STATUS_LAMP_CLASSES[row.original.status]}`}
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
    <div className="min-h-0 w-full flex-1 overflow-hidden rounded-lg border bg-background">
      <DataTable columns={columns} data={jobs} emptyMessage={emptyMessage} />
    </div>
  );
}
