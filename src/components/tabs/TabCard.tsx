import { Input } from "../ui/input";
import { Switch } from "../ui/switch";

export default function TabCard({
  variant = "number",
  title,
  value,
  onValueChange,
  min,
  max,
  placeholder,
  disabled = false,
  children,
}: {
  variant?: "number" | "string" | "boolean" | "none";
  title: string;
  value?: string | boolean;
  onValueChange?: (value: string | boolean) => void;
  min?: number;
  max?: number;
  placeholder?: string;
  disabled?: boolean;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex h-[72px] min-w-0 w-full flex-col justify-between rounded-lg border bg-white px-2 py-1 text-[0.775rem] select-none">
      <span className="flex min-w-0 items-center justify-between gap-1">
        <p className="min-w-0 truncate font-bold" title={title}>
          {title}
        </p>
        {variant === "boolean" && (
          <Switch
            size="sm"
            checked={value === true}
            disabled={disabled}
            onCheckedChange={(checked) => onValueChange?.(checked)}
          />
        )}
      </span>
      {variant === "number" && (
        <Input
          type={variant === "number" ? "number" : "text"}
          value={typeof value === "string" ? value : ""}
          placeholder={placeholder}
          min={variant === "number" ? min : undefined}
          max={variant === "number" ? max : undefined}
          disabled={disabled}
          className="h-6 px-1.5 my-1 text-right"
          onChange={(event) => onValueChange?.(event.target.value)}
        />
      )}
      {children}
    </div>
  );
}
