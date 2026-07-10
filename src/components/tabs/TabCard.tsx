import { Input } from "../ui/input";
import { Switch } from "../ui/switch";

export default function TabCard({
  variant = "number",
  title,
  defaultValue,
  onValueChange,
  min,
  max,
  children,
}: {
  variant?: "number" | "string" | "boolean" | "none";
  title: string;
  defaultValue?: any;
  onValueChange?: (value: any) => void;
  min?: number;
  max?: number;
  children?: React.ReactNode;
}) {
  return (
    <div className="h-[72px] w-36 border bg-white rounded-lg flex flex-col justify-between select-none px-2 py-1 text-[0.775rem]">
      <span className="flex justify-between items-center">
        <p className="font-bold">{title}</p>
        {variant === "boolean" && (
          <Switch
            size="sm"
            defaultChecked={defaultValue}
            onCheckedChange={onValueChange}
          />
        )}
      </span>
      {variant === "number" && (
        <Input
          type={variant === "number" ? "number" : "text"}
          defaultValue={defaultValue}
          min={variant === "number" ? min : undefined}
          max={variant === "number" ? max : undefined}
          className="h-6 px-1.5 my-1 text-right"
          onBlur={(e) => {
            onValueChange && onValueChange(e.target.value);
          }}
        />
      )}
      {children}
    </div>
  );
}
