import {
  createTaskDraft,
  updateCreateTaskDraft,
} from "@/features/create-task";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel, type EncoderTabProps } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function OxiPngTab({
  extensions,
}: EncoderTabProps) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).oxipng;

  return (
    <EncoderPanel
      description={t("oxipngDescription")}
      extensions={extensions}
    >
      <TabCard
        title={t("interlace")}
        variant="boolean"
        value={config.interlace}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.oxipng.interlace = value;
          })
        }
      />
      <TabCard
        title={t("effort")}
        value={config.effort}
        min={0}
        max={6}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.oxipng.effort = value;
          })
        }
      />
    </EncoderPanel>
  );
}
