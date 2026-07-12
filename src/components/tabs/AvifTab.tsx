import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createTaskDraft,
  markCreateTaskDirty,
} from "@/features/create-task";
import type { AvifAlphaMode, AvifColorSpace } from "@/lib/ipc/contracts";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel } from "./EncoderPanel";
import TabCard from "./TabCard";

export default function AvifTab({
  extensions,
}: {
  extensions: readonly string[];
}) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).avif;

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

  return (
    <EncoderPanel
      description={t("avifDescription")}
      extensions={extensions}
    >
      <TabCard
        title={t("quality")}
        value={config.quality}
        min={1}
        max={100}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.avif.quality = String(value);
          })
        }
      />
      <TabCard
        title={t("alphaQuality")}
        value={config.alphaQuality}
        placeholder={t("automatic")}
        min={1}
        max={100}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.avif.alphaQuality = String(value);
          })
        }
      />
      <TabCard
        title={t("speed")}
        value={config.speed}
        min={1}
        max={10}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.avif.speed = String(value);
          })
        }
      />
      <TabCard title={t("colorSpace")} variant="none">
        <Select
          value={config.colorSpace}
          onValueChange={(value: AvifColorSpace) =>
            update(() => {
              createTaskDraft.avif.colorSpace = value;
            })
          }
        >
          <SelectTrigger className="my-1 h-6 px-1.5 text-[0.775rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="ycbcr">YCbCr</SelectItem>
            <SelectItem value="rgb">RGB</SelectItem>
          </SelectContent>
        </Select>
      </TabCard>
      <TabCard title={t("alphaMode")} variant="none">
        <Select
          value={config.alphaMode}
          onValueChange={(value: AvifAlphaMode) =>
            update(() => {
              createTaskDraft.avif.alphaMode = value;
            })
          }
        >
          <SelectTrigger className="my-1 h-6 px-1.5 text-[0.67rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="unassociated_dirty">
              {t("alphaModeUnassociatedDirty")}
            </SelectItem>
            <SelectItem value="unassociated_clean">
              {t("alphaModeUnassociatedClean")}
            </SelectItem>
            <SelectItem value="premultiplied">
              {t("alphaModePremultiplied")}
            </SelectItem>
          </SelectContent>
        </Select>
      </TabCard>
    </EncoderPanel>
  );
}
