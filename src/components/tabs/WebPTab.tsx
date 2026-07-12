import {
  createTaskDraft,
  markCreateTaskDirty,
} from "@/features/create-task";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function WebPTab({
  extensions,
}: {
  extensions: readonly string[];
}) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).webp;

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

  return (
    <EncoderPanel description={t("webpDescription")} extensions={extensions}>
      <TabCard
        title={t("lossless")}
        variant="boolean"
        value={config.lossless}
        onValueChange={(value) =>
          update(() => {
            const lossless = Boolean(value);
            createTaskDraft.webp.lossless = lossless;
            if (!lossless) createTaskDraft.webp.slightLoss = "0";
          })
        }
      />
      <TabCard
        title={t("quality")}
        value={config.quality}
        min={1}
        max={100}
        disabled={config.lossless}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.webp.quality = String(value);
          })
        }
      />
      <TabCard
        title={t("slightLoss")}
        value={config.slightLoss}
        min={0}
        max={100}
        disabled={!config.lossless}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.webp.slightLoss = String(value);
          })
        }
      />
      <TabCard
        title={t("preserveTransparentRgb")}
        variant="boolean"
        value={config.exact}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.webp.exact = Boolean(value);
          })
        }
      />
    </EncoderPanel>
  );
}
