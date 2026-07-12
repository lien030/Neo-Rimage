import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  createTaskDraft,
  markCreateTaskDirty,
} from "@/features/create-task";
import type { BackendSyncStatus } from "@/features/backend";
import type {
  EncoderCapability,
  EncoderKind,
} from "@/lib/ipc/contracts";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import AvifTab from "./AvifTab";
import { OptionlessEncoderPanel } from "./EncoderPanel";
import JpegTab from "./JpegTab";
import MozjpegTab from "./MozjpegTab";
import OxiPngTab from "./OxiPngTab";
import WebPTab from "./WebPTab";

const ENCODER_LABELS: Record<EncoderKind, string> = {
  mozjpeg: "MozJPEG",
  jpeg: "JPEG",
  avif: "AVIF",
  oxipng: "OxiPNG",
  webp: "WebP",
  jpeg_xl: "JPEG XL",
  png: "PNG",
  farbfeld: "Farbfeld",
  ppm: "PPM",
  qoi: "QOI",
};

const OPTIONLESS_DESCRIPTION_KEYS: Partial<Record<EncoderKind, string>> = {
  jpeg_xl: "jpegXlDescription",
  png: "pngDescription",
  farbfeld: "farbfeldDescription",
  ppm: "ppmDescription",
  qoi: "qoiDescription",
};

export default function EncoderTabs({
  capabilities,
  syncStatus,
  lastError,
}: {
  capabilities: readonly EncoderCapability[] | null;
  syncStatus: BackendSyncStatus;
  lastError: string | null;
}) {
  const { t } = useTranslation();
  const draft = useSnapshot(createTaskDraft);

  if (!capabilities) {
    const failed =
      syncStatus === "unavailable" || syncStatus === "needs_resync";
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center rounded-lg border px-6 text-center text-xs text-muted-foreground">
        <div className="flex max-w-sm flex-col gap-1">
          <p>
            {failed
              ? t("encoderCapabilitiesUnavailable")
              : t("loadingEncoderCapabilities")}
          </p>
          {failed && lastError ? (
            <p className="text-[0.65rem] text-destructive/80" title={lastError}>
              {lastError}
            </p>
          ) : null}
        </div>
      </div>
    );
  }

  // Single source of truth: only render the draft's encoder. Parent aligns
  // draft.activeEncoder when capabilities make the current choice unavailable.
  const selectedCapability = capabilities.find(
    (capability) =>
      capability.kind === draft.activeEncoder && capability.available,
  );
  const hasAvailableEncoder = capabilities.some(
    (capability) => capability.available,
  );

  if (!selectedCapability) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center rounded-lg border px-6 text-center text-xs text-muted-foreground">
        {hasAvailableEncoder
          ? t("aligningEncoderSelection")
          : t("noEncodersAvailable")}
      </div>
    );
  }

  return (
    <Tabs
      value={selectedCapability.kind}
      className="min-h-0 w-full gap-1 overflow-hidden"
      onValueChange={(value) => {
        const kind = value as EncoderKind;
        if (!capabilities.some((item) => item.kind === kind && item.available)) {
          return;
        }
        createTaskDraft.activeEncoder = kind;
        markCreateTaskDirty();
      }}
    >
      <TabsList className="my-1 grid w-full auto-rows-[1.5rem] grid-cols-[repeat(auto-fit,minmax(4.5rem,1fr))] gap-0.5 group-data-horizontal/tabs:h-auto">
        {capabilities.map((capability) => (
          <TabsTrigger
            key={capability.kind}
            value={capability.kind}
            disabled={!capability.available}
            title={
              capability.available
                ? ENCODER_LABELS[capability.kind]
                : t("encoderUnavailable")
            }
            className="h-6 min-w-0 px-1 text-[0.7rem] disabled:pointer-events-auto disabled:cursor-not-allowed"
          >
            {ENCODER_LABELS[capability.kind]}
          </TabsTrigger>
        ))}
      </TabsList>
      <TabsContent
        value={selectedCapability.kind}
        className="min-h-0 overflow-hidden"
      >
        <EncoderPanelFor capability={selectedCapability} />
      </TabsContent>
    </Tabs>
  );
}

function EncoderPanelFor({
  capability,
}: {
  capability: EncoderCapability;
}) {
  const { t } = useTranslation();
  const { outputExtensions: extensions } = capability;

  switch (capability.kind) {
    case "mozjpeg":
      return <MozjpegTab extensions={extensions} />;
    case "jpeg":
      return <JpegTab extensions={extensions} />;
    case "avif":
      return <AvifTab extensions={extensions} />;
    case "oxipng":
      return <OxiPngTab extensions={extensions} />;
    case "webp":
      return <WebPTab extensions={extensions} />;
    case "jpeg_xl":
    case "png":
    case "farbfeld":
    case "ppm":
    case "qoi":
      return (
        <OptionlessEncoderPanel
          description={t(OPTIONLESS_DESCRIPTION_KEYS[capability.kind]!)}
          extensions={extensions}
        />
      );
  }
}
