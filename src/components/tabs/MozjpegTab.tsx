import { MozjpegColorSpaceType, TaskConfig } from "@/lib/type";
import { useEffect } from "react";
import TabCard from "./TabCard";
import { useTranslation } from "react-i18next";
import { Lightbulb } from "lucide-react";
import { Badge } from "../ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export default function MozjpegTab({ config }: { config: TaskConfig }) {
  const { t } = useTranslation();
  useEffect(() => {
    if (!config.encodeConfig) {
      config.encodeConfig = {
        mozjpeg: {
          quality: 75,
          progressive: true,
          optimizeCoding: true,
          smoothing: 0,
          colorSpace: MozjpegColorSpaceType.JCS_YCbCr,
          trellisMultipass: false,
        },
      };
    }
  }, []);
  return (
    <div className="w-full flex flex-col gap-1">
      <div className="w-full h-5 flex items-center mx-2 gap-1">
        <Lightbulb size={16} className="text-muted-foreground -rotate-12" />
        <p className="text-muted-foreground text-[0.775rem] -mt-0.5">
          {t("mozjpegDescription")}
        </p>
        <Badge className="text-[0.6rem] py-0 px-1.5">REC.</Badge>
        <Badge className="text-[0.6rem] py-0 px-1.5">Small</Badge>
      </div>
      <div className="w-full flex gap-2 flex-wrap">
        <TabCard
          title={t("quality")}
          defaultValue={config.encodeConfig?.mozjpeg?.quality}
          min={1}
          max={100}
        />
        <TabCard
          title={t("smoothing")}
          defaultValue={config.encodeConfig?.mozjpeg?.smoothing}
        />
        <TabCard title={t("progressive")} variant="boolean" />
        <TabCard title={t("optimizeCoding")} variant="boolean" />
        <TabCard title={t("colorSpace")} variant="none">
          <Select
            defaultValue={config.encodeConfig?.mozjpeg?.colorSpace.toString()}
            onValueChange={(v) => {
              if (config.encodeConfig && config.encodeConfig.mozjpeg) {
                config.encodeConfig.mozjpeg.colorSpace = parseInt(v);
              }
            }}
          >
            <SelectTrigger className="h-6 px-1.5 my-1 text-[0.775rem]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={MozjpegColorSpaceType.JCS_YCbCr.toString()}>
                JCS YCbCr
              </SelectItem>
            </SelectContent>
          </Select>
        </TabCard>
      </div>
    </div>
  );
}

