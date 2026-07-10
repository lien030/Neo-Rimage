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
  children,
}: {
  variant?: "number" | "string" | "boolean" | "none";
  title: string;
  value?: string | boolean;
  onValueChange?: (value: string | boolean) => void;
  min?: number;
  max?: number;
  placeholder?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="h-[72px] w-36 border bg-white rounded-lg flex flex-col justify-between select-none px-2 py-1 text-[0.775rem]">
      <span className="flex justify-between items-center">
        <p className="font-bold">{title}</p>
        {variant === "boolean" && (
          <Switch
            size="sm"
            checked={value === true}
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
          className="h-6 px-1.5 my-1 text-right"
          onChange={(event) => onValueChange?.(event.target.value)}
        />
      )}
      {children}
    </div>
  );
}
