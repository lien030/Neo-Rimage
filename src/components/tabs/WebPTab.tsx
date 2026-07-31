import {
  createTaskDraft,
  updateCreateTaskDraft,
} from "@/features/create-task";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel, type EncoderTabProps } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function WebPTab({
  extensions,
}: EncoderTabProps) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).webp;

  return (
    <EncoderPanel
      description={t("webpDescription")}
      extensions={extensions}
    >
      <TabCard
        title={t("lossless")}
        variant="boolean"
        value={config.lossless}
        onValueChange={(lossless) =>
          updateCreateTaskDraft((draft) => {
            draft.webp.lossless = lossless;
            if (!lossless) {
              draft.webp.slightLoss = "0";
            }
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
          updateCreateTaskDraft((draft) => {
            draft.webp.quality = value;
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
          updateCreateTaskDraft((draft) => {
            draft.webp.slightLoss = value;
          })
        }
      />
      <TabCard
        title={t("preserveTransparentRgb")}
        variant="boolean"
        value={config.exact}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.webp.exact = value;
          })
        }
      />
    </EncoderPanel>
  );
}
