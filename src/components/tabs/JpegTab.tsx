import {
  createTaskDraft,
  markCreateTaskDirty,
} from "@/features/create-task";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function JpegTab({
  extensions,
}: {
  extensions: readonly string[];
}) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).jpeg;

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

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
          update(() => {
            createTaskDraft.jpeg.quality = String(value);
          })
        }
      />
      <TabCard
        title={t("progressive")}
        variant="boolean"
        value={config.progressive}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.jpeg.progressive = Boolean(value);
          })
        }
      />
    </EncoderPanel>
  );
}
