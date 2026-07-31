import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { Lightbulb, SlidersHorizontal } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

export interface EncoderTabProps {
  extensions: readonly string[];
}

interface EncoderPanelProps extends EncoderTabProps {
  description: string;
  fillContent?: boolean;
  children: ReactNode;
}

export function EncoderPanel({
  description,
  extensions,
  fillContent = false,
  children,
}: EncoderPanelProps) {
  return (
    <div className="flex h-full min-h-0 w-full flex-col gap-1">
      <div className="mx-2 flex h-5 min-w-0 items-center gap-1">
        <Lightbulb
          size={16}
          className="shrink-0 -rotate-12 text-muted-foreground"
        />
        <p
          className="min-w-0 flex-1 truncate text-[0.775rem] text-muted-foreground"
          title={description}
        >
          {description}
        </p>
        {extensions.map((extension) => (
          <Badge
            key={extension}
            variant="secondary"
            className="h-4 px-1.5 py-0 text-[0.6rem]"
          >
            .{extension}
          </Badge>
        ))}
      </div>
      <div
        className={cn(
          "grid min-h-0 w-full flex-1 grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-2 overflow-y-auto pr-1",
          fillContent
            ? "grid-rows-[minmax(0,1fr)] content-stretch"
            : "auto-rows-max content-start",
        )}
      >
        {children}
      </div>
    </div>
  );
}

export function OptionlessEncoderPanel({
  description,
  extensions,
}: {
  description: string;
  extensions: EncoderTabProps["extensions"];
}) {
  const { t } = useTranslation();

  return (
    <EncoderPanel
      description={description}
      extensions={extensions}
      fillContent
    >
      <div className="col-span-full flex h-full min-h-28 w-full items-center justify-center rounded-lg border border-dashed px-6 text-center text-muted-foreground">
        <div className="flex max-w-sm flex-col items-center gap-2">
          <SlidersHorizontal className="size-5" />
          <p className="text-xs">{t("encoderNoSpecificOptions")}</p>
        </div>
      </div>
    </EncoderPanel>
  );
}
