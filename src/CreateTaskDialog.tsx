import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/task-dialog";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { useAppState, useTaskStore } from "./lib/State";
import { useTranslation } from "react-i18next";
import { Button } from "./components/ui/button";
import { Settings, X } from "lucide-react";
import { Switch } from "./components/ui/switch";
import {
  EncoderType,
  ResizeFilterType,
  TaskConfig,
} from "./lib/type";
import { proxy, useSnapshot } from "valtio";
import { Input } from "./components/ui/input";
import { Checkbox } from "./components/ui/checkbox";
import MozjpegTab from "./components/tabs/MozjpegTab";

export default function CreateTaskDialog() {
  const appState = useAppState();
  const taskStore = useTaskStore();
  const { t } = useTranslation();

  const defConfig: TaskConfig = proxy({
    filePath: "",
    fileName: "",
    encoder: EncoderType.Mozjpeg,
    resize: false,
    resizeConfig: {
      firstly: false,
      width: 100,
      height: 100,
      filter: ResizeFilterType.Lanczos3,
    },
    suffix: "",
    recursive: false,
    backup: false,
  });

  function handleRemoveTaskCache(fileName: string) {
    taskStore.taskCache = taskStore.taskCache.filter(
      (task) => task.fileName !== fileName
    );
  }

  function ResizeCard() {
    const snap = useSnapshot(defConfig);
    return (
      <div className="border rounded-lg flex flex-col gap-1">
        <span className="w-full px-3 py-2 flex justify-between items-center">
          <p className="text-sm font-bold">{t("resize")}</p>
          <Switch
            size="sm"
            defaultChecked={defConfig.resize}
            onCheckedChange={(checked) => {
              defConfig.resize = checked;
            }}
          />
        </span>
        <span className="flex justify-center items-center text-[0.8rem] gap-2 px-3">
          <p className="grow text-right">{t("width")}</p>
          <Input
            type="number"
            defaultValue={defConfig.resizeConfig?.width}
            className="h-7 w-20 px-1 text-right"
            disabled={!snap.resize}
            onBlur={(e) => {
              if (defConfig.resizeConfig) {
                defConfig.resizeConfig.width = parseInt(e.target.value);
              }
            }}
          />
          <p className="">px</p>
        </span>
        <span className="flex justify-center items-center text-[0.8rem] gap-2 px-3">
          <p className="grow text-right">{t("height")}</p>
          <Input
            defaultValue={defConfig.resizeConfig?.height}
            type="number"
            className="h-7 w-20 px-1 text-right"
            disabled={!snap.resize}
            onBlur={(e) => {
              if (defConfig.resizeConfig) {
                defConfig.resizeConfig.height = parseInt(e.target.value);
              }
            }}
          />
          <p className="">px</p>
        </span>
        <span className="flex justify-center items-center text-[0.8rem] gap-2 px-3">
          <p className="grow text-right text-nowrap">{t("filter")}</p>
          <Select
            defaultValue={ResizeFilterType.Lanczos3.toString()}
            onValueChange={(v) => {
              if (defConfig.resizeConfig) {
                defConfig.resizeConfig.filter = parseInt(v);
              }
            }}
            disabled={!snap.resize}
          >
            <SelectTrigger className="h-7 w-[102px] px-2">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ResizeFilterType.Lanczos3.toString()}>
                Lanczos3
              </SelectItem>
              <SelectItem value={ResizeFilterType.Nearest.toString()}>
                Nearest
              </SelectItem>
            </SelectContent>
          </Select>
        </span>
        <span className="flex justify-end items-center text-[0.8rem] gap-2 px-3">
          <Checkbox className="" disabled={!snap.resize} />
          <TooltipProvider delayDuration={300}>
            <Tooltip>
              <TooltipTrigger asChild>
                <p className="w-[102px] underline underline-offset-2">
                  {t("resizeFirstly")}
                </p>
              </TooltipTrigger>
              <TooltipContent>{t("resizeFirstlyDescription")}</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </span>
      </div>
    );
  }

  return (
    <Dialog
      open={appState.isShowCreateTask}
      onOpenChange={(open) => {
        appState.isShowCreateTask = open;
        if (!open) {
          taskStore.taskCache = [];
        }
      }}
    >
      <DialogContent
        className="h-[500px] w-[calc(100%-2rem)] max-w-none overflow-hidden sm:max-w-[calc(100%-2rem)] flex flex-col"
        onPointerDownOutside={(e) => e.preventDefault()}
        onInteractOutside={(e) => e.preventDefault()}
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle className="select-none">{t("createTask")}</DialogTitle>
        </DialogHeader>
        <div className="w-full min-h-0 flex-1 grid grid-cols-[200px_minmax(0,1fr)] gap-4">
          <div className="flex flex-col border rounded-lg overflow-x-hidden overflow-y-auto select-none">
            {taskStore.taskCache.map((task) => {
              return (
                <TaskCard
                  fileName={task.fileName}
                  onRemove={handleRemoveTaskCache}
                  key={task.path}
                />
              );
            })}
          </div>
          <div className="min-w-0 min-h-0 flex flex-col select-none">
            <span className="flex items-center gap-1">
              <Settings size={18} />
              <p className="text-sm font-bold">{t("outputSettings")}</p>
            </span>
            <div className="min-h-0 grid grid-rows-[minmax(0,1fr)_170px] grow gap-4">
              <Tabs
                defaultValue={defConfig.encoder.toString()}
                className="w-full min-h-0 overflow-hidden"
              >
                <TabsList className="w-full h-8 my-1 justify-start overflow-x-auto overflow-y-hidden">
                  <TabsTrigger value={EncoderType.Mozjpeg.toString()}>
                    Mozjpeg
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Jpeg.toString()}>
                    JPEG
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Jpeg_xl.toString()}>
                    JPEG XL
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Oxipng.toString()}>
                    Oxipng
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Png.toString()}>
                    PNG
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Webp.toString()}>
                    Webp
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Ppm.toString()}>
                    PPM
                  </TabsTrigger>
                  <TabsTrigger value={EncoderType.Qoi.toString()}>
                    QOI
                  </TabsTrigger>
                </TabsList>
                <TabsContent value={EncoderType.Mozjpeg.toString()}>
                  <MozjpegTab config={defConfig} />
                </TabsContent>
                <TabsContent value="password">
                  Change your password here.
                </TabsContent>
              </Tabs>
              <div className="grid grid-cols-[180px_auto] gap-4">
                <ResizeCard />
                <div className=" border"></div>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function TaskCard({
  fileName,
  onRemove,
}: {
  fileName: string;
  onRemove: (fileName: string) => void;
}) {
  return (
    <div className="w-full h-6 px-2 py-1 grid grid-cols-[auto_20px] gap-1 hover:bg-slate-100">
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <p className="w-full whitespace-nowrap overflow-hidden text-ellipsis text-sm">
              {fileName}
            </p>
          </TooltipTrigger>
          <TooltipContent>
            <p>{fileName}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <figure className="flex justify-center items-center">
        <Button
          variant={"ghost"}
          className="p-0 m-0 h-full"
          onClick={() => {
            onRemove(fileName);
          }}
        >
          <X
            size={16}
            className="text-muted-foreground/50 hover:text-muted-foreground"
          />
        </Button>
      </figure>
    </div>
  );
}
