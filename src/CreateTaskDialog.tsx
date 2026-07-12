import {
  appState,
  taskState,
  useAppState,
  useTaskStore,
} from "@/lib/State";
import { backendClient, createJobCommand } from "@/lib/ipc";
import {
  CreateTaskValidationError,
  buildCreateJobRequest,
  createCorrelationId,
  createTaskDraft,
  createTaskUiState,
  markCreateTaskDirty,
  resetCreateTaskDraft,
} from "@/features/create-task";
import { useBackendRuntimeState } from "@/features/backend";
import type {
  BackendCapabilities,
  CollisionPolicy,
  ResizeFilter,
} from "@/lib/ipc/contracts";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/task-dialog";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import EncoderTabs from "@/components/tabs/EncoderTabs";
import { AlertCircle, CircleHelp, Settings, X } from "lucide-react";
import { useEffect, useId, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

export default function CreateTaskDialog() {
  const app = useAppState();
  const taskStore = useTaskStore();
  const ui = useSnapshot(createTaskUiState);
  const draft = useSnapshot(createTaskDraft);
  // Valtio's recursive snapshot type exceeds TypeScript's instantiation depth
  // for the full capability contract. Keep the cast local to this read-only view.
  const backend = useBackendRuntimeState() as unknown as {
    readonly capabilities: BackendCapabilities | null;
    readonly syncStatus:
      | "idle"
      | "syncing"
      | "ready"
      | "needs_resync"
      | "unavailable";
    readonly lastError: string | null;
  };
  const { t } = useTranslation();
  const encoderCapabilities = backend.capabilities?.encoders ?? null;
  const availableEncoderKinds = useMemo(
    () =>
      encoderCapabilities
        ?.filter((capability) => capability.available)
        .map((capability) => capability.kind) ?? [],
    [encoderCapabilities],
  );

  // Single owner of draft.activeEncoder alignment when capabilities change.
  // Do not mark dirty: this is a system correction, not a user edit.
  useEffect(() => {
    if (
      availableEncoderKinds.length > 0 &&
      !availableEncoderKinds.includes(draft.activeEncoder)
    ) {
      createTaskDraft.activeEncoder = availableEncoderKinds[0];
    }
  }, [availableEncoderKinds, draft.activeEncoder]);

  function discardAndClose() {
    if (createTaskUiState.isSubmitting) return;
    taskState.taskCache = [];
    resetCreateTaskDraft();
    createTaskUiState.isOpen = false;
    appState.isShowCreateTask = false;
  }

  function handleRemoveTaskCache(path: string) {
    if (createTaskUiState.isSubmitting) return;
    taskState.taskCache = taskState.taskCache.filter((task) => task.path !== path);
    markCreateTaskDirty();
  }

  async function handleCreate() {
    if (createTaskUiState.isSubmitting) return;

    try {
      createTaskUiState.globalError = null;
      const request = buildCreateJobRequest(
        createTaskDraft,
        taskState.taskCache,
        availableEncoderKinds,
      );
      createTaskUiState.isSubmitting = true;

      await backendClient.createJob(
        createJobCommand(createCorrelationId(), request),
      );

      taskState.taskCache = [];
      resetCreateTaskDraft();
      createTaskUiState.isOpen = false;
      appState.isShowCreateTask = false;
    } catch (error) {
      createTaskUiState.globalError =
        error instanceof CreateTaskValidationError &&
        error.code === "inputs_required"
          ? null
          : createTaskErrorMessage(error, t);
    } finally {
      createTaskUiState.isSubmitting = false;
    }
  }

  return (
    <Dialog
      open={app.isShowCreateTask}
      onOpenChange={(open) => {
        if (open) {
          createTaskUiState.isOpen = true;
          appState.isShowCreateTask = true;
          return;
        }
        discardAndClose();
      }}
    >
      <DialogContent
        className="h-[540px] w-[calc(100%-2rem)] max-w-none overflow-hidden sm:max-w-[calc(100%-2rem)] flex flex-col"
        showCloseButton={!ui.isSubmitting}
        onPointerDownOutside={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
        onOpenAutoFocus={(event) => event.preventDefault()}
        onEscapeKeyDown={(event) => {
          if (ui.isSubmitting) event.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle className="select-none">{t("createTask")}</DialogTitle>
        </DialogHeader>

        <div className="w-full min-h-0 flex-1 grid grid-cols-[200px_minmax(0,1fr)] gap-4">
          <div className="flex flex-col border rounded-lg overflow-x-hidden overflow-y-auto select-none">
            {taskStore.taskCache.length === 0 ? (
              <p className="m-auto px-4 text-center text-xs text-muted-foreground">
                {t("createTaskNoInputs")}
              </p>
            ) : (
              taskStore.taskCache.map((task) => (
                <TaskCard
                  fileName={task.fileName}
                  path={task.path}
                  disabled={ui.isSubmitting}
                  onRemove={handleRemoveTaskCache}
                  key={task.path}
                />
              ))
            )}
          </div>

          <div className="min-w-0 min-h-0 flex flex-col select-none">
            <span className="flex items-center gap-1">
              <Settings size={18} />
              <p className="text-sm font-bold">{t("outputSettings")}</p>
            </span>
            <div className="min-h-0 grid grid-rows-[minmax(0,1fr)_190px] grow gap-3">
              <EncoderTabs
                capabilities={encoderCapabilities}
                syncStatus={backend.syncStatus}
                lastError={backend.lastError}
              />

              <div className="grid grid-cols-[180px_minmax(0,1fr)] gap-3">
                <ResizeCard />
                <OutputCard
                  isSubmitting={ui.isSubmitting}
                  canCreate={
                    !ui.isSubmitting &&
                    availableEncoderKinds.length > 0 &&
                    availableEncoderKinds.includes(draft.activeEncoder)
                  }
                  onCancel={discardAndClose}
                  onCreate={handleCreate}
                />
              </div>
            </div>
          </div>
        </div>

        {ui.globalError && (
          <DialogFooter className="sm:justify-start">
            <div
              role="alert"
              className="flex min-w-0 items-start gap-2 text-xs text-destructive"
            >
              <AlertCircle className="mt-0.5 size-4 shrink-0" />
              <p className="min-w-0 break-words">{ui.globalError}</p>
            </div>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

function ResizeCard() {
  const { t } = useTranslation();
  const snap = useSnapshot(createTaskDraft);
  const resize = snap.resize;

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

  return (
    <div className="border rounded-lg flex flex-col gap-1 px-3 py-2">
      <span className="flex justify-between items-center">
        <p className="text-sm font-bold">{t("resize")}</p>
        <Switch
          size="sm"
          checked={resize.enabled}
          onCheckedChange={(checked) =>
            update(() => {
              createTaskDraft.resize.enabled = checked;
            })
          }
        />
      </span>
      <CompactNumberField
        label={t("width")}
        value={resize.width}
        disabled={!resize.enabled}
        onChange={(value) =>
          update(() => {
            createTaskDraft.resize.width = value;
          })
        }
      />
      <CompactNumberField
        label={t("height")}
        value={resize.height}
        disabled={!resize.enabled}
        onChange={(value) =>
          update(() => {
            createTaskDraft.resize.height = value;
          })
        }
      />
      <Select
        value={resize.filter}
        disabled={!resize.enabled}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.resize.filter = value as ResizeFilter;
          })
        }
      >
        <SelectTrigger className="h-7 px-2 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="nearest">Nearest</SelectItem>
          <SelectItem value="bilinear">Bilinear</SelectItem>
          <SelectItem value="hamming">Hamming</SelectItem>
          <SelectItem value="catmull_rom">Catmull-Rom</SelectItem>
          <SelectItem value="mitchell">Mitchell</SelectItem>
          <SelectItem value="lanczos3">Lanczos3</SelectItem>
        </SelectContent>
      </Select>
      <CompactSwitch
        label={t("allowUpscale")}
        description={t("allowUpscaleDescription")}
        checked={resize.allowUpscale}
        disabled={!resize.enabled}
        onCheckedChange={(checked) =>
          update(() => {
            createTaskDraft.resize.allowUpscale = checked;
          })
        }
      />
      <CompactSwitch
        label={t("allowDownscale")}
        description={t("allowDownscaleDescription")}
        checked={resize.allowDownscale}
        disabled={!resize.enabled}
        onCheckedChange={(checked) =>
          update(() => {
            createTaskDraft.resize.allowDownscale = checked;
          })
        }
      />
    </div>
  );
}

function OutputCard({
  isSubmitting,
  canCreate,
  onCancel,
  onCreate,
}: {
  isSubmitting: boolean;
  canCreate: boolean;
  onCancel: () => void;
  onCreate: () => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const snap = useSnapshot(createTaskDraft);
  const output = snap.output;
  const isDirectory = output.locationMode === "directory";
  const canBackup = output.collision === "replace";

  function update(action: () => void) {
    action();
    markCreateTaskDirty();
  }

  return (
    <div className="grid min-w-0 grid-cols-2 grid-rows-[auto_auto_auto_auto_auto_1fr] gap-x-3 gap-y-1 rounded-lg border px-3 py-2">
      <p className="col-span-2 text-sm font-bold">{t("output")}</p>
      <Select
        value={output.locationMode}
        onValueChange={(value) =>
          update(() => {
            createTaskDraft.output.locationMode = value as
              | "same_directory"
              | "directory";
          })
        }
      >
        <SelectTrigger className="h-7 px-2 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="same_directory">{t("sameDirectory")}</SelectItem>
          <SelectItem value="directory">{t("customDirectory")}</SelectItem>
        </SelectContent>
      </Select>
      <Input
        value={output.outputDirectory}
        disabled={!isDirectory}
        placeholder={t("outputDirectory")}
        className="h-7 px-2 text-xs"
        onChange={(event) =>
          update(() => {
            createTaskDraft.output.outputDirectory = event.target.value;
          })
        }
      />
      <Input
        value={output.suffix}
        placeholder={t("suffix")}
        className="h-7 px-2 text-xs"
        onChange={(event) =>
          update(() => {
            createTaskDraft.output.suffix = event.target.value;
          })
        }
      />
      <Select
        value={output.collision}
        onValueChange={(value) =>
          update(() => {
            const collision = value as CollisionPolicy;
            createTaskDraft.output.collision = collision;
            if (collision !== "replace") {
              createTaskDraft.output.sourceBackup = false;
              createTaskDraft.output.existingOutputBackup = false;
            }
          })
        }
      >
        <SelectTrigger className="h-7 px-2 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="fail">{t("collisionFail")}</SelectItem>
          <SelectItem value="replace">{t("collisionReplace")}</SelectItem>
          <SelectItem value="auto_rename">{t("collisionAutoRename")}</SelectItem>
        </SelectContent>
      </Select>
      <CompactSwitch
        label={t("preserveStructure")}
        checked={output.preserveStructure}
        disabled={!isDirectory}
        onCheckedChange={(checked) =>
          update(() => {
            createTaskDraft.output.preserveStructure = checked;
          })
        }
      />
      <CompactSwitch
        label={t("sourceBackup")}
        checked={output.sourceBackup}
        disabled={!canBackup}
        onCheckedChange={(checked) =>
          update(() => {
            createTaskDraft.output.sourceBackup = checked;
          })
        }
      />
      <CompactSwitch
        label={t("existingOutputBackup")}
        checked={output.existingOutputBackup}
        disabled={!canBackup}
        onCheckedChange={(checked) =>
          update(() => {
            createTaskDraft.output.existingOutputBackup = checked;
          })
        }
      />
      <div className="col-span-2 flex self-end justify-end gap-2 pt-1">
        <Button
          type="button"
          variant="outline"
          className="min-w-14"
          disabled={isSubmitting}
          onClick={onCancel}
        >
          {t("cancel")}
        </Button>
        <Button
          type="button"
          className="min-w-14"
          disabled={!canCreate}
          onClick={onCreate}
        >
          {isSubmitting ? t("creatingTask") : t("create")}
        </Button>
      </div>
    </div>
  );
}

function CompactNumberField({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-xs">
      <span className="grow text-right">{label}</span>
      <Input
        type="number"
        min={1}
        value={value}
        disabled={disabled}
        className="h-7 w-20 px-1 text-right"
        onChange={(event) => onChange(event.target.value)}
      />
      <span>px</span>
    </label>
  );
}

function CompactSwitch({
  label,
  description,
  checked,
  disabled = false,
  onCheckedChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  const switchId = useId();

  return (
    <div className="flex min-w-0 items-center justify-between gap-2 text-[0.7rem]">
      <span className="flex min-w-0 items-center gap-1">
        <label htmlFor={switchId} className="truncate">
          {label}
        </label>
        {description && (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                className="shrink-0 text-muted-foreground/70 transition-colors hover:text-foreground focus-visible:text-foreground"
                aria-label={description}
              >
                <CircleHelp className="size-3" />
              </button>
            </TooltipTrigger>
            <TooltipContent
              side="top"
              sideOffset={6}
              className="max-w-64 leading-relaxed"
            >
              <p>{description}</p>
            </TooltipContent>
          </Tooltip>
        )}
      </span>
      <Switch
        id={switchId}
        size="sm"
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
      />
    </div>
  );
}

function TaskCard({
  fileName,
  path,
  disabled,
  onRemove,
}: {
  fileName: string;
  path: string;
  disabled: boolean;
  onRemove: (path: string) => void;
}) {
  return (
    <div className="w-full h-7 px-2 py-1 grid grid-cols-[minmax(0,1fr)_20px] gap-1 hover:bg-slate-100">
      <Tooltip>
        <TooltipTrigger asChild>
          <p className="w-full whitespace-nowrap overflow-hidden text-ellipsis text-sm">
            {fileName}
          </p>
        </TooltipTrigger>
        <TooltipContent>
          <p>{path}</p>
        </TooltipContent>
      </Tooltip>

      <figure className="flex justify-center items-center">
        <Button
          type="button"
          variant="ghost"
          disabled={disabled}
          className="p-0 m-0 h-full"
          onClick={() => onRemove(path)}
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

function createTaskErrorMessage(
  error: unknown,
  t: (key: string) => string,
): string {
  if (error instanceof CreateTaskValidationError) {
    const keyByCode: Record<CreateTaskValidationError["code"], string> = {
      inputs_required: "createTaskErrorInputsRequired",
      input_path_required: "createTaskErrorInputPath",
      encoder_unsupported: "createTaskErrorEncoderUnsupported",
      quality_invalid: "createTaskErrorQuality",
      chroma_quality_invalid: "createTaskErrorChromaQuality",
      smoothing_invalid: "createTaskErrorSmoothing",
      chroma_subsample_invalid: "createTaskErrorChromaSubsample",
      jpeg_quality_invalid: "createTaskErrorJpegQuality",
      avif_quality_invalid: "createTaskErrorAvifQuality",
      avif_alpha_quality_invalid: "createTaskErrorAvifAlphaQuality",
      avif_speed_invalid: "createTaskErrorAvifSpeed",
      oxipng_effort_invalid: "createTaskErrorOxiPngEffort",
      webp_quality_invalid: "createTaskErrorWebPQuality",
      webp_slight_loss_invalid: "createTaskErrorWebPSlightLoss",
      webp_slight_loss_requires_lossless:
        "createTaskErrorWebPSlightLossRequiresLossless",
      resize_width_invalid: "createTaskErrorResizeWidth",
      resize_height_invalid: "createTaskErrorResizeHeight",
      output_directory_required: "createTaskErrorOutputDirectory",
      suffix_invalid: "createTaskErrorSuffix",
      backup_requires_replace: "createTaskErrorBackupPolicy",
    };
    return t(keyByCode[error.code]);
  }

  if (isRecord(error)) {
    const appError = isRecord(error.error) ? error.error : error;
    if (typeof appError.fallbackMessage === "string") {
      return appError.fallbackMessage;
    }
    if (typeof appError.message === "string") return appError.message;
  }
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string") return error;
  return t("createTaskErrorUnknown");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
