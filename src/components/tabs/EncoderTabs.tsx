import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  createTaskDraft,
  markCreateTaskDirty,
} from "@/features/create-task";
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
}: {
  capabilities: readonly EncoderCapability[] | null;
}) {
  const { t } = useTranslation();
  const draft = useSnapshot(createTaskDraft);

  if (!capabilities) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center rounded-lg border text-xs text-muted-foreground">
        {t("loadingEncoderCapabilities")}
      </div>
    );
  }

  const activeCapability = capabilities.find(
    (capability) =>
      capability.kind === draft.activeEncoder && capability.available,
  );
  const fallbackCapability = capabilities.find(
    (capability) => capability.available,
  );
  const selectedCapability = activeCapability ?? fallbackCapability;

  if (!selectedCapability) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center rounded-lg border px-6 text-center text-xs text-muted-foreground">
        {t("noEncodersAvailable")}
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
      <TabsList className="my-1 grid w-full grid-cols-5 gap-0.5 group-data-horizontal/tabs:h-14">
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

  switch (capability.kind) {
    case "mozjpeg":
      return <MozjpegTab extensions={capability.outputExtensions} />;
    case "jpeg":
      return <JpegTab extensions={capability.outputExtensions} />;
    case "avif":
      return <AvifTab extensions={capability.outputExtensions} />;
    case "oxipng":
      return <OxiPngTab extensions={capability.outputExtensions} />;
    case "webp":
      return <WebPTab extensions={capability.outputExtensions} />;
    case "jpeg_xl":
    case "png":
    case "farbfeld":
    case "ppm":
    case "qoi":
      return (
        <OptionlessEncoderPanel
          description={t(OPTIONLESS_DESCRIPTION_KEYS[capability.kind]!)}
          extensions={capability.outputExtensions}
        />
      );
  }
}
