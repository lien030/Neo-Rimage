import { desktopDir } from "@tauri-apps/api/path";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";

import { CompactSwitch, InlineHelp } from "./DraftControls";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createTaskDraft,
  createTaskUiState,
  updateCreateTaskDraft,
} from "@/features/create-task";
import type { CollisionPolicy } from "@/lib/ipc/contracts";

interface OutputSettingsCardProps {
  isSubmitting: boolean;
  canCreate: boolean;
  onCancel: () => void;
  onCreate: () => void | Promise<void>;
}

export function OutputSettingsCard({
  isSubmitting,
  canCreate,
  onCancel,
  onCreate,
}: OutputSettingsCardProps) {
  const { t } = useTranslation();
  const output = useSnapshot(createTaskDraft).output;
  const usesCustomDirectory = output.locationMode === "directory";
  const canBackup = output.collision === "replace";

  async function ensureDesktopDefaultDirectory() {
    if (createTaskDraft.output.outputDirectory.trim()) {
      return;
    }

    try {
      const path = await desktopDir();
      if (
        createTaskUiState.isOpen &&
        !createTaskUiState.isSubmitting &&
        !createTaskDraft.output.outputDirectory.trim()
      ) {
        // This is a system-provided default, not a user edit.
        createTaskDraft.output.outputDirectory = path;
      }
    } catch {
      // Keep the field empty if the platform cannot resolve a desktop path.
    }
  }

  async function handleSelectOutputDirectory() {
    if (isSubmitting || !usesCustomDirectory) {
      return;
    }

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: output.outputDirectory || undefined,
      });

      // The native picker can resolve after submit, reset, or close.
      if (
        !createTaskUiState.isOpen ||
        createTaskUiState.isSubmitting ||
        typeof selected !== "string"
      ) {
        return;
      }

      updateCreateTaskDraft((draft) => {
        draft.output.locationMode = "directory";
        draft.output.outputDirectory = selected;
      });
    } catch {
      if (createTaskUiState.isOpen && !createTaskUiState.isSubmitting) {
        createTaskUiState.globalError = t("createTaskErrorDirectoryPicker");
      }
    }
  }

  return (
    <div className="flex min-w-0 flex-col gap-0.5 rounded-lg border px-3 py-1.5">
      <p className="text-sm font-bold">{t("output")}</p>
      <div className="grid min-w-0 grid-cols-[clamp(7rem,14vw,8rem)_minmax(0,1fr)_max-content] gap-2">
        <Select
          value={output.locationMode}
          disabled={isSubmitting}
          onValueChange={(locationMode: "same_directory" | "directory") => {
            updateCreateTaskDraft((draft) => {
              draft.output.locationMode = locationMode;
            });
            if (locationMode === "directory") {
              void ensureDesktopDefaultDirectory();
            }
          }}
        >
          <SelectTrigger size="sm" className="px-2 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="same_directory">{t("sameDirectory")}</SelectItem>
            <SelectItem value="directory">{t("customDirectory")}</SelectItem>
          </SelectContent>
        </Select>
        <Input
          value={output.outputDirectory}
          disabled={!usesCustomDirectory || isSubmitting}
          placeholder={t("outputDirectory")}
          className="h-7 min-w-0 px-2 text-xs"
          onChange={(event) =>
            updateCreateTaskDraft((draft) => {
              draft.output.outputDirectory = event.target.value;
            })
          }
        />
        <Button
          type="button"
          variant="outline"
          className="h-7 whitespace-nowrap px-2 text-xs"
          disabled={!usesCustomDirectory || isSubmitting}
          onClick={handleSelectOutputDirectory}
        >
          {t("selectDirectory")}
        </Button>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="flex min-w-0 flex-col">
          <span className="flex items-center gap-1 text-[0.7rem]">
            <span>{t("collisionPolicy")}</span>
            <InlineHelp description={t("collisionPolicyDescription")} />
          </span>
          <Select
            value={output.collision}
            onValueChange={(collision: CollisionPolicy) =>
              updateCreateTaskDraft((draft) => {
                draft.output.collision = collision;
                if (collision !== "replace") {
                  draft.output.sourceBackup = false;
                  draft.output.existingOutputBackup = false;
                }
              })
            }
          >
            <SelectTrigger size="sm" className="px-2 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="fail">{t("collisionFail")}</SelectItem>
              <SelectItem value="replace">{t("collisionReplace")}</SelectItem>
              <SelectItem value="auto_rename">
                {t("collisionAutoRename")}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <label className="flex min-w-0 flex-col text-[0.7rem]">
          <span>{t("suffix")}</span>
          <Input
            value={output.suffix}
            placeholder={t("suffix")}
            className="h-7 px-2 text-xs"
            onChange={(event) =>
              updateCreateTaskDraft((draft) => {
                draft.output.suffix = event.target.value;
              })
            }
          />
        </label>
      </div>
      <div className="grid grid-cols-2 gap-x-3 gap-y-0.5">
        <CompactSwitch
          label={t("preserveStructure")}
          checked={output.preserveStructure}
          disabled={!usesCustomDirectory}
          onCheckedChange={(checked) =>
            updateCreateTaskDraft((draft) => {
              draft.output.preserveStructure = checked;
            })
          }
        />
        <CompactSwitch
          label={t("sourceBackup")}
          checked={output.sourceBackup}
          disabled={!canBackup}
          onCheckedChange={(checked) =>
            updateCreateTaskDraft((draft) => {
              draft.output.sourceBackup = checked;
            })
          }
        />
        <CompactSwitch
          label={t("existingOutputBackup")}
          checked={output.existingOutputBackup}
          disabled={!canBackup}
          onCheckedChange={(checked) =>
            updateCreateTaskDraft((draft) => {
              draft.output.existingOutputBackup = checked;
            })
          }
        />
      </div>
      <div className="mt-auto flex justify-end gap-2">
        <Button
          type="button"
          variant="outline"
          className="h-7 min-w-14"
          disabled={isSubmitting}
          onClick={onCancel}
        >
          {t("cancel")}
        </Button>
        <Button
          type="button"
          className="h-7 min-w-14"
          disabled={!canCreate}
          onClick={onCreate}
        >
          {isSubmitting ? t("creatingTask") : t("create")}
        </Button>
      </div>
    </div>
  );
}
