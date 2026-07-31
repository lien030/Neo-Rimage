import {
  createTaskDraft,
  updateCreateTaskDraft,
} from "@/features/create-task";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel, type EncoderTabProps } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function JpegTab({
  extensions,
}: EncoderTabProps) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).jpeg;

  return (
    <EncoderPanel
      description={t("jpegDescription")}
      extensions={extensions}
    >
      <TabCard
        title={t("quality")}
        value={config.quality}
        min={1}
        max={100}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.jpeg.quality = value;
          })
        }
      />
      <TabCard
        title={t("progressive")}
        variant="boolean"
        value={config.progressive}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.jpeg.progressive = value;
          })
        }
      />
    </EncoderPanel>
  );
}
