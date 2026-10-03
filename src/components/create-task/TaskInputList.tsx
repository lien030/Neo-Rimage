import { X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface TaskInput {
  readonly fileName: string;
  readonly path: string;
}

interface TaskInputListProps {
  tasks: readonly TaskInput[];
  emptyMessage: string;
  disabled: boolean;
  onRemove: (path: string) => void;
}

export function TaskInputList({
  tasks,
  emptyMessage,
  disabled,
  onRemove,
}: TaskInputListProps) {
  return (
    <div className="flex min-h-0 min-w-0 select-none flex-col overflow-x-hidden overflow-y-auto rounded-lg border">
      {tasks.length === 0 ? (
        <p className="m-auto px-4 text-center text-xs text-muted-foreground">
          {emptyMessage}
        </p>
      ) : (
        tasks.map((task) => (
          <TaskInputRow
            key={task.path}
            task={task}
            disabled={disabled}
            onRemove={onRemove}
          />
        ))
      )}
    </div>
  );
}

function TaskInputRow({
  task,
  disabled,
  onRemove,
}: {
  task: TaskInput;
  disabled: boolean;
  onRemove: (path: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid h-7 w-full grid-cols-[minmax(0,1fr)_20px] gap-1 px-2 py-1 hover:bg-slate-100">
      <Tooltip>
        <TooltipTrigger asChild>
          <p className="w-full overflow-hidden text-ellipsis whitespace-nowrap text-sm">
            {task.fileName}
          </p>
        </TooltipTrigger>
        <TooltipContent>
          <p>{task.path}</p>
        </TooltipContent>
      </Tooltip>

      <figure className="flex items-center justify-center">
        <Button
          type="button"
          variant="ghost"
          disabled={disabled}
          className="m-0 h-full p-0"
          onClick={() => onRemove(task.path)}
          aria-label={t("removeInput")}
          title={t("removeInput")}
        >
          <X
            size={16}
            className="text-muted-foreground/50 hover:text-muted-foreground"
          />
        </Button>
      </figure>
    </div>
  );
}
