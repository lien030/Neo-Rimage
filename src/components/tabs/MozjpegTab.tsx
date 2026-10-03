import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createTaskDraft,
  updateCreateTaskDraft,
} from "@/features/create-task";
import type {
  MozJpegColorSpace,
  MozJpegQuantizationTable,
} from "@/lib/ipc/contracts";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel, type EncoderTabProps } from "./EncoderPanel";
import TabCard from "./TabCard";

const QUANTIZATION_TABLES: readonly {
  value: MozJpegQuantizationTable;
  label: string;
}[] = [
  { value: "ahumada_watson_peterson", label: "Ahumada Watson Peterson" },
  { value: "annex_k", label: "Annex K" },
  { value: "flat", label: "Flat" },
  { value: "klein_silverstein_carney", label: "Klein Silverstein Carney" },
  { value: "msssim", label: "MSSSIM" },
  { value: "n_robidoux", label: "N. Robidoux" },
  { value: "psnr_hvs", label: "PSNR-HVS" },
  { value: "peterson_ahumada_watson", label: "Peterson Ahumada Watson" },
  { value: "watson_taylor_borthwick", label: "Watson Taylor Borthwick" },
];

export default function MozjpegTab({
  extensions,
}: EncoderTabProps) {
  const { t } = useTranslation();
  const config = useSnapshot(createTaskDraft).mozjpeg;

  return (
    <EncoderPanel
      description={t("mozjpegDescription")}
      extensions={extensions}
    >
      <TabCard
        title={t("quality")}
        value={config.quality}
        min={1}
        max={100}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.quality = value;
          })
        }
      />
      <TabCard
        title={t("chromaQuality")}
        value={config.chromaQuality}
        placeholder={t("automatic")}
        min={1}
        max={100}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.chromaQuality = value;
          })
        }
      />
      <TabCard
        title={t("smoothing")}
        value={config.smoothing}
        min={0}
        max={100}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.smoothing = value;
          })
        }
      />
      <TabCard
        title={t("progressive")}
        variant="boolean"
        value={config.progressive}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.progressive = value;
          })
        }
      />
      <TabCard
        title={t("optimizeCoding")}
        variant="boolean"
        value={config.optimizeCoding}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.optimizeCoding = value;
          })
        }
      />
      <TabCard
        title={t("trellisMultipass")}
        variant="boolean"
        value={config.trellisMultipass}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.trellisMultipass = value;
          })
        }
      />
      <TabCard title={t("colorSpace")} variant="none">
        <Select
          value={config.colorSpace}
          onValueChange={(value: MozJpegColorSpace) =>
            updateCreateTaskDraft((draft) => {
              draft.mozjpeg.colorSpace = value;
            })
          }
        >
          <SelectTrigger size="sm" className="my-1 px-1.5 text-[0.775rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="ycbcr">YCbCr</SelectItem>
            <SelectItem value="rgb">RGB</SelectItem>
            <SelectItem value="grayscale">{t("grayscale")}</SelectItem>
          </SelectContent>
        </Select>
      </TabCard>
      <TabCard
        title={t("chromaSubsample")}
        value={config.chromaSubsample}
        placeholder={t("automatic")}
        min={1}
        max={4}
        onValueChange={(value) =>
          updateCreateTaskDraft((draft) => {
            draft.mozjpeg.chromaSubsample = value;
          })
        }
      />
      <TabCard title={t("quantizationTable")} variant="none">
        <Select
          value={config.quantizationTable ?? "default"}
          onValueChange={(value) =>
            updateCreateTaskDraft((draft) => {
              draft.mozjpeg.quantizationTable =
                value === "default"
                  ? null
                  : (value as MozJpegQuantizationTable);
            })
          }
        >
          <SelectTrigger size="sm" className="my-1 px-1.5 text-[0.7rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="default">{t("encoderDefault")}</SelectItem>
            {QUANTIZATION_TABLES.map((table) => (
              <SelectItem key={table.value} value={table.value}>
                {table.value === "flat" ? t("quantizationTableFlat") : table.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </TabCard>
    </EncoderPanel>
  );
}
