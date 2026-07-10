import {
  createTaskDraft,
  markCreateTaskDirty,
} from "@/features/create-task";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function OxiPngTab({
  extensions,
}: {
  extensions: readonly string[];
}) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).oxipng;

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

  return (
    <EncoderPanel description={t("oxipngDescription")} extensions={extensions}>
      <TabCard
        title={t("interlace")}
        variant="boolean"
        value={config.interlace}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.oxipng.interlace = Boolean(value);
          })
        }
      />
      <TabCard
        title={t("effort")}
        value={config.effort}
        min={0}
        max={6}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.oxipng.effort = String(value);
          })
        }
      />
    </EncoderPanel>
  );
}
