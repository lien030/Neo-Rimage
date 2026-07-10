import { Task, TaskStatusType } from "@/lib/type";
import {
  ColumnDef,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";

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
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

import { useTaskStore } from "@/lib/State";
import { useTranslation } from "react-i18next";

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
}

function lampColor(status: TaskStatusType) {
  switch (status) {
    case TaskStatusType.Idle:
      return "bg-gray-400";
    case TaskStatusType.Processing:
      return "bg-yellow-400";
    case TaskStatusType.Done:
      return "bg-green-400";
    case TaskStatusType.Error:
      return "bg-red-400";
  }
}

const colums: ColumnDef<Task>[] = [
  {
    accessorKey: "status",
    header: "",
    minSize: 32,
    maxSize: 32,
    cell: ({ row }) => {
      const rowData = row.original;
      return (
        <figure className="flex justify-center items-center">
          <div
            className={`h-2 w-2 rounded-full ${lampColor(rowData.status)}`}
          />
        </figure>
      );
    },
  },
  {
    accessorKey: "fileName",
    header: ()=>{
      const {t} = useTranslation();
      return t("file");
    },
    minSize: 120,
    maxSize: 120,
    cell: ({ row }) => {
      const rowData = row.original;
      return (
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="whitespace-nowrap text-ellipsis w-full overflow-hidden">
                {rowData.fileName}
              </p>
            </TooltipTrigger>
            <TooltipContent>
              <p>{rowData.fileName}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      );
    },
  },
  // {
  //   accessorKey: "encoder",
  //   header: "Encoder",
  //   minSize: 50,
  //   maxSize: 50,
  // },
  // {
  //   accessorKey: "suffix",
  //   header: "Suffix",
  // },
  // {
  //   accessorKey: "backup",
  //   header: "Backup",
  // },
  // {
  //   accessorKey: "recursive",
  //   header: "Recursive",
  // },
  {
    accessorKey: "actions",
    header: ()=>{
      const {t} = useTranslation();
      return t("options");
    },
    minSize: 200,
    maxSize: 200,
  },
];

function DataTable<TData, TValue>({
  columns,
  data,
}: DataTableProps<TData, TValue>) {
  const table = useReactTable({
    data,
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
            {headerGroup.headers.map((header) => {
              return (
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
                        header.getContext()
                      )}
                </TableHead>
              );
            })}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows?.length ? (
          table.getRowModel().rows.map((row) => (
            <TableRow
              key={row.id}
              data-state={row.getIsSelected() && "selected"}
            >
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
              No results.
            </TableCell>
          </TableRow>
        )}
      </TableBody>
    </Table>
  );
}

export default function TaskTable() {
  const taskstore = useTaskStore();
  return (
    <div className="w-full h-[488px] border rounded-lg bg-white overflow-hidden">
      <DataTable columns={colums} data={taskstore.taskList} />
    </div>
  );
}
