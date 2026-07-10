import { Badge } from "@/components/ui/badge";
import { Lightbulb, SlidersHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";

export function EncoderPanel({
  description,
  extensions,
  children,
}: {
  description: string;
  extensions: readonly string[];
  children: React.ReactNode;
}) {
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
      <div className="flex min-h-0 w-full flex-1 content-start gap-2 overflow-y-auto pr-1 flex-wrap">
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
  extensions: readonly string[];
}) {
  const { t } = useTranslation();

  return (
    <EncoderPanel description={description} extensions={extensions}>
      <div className="flex min-h-28 w-full items-center justify-center rounded-lg border border-dashed px-6 text-center text-muted-foreground">
        <div className="flex max-w-sm flex-col items-center gap-2">
          <SlidersHorizontal className="size-5" />
          <p className="text-xs">{t("encoderNoSpecificOptions")}</p>
        </div>
      </div>
    </EncoderPanel>
  );
}
