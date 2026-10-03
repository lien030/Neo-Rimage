import { AlertCircle, Settings } from "lucide-react";
import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ref, useSnapshot } from "valtio";

import { OutputSettingsCard } from "@/components/create-task/OutputSettingsCard";
import { ResizeSettingsCard } from "@/components/create-task/ResizeSettingsCard";
import { TaskInputList } from "@/components/create-task/TaskInputList";
import { createTaskErrorMessage } from "@/components/create-task/create-task-error";
import EncoderTabs from "@/components/tabs/EncoderTabs";
import { describeEncoderLimitation } from "@/components/tabs/encoder-limitations";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { BackendSyncStatus } from "@/features/backend";
import { formatBackendError, useBackendRuntimeState } from "@/features/backend";
import {
  CreateTaskValidationError,
  buildCreateJobRequest,
  createCorrelationId,
  createTaskDraft,
  createTaskInputState,
  createTaskUiState,
  markCreateTaskDirty,
  resetCreateTaskDraft,
  resetCreateTaskInputs,
} from "@/features/create-task";
import { backendClient, createJobCommand } from "@/lib/ipc";
import type { BackendCapabilities } from "@/lib/ipc/contracts";

interface CreateTaskBackendState {
  readonly capabilities: BackendCapabilities | null;
  readonly syncStatus: BackendSyncStatus;
  readonly lastError: string | null;
  readonly lastErrorDetails: unknown;
}

export default function CreateTaskDialog() {
  const { t } = useTranslation();
  const inputState = useSnapshot(createTaskInputState);
  const ui = useSnapshot(createTaskUiState);
  const draft = useSnapshot(createTaskDraft);

  // Valtio recursively snapshots the capability contract deeply enough to hit
  // TypeScript's instantiation limit. Keep the workaround at this UI boundary.
  const backend =
    useBackendRuntimeState() as unknown as CreateTaskBackendState;
  const encoderCapabilities = backend.capabilities?.encoders ?? null;
  const availableEncoderKinds = useMemo(
    () =>
      encoderCapabilities
        ?.filter((capability) => capability.available)
        .map((capability) => capability.kind) ?? [],
    [encoderCapabilities],
  );
  const selectedEncoderLimitationText =
    encoderCapabilities
      ?.find(
        (capability) =>
          capability.kind === draft.activeEncoder && capability.available,
      )
      ?.limitations.map((code) => describeEncoderLimitation(code, t))
      .join(" · ") ?? "";
  const canCreate =
    !ui.isSubmitting &&
    availableEncoderKinds.length > 0 &&
    availableEncoderKinds.includes(draft.activeEncoder);

  useEffect(() => {
    if (
      availableEncoderKinds.length > 0 &&
      !availableEncoderKinds.includes(draft.activeEncoder)
    ) {
      // Capability reconciliation is a system correction, not a user edit.
      createTaskDraft.activeEncoder = availableEncoderKinds[0];
    }
  }, [availableEncoderKinds, draft.activeEncoder]);

  function resetAndClose() {
    resetCreateTaskInputs();
    resetCreateTaskDraft();
    createTaskUiState.isOpen = false;
  }

  function discardAndClose() {
    if (createTaskUiState.isSubmitting) {
      return;
    }
    resetAndClose();
  }

  function handleOpenChange(open: boolean) {
    if (open) {
      createTaskUiState.isOpen = true;
      return;
    }
    discardAndClose();
  }

  function handleRemoveTask(path: string) {
    if (createTaskUiState.isSubmitting) {
      return;
    }
    createTaskInputState.files = createTaskInputState.files.filter(
      (input) => input.path !== path,
    );
    markCreateTaskDirty();
  }

  async function handleCreate() {
    if (createTaskUiState.isSubmitting) {
      return;
    }

    try {
      createTaskUiState.globalError = null;
      const request = buildCreateJobRequest(
        createTaskDraft,
        createTaskInputState.files,
        availableEncoderKinds,
      );
      createTaskUiState.isSubmitting = true;

      await backendClient.createJob(
        createJobCommand(createCorrelationId(), request),
      );

      resetAndClose();
    } catch (error) {
      createTaskUiState.globalError =
        error instanceof CreateTaskValidationError &&
        error.code === "inputs_required"
          ? null
          : error && typeof error === "object"
            ? ref(error)
            : error;
    } finally {
      createTaskUiState.isSubmitting = false;
    }
  }

  return (
    <Dialog open={ui.isOpen} onOpenChange={handleOpenChange}>
      <DialogContent
        className="flex h-[min(90vh,900px)] w-[min(96vw,1440px)] max-w-none flex-col overflow-hidden sm:max-w-none"
        showCloseButton={!ui.isSubmitting}
        onPointerDownOutside={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
        onOpenAutoFocus={(event) => event.preventDefault()}
        onEscapeKeyDown={(event) => {
          if (ui.isSubmitting) {
            event.preventDefault();
          }
        }}
      >
        <DialogHeader>
          <DialogTitle className="select-none">{t("createTask")}</DialogTitle>
        </DialogHeader>

        <div className="grid min-h-0 w-full flex-1 grid-cols-[clamp(200px,22vw,320px)_minmax(0,1fr)] gap-4">
          <TaskInputList
            tasks={inputState.files}
            emptyMessage={t("createTaskNoInputs")}
            disabled={ui.isSubmitting}
            onRemove={handleRemoveTask}
          />

          <div className="flex min-h-0 min-w-0 select-none flex-col">
            <div className="flex h-5 min-w-0 items-center gap-1">
              <Settings className="shrink-0" size={18} />
              <p className="shrink-0 text-sm font-bold">
                {t("outputSettings")}
              </p>
              {selectedEncoderLimitationText && (
                <p
                  className="ml-1 mt-0.5 min-w-0 flex-1 truncate text-[0.65rem] text-muted-foreground/80"
                  title={selectedEncoderLimitationText}
                >
                  {selectedEncoderLimitationText}
                </p>
              )}
            </div>
            <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)_190px] gap-3">
              <EncoderTabs
                capabilities={encoderCapabilities}
                syncStatus={backend.syncStatus}
                lastError={
                  backend.lastError
                    ? formatBackendError(backend.lastErrorDetails ?? backend.lastError, t)
                    : null
                }
              />

              <div className="grid min-h-0 min-w-0 grid-cols-[clamp(180px,26%,256px)_minmax(0,1fr)] gap-3">
                <ResizeSettingsCard />
                <OutputSettingsCard
                  isSubmitting={ui.isSubmitting}
                  canCreate={canCreate}
                  onCancel={discardAndClose}
                  onCreate={handleCreate}
                />
              </div>
            </div>
          </div>
        </div>

        {ui.globalError !== null && (
          <DialogFooter className="max-h-12 shrink-0 overflow-y-auto pr-1 sm:justify-start">
            <div
              role="alert"
              className="flex min-w-0 items-start gap-2 text-xs text-destructive"
            >
              <AlertCircle className="mt-0.5 size-4 shrink-0" />
              <p className="min-w-0 break-words">{createTaskErrorMessage(ui.globalError, t)}</p>
            </div>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
