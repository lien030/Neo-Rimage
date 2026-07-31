import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { CompactNumberField, CompactSwitch } from "./DraftControls";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  createTaskDraft,
  updateCreateTaskDraft,
} from "@/features/create-task";
import type { ResizeFilter } from "@/lib/ipc/contracts";

const RESIZE_FILTERS: readonly { value: ResizeFilter; label: string }[] = [
  { value: "nearest", label: "Nearest" },
  { value: "bilinear", label: "Bilinear" },
  { value: "hamming", label: "Hamming" },
  { value: "catmull_rom", label: "Catmull-Rom" },
  { value: "mitchell", label: "Mitchell" },
  { value: "lanczos3", label: "Lanczos3" },
];

export function ResizeSettingsCard() {
  const { t } = useTranslation();
  const resize = useSnapshot(createTaskDraft).resize;

  return (
    <div className="flex flex-col gap-1 rounded-lg border px-3 py-2">
      <span className="flex items-center justify-between">
        <p className="text-sm font-bold">{t("resize")}</p>
        <Switch
          size="sm"
          checked={resize.enabled}
          onCheckedChange={(checked) =>
            updateCreateTaskDraft((draft) => {
              draft.resize.enabled = checked;
            })
          }
        />
      </span>
      <CompactNumberField
        label={t("width")}
        value={resize.width}
        disabled={!resize.enabled}
        onChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.resize.width = value;
          })
        }
      />
      <CompactNumberField
        label={t("height")}
        value={resize.height}
        disabled={!resize.enabled}
        onChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.resize.height = value;
          })
        }
      />
      <div className="grid grid-cols-[minmax(0,1fr)_100px] items-center gap-2 text-xs">
        <span className="text-right">{t("filter")}</span>
        <Select
          value={resize.filter}
          disabled={!resize.enabled}
          onValueChange={(filter: ResizeFilter) =>
            updateCreateTaskDraft((draft) => {
              draft.resize.filter = filter;
            })
          }
        >
          <SelectTrigger size="sm" className="min-w-0 px-2 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent align="end" className="w-36 max-w-36">
            {RESIZE_FILTERS.map((filter) => (
              <SelectItem key={filter.value} value={filter.value}>
                {filter.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <CompactSwitch
        label={t("allowUpscale")}
        description={t("allowUpscaleDescription")}
        checked={resize.allowUpscale}
        disabled={!resize.enabled}
        onCheckedChange={(checked) =>
          updateCreateTaskDraft((draft) => {
            draft.resize.allowUpscale = checked;
          })
        }
      />
      <CompactSwitch
        label={t("allowDownscale")}
        description={t("allowDownscaleDescription")}
        checked={resize.allowDownscale}
        disabled={!resize.enabled}
        onCheckedChange={(checked) =>
          updateCreateTaskDraft((draft) => {
            draft.resize.allowDownscale = checked;
          })
        }
      />
    </div>
  );
}
