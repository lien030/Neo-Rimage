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
import type {
  MozJpegColorSpace,
  MozJpegQuantizationTable,
} from "@/lib/ipc/contracts";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { EncoderPanel } from "./EncoderPanel";
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
}: {
  extensions: readonly string[];
}) {
  const { t } = useTranslation();
  const snap = useSnapshot(createTaskDraft);
  const config = snap.mozjpeg;

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

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
          update(() => {
            createTaskDraft.mozjpeg.quality = String(value);
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
          update(() => {
            createTaskDraft.mozjpeg.chromaQuality = String(value);
          })
        }
      />
      <TabCard
        title={t("smoothing")}
        value={config.smoothing}
        min={0}
        max={100}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.mozjpeg.smoothing = String(value);
          })
        }
      />
      <TabCard
        title={t("progressive")}
        variant="boolean"
        value={config.progressive}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.mozjpeg.progressive = Boolean(value);
          })
        }
      />
      <TabCard
        title={t("optimizeCoding")}
        variant="boolean"
        value={config.optimizeCoding}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.mozjpeg.optimizeCoding = Boolean(value);
          })
        }
      />
      <TabCard
        title={t("trellisMultipass")}
        variant="boolean"
        value={config.trellisMultipass}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.mozjpeg.trellisMultipass = Boolean(value);
          })
        }
      />
      <TabCard title={t("colorSpace")} variant="none">
        <Select
          value={config.colorSpace}
          onValueChange={(value: MozJpegColorSpace) =>
            update(() => {
              createTaskDraft.mozjpeg.colorSpace = value;
            })
          }
        >
          <SelectTrigger size="sm" className="my-1 px-1.5 text-[0.775rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="ycbcr">YCbCr</SelectItem>
            <SelectItem value="rgb">RGB</SelectItem>
            <SelectItem value="grayscale">Grayscale</SelectItem>
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
          update(() => {
            createTaskDraft.mozjpeg.chromaSubsample = String(value);
          })
        }
      />
      <TabCard title={t("quantizationTable")} variant="none">
        <Select
          value={config.quantizationTable ?? "default"}
          onValueChange={(value) =>
            update(() => {
              createTaskDraft.mozjpeg.quantizationTable =
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
                {table.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </TabCard>
    </EncoderPanel>
  );
}
