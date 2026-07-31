import { CircleHelp } from "lucide-react";
import { useId } from "react";

import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface CompactNumberFieldProps {
  label: string;
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}

export function CompactNumberField({
  label,
  value,
  disabled,
  onChange,
}: CompactNumberFieldProps) {
  return (
    <label className="flex items-center gap-2 text-xs">
      <span className="grow text-right">{label}</span>
      <Input
        type="number"
        min={1}
        value={value}
        disabled={disabled}
        className="h-7 w-20 px-1 text-right"
        onChange={(event) => onChange(event.target.value)}
      />
      <span>px</span>
    </label>
  );
}

interface CompactSwitchProps {
  label: string;
  description?: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
}

export function CompactSwitch({
  label,
  description,
  checked,
  disabled = false,
  onCheckedChange,
}: CompactSwitchProps) {
  const switchId = useId();

  return (
    <div className="flex min-w-0 items-center justify-between gap-2 text-[0.7rem]">
      <span className="flex min-w-0 items-center gap-1">
        <label htmlFor={switchId} className="truncate">
          {label}
        </label>
        {description && <InlineHelp description={description} />}
      </span>
      <Switch
        id={switchId}
        size="sm"
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
      />
    </div>
  );
}

export function InlineHelp({ description }: { description: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="shrink-0 text-muted-foreground/70 transition-colors hover:text-foreground focus-visible:text-foreground"
          aria-label={description}
        >
          <CircleHelp className="size-3" />
        </button>
      </TooltipTrigger>
      <TooltipContent
        side="top"
        sideOffset={6}
        className="max-w-64 leading-relaxed"
      >
        <p>{description}</p>
      </TooltipContent>
    </Tooltip>
  );
}
