import type { ReactNode } from "react";

import { InlineHelp } from "../create-task/DraftControls";
import { Input } from "../ui/input";
import { Switch } from "../ui/switch";

interface BaseTabCardProps {
  title: string;
  description?: string;
  disabled?: boolean;
}

interface NumberTabCardProps extends BaseTabCardProps {
  variant?: "number";
  value: string;
  onValueChange: (value: string) => void;
  min?: number;
  max?: number;
  placeholder?: string;
}

interface BooleanTabCardProps extends BaseTabCardProps {
  variant: "boolean";
  value: boolean;
  onValueChange: (value: boolean) => void;
}

interface ContentTabCardProps extends BaseTabCardProps {
  variant: "none";
  children: ReactNode;
}

type TabCardProps =
  | NumberTabCardProps
  | BooleanTabCardProps
  | ContentTabCardProps;

export default function TabCard(props: TabCardProps) {
  return (
    <div className="flex h-[72px] w-full min-w-0 select-none flex-col justify-between rounded-lg border bg-white px-2 py-1 text-[0.775rem]">
      <span className="flex min-w-0 items-center justify-between gap-1">
        <p className="min-w-0 truncate font-bold" title={props.title}>
          {props.title}
        </p>
        {props.description && <InlineHelp label={props.title} description={props.description} />}
        {props.variant === "boolean" && (
          <Switch
            size="sm"
            aria-label={props.title}
            checked={props.value}
            disabled={props.disabled}
            onCheckedChange={props.onValueChange}
          />
        )}
      </span>
      {(props.variant === undefined || props.variant === "number") && (
        <Input
          type="number"
          aria-label={props.title}
          value={props.value}
          placeholder={props.placeholder}
          min={props.min}
          max={props.max}
          disabled={props.disabled}
          className="my-1 h-7 px-1.5 text-right"
          onChange={(event) => props.onValueChange(event.target.value)}
        />
      )}
      {props.variant === "none" && props.children}
    </div>
  );
}
