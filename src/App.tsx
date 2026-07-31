import { listen } from "@tauri-apps/api/event";
import {
  CookingPot,
  Download,
  Minus,
  Pickaxe,
  Plus,
  ScrollText,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import CreateTaskDialog from "./CreateTaskDialog";
import TaskTable from "./components/TaskTable";
import TitleBar from "./components/TitleBar";
import WorkerList from "./components/WorkerList";
import { Button } from "./components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./components/ui/dropdown-menu";
import {
  formatBackendError,
  setSchedulerPaused,
  setWorkerCount,
  useBackendCommandState,
  useBackendRuntimeState,
  useBackendRuntimeSync,
} from "./features/backend";
import {
  collectImageInputs,
  createTaskInputState,
  createTaskUiState,
  expandDroppedPaths,
  mergeSelectedInputs,
} from "./features/create-task";

const SCAN_DROPPED_DIRECTORIES_RECURSIVELY = true;

function App() {
  const { t } = useTranslation();
  const [dragOverlayVisible, setDragOverlayVisible] = useState(false);
  const backend = useBackendRuntimeState();
  const backendCommands = useBackendCommandState();

  useBackendRuntimeSync();

  const scheduler = backend.snapshot?.scheduler;
  const schedulerRunning = scheduler?.mode === "running";
  const backendReady = backend.syncStatus === "ready" && scheduler !== undefined;
  const schedulerStopState = backendReady && schedulerRunning;
  const minimumConcurrency = backend.capabilities?.concurrency.minimum ?? 1;
  const maximumConcurrency =
    backend.capabilities?.concurrency.maximum ?? scheduler?.maxConcurrency ?? 1;
  const schedulerButtonLabel = !backendReady
    ? "SYNC"
    : schedulerStopState
      ? "STOP"
      : "GO";

  useEffect(() => {
    const listeners = [
      listen(
        "tauri://drag-drop",
        (event: { payload: { paths: string[] } }) => {
          createTaskUiState.isOpen = true;
          setDragOverlayVisible(false);
          if (event.payload) {
            void handleDroppedPaths(event.payload.paths).catch(
              (error: unknown) => {
                toast.error("Unable to add dropped files", {
                  description: formatBackendError(error),
                });
              },
            );
          }
        },
      ),
      listen("tauri://drag-enter", () => {
        if (!createTaskUiState.isOpen) {
          setDragOverlayVisible(true);
        }
      }),
      listen("tauri://drag-leave", () => {
        setDragOverlayVisible(false);
      }),
    ];

    return () => {
      // Listener registration is asynchronous, so cleanup must also handle
      // registrations that resolve after this component has unmounted.
      listeners.forEach((listener) => {
        void listener.then((unlisten) => unlisten());
      });
    };
  }, []);

  function openCreateTaskDialog() {
    createTaskUiState.isOpen = true;
  }

  async function handleDroppedPaths(paths: string[]) {
    const filePaths = await expandDroppedPaths(
      paths,
      SCAN_DROPPED_DIRECTORIES_RECURSIVELY,
    );
    const inputs = await collectImageInputs(filePaths);
    createTaskInputState.files = mergeSelectedInputs(
      createTaskInputState.files,
      inputs,
    );
    createTaskUiState.isOpen = true;
  }

  async function adjustWorkerCount(delta: -1 | 1) {
    if (!scheduler || backendCommands.workerCountPending) {
      return;
    }

    const desiredConcurrency =
      delta > 0
        ? Math.min(scheduler.desiredConcurrency + 1, maximumConcurrency)
        : Math.max(scheduler.desiredConcurrency - 1, minimumConcurrency);
    if (desiredConcurrency === scheduler.desiredConcurrency) {
      return;
    }

    try {
      await setWorkerCount(desiredConcurrency);
    } catch (error: unknown) {
      toast.error(
        delta > 0
          ? "Unable to increase concurrency"
          : "Unable to decrease concurrency",
        { description: formatBackendError(error) },
      );
    }
  }

  async function handleToggleScheduler() {
    if (!scheduler || backendCommands.schedulerPending) {
      return;
    }

    try {
      await setSchedulerPaused(scheduler.mode === "running");
    } catch (error: unknown) {
      toast.error("Unable to update the scheduler", {
        description: formatBackendError(error),
      });
    }
  }

  return (
    <div className="relative h-screen w-screen overflow-hidden border bg-transparent">
      <main className="relative flex h-full min-h-0 w-full flex-col">
        <div className="h-14 shrink-0" />
        <div className="grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_clamp(240px,30vw,360px)]">
          <div className="flex min-h-0 min-w-0 flex-col px-4 pb-4">
            <div className="flex h-9 shrink-0 justify-between">
              <span className="mx-2 flex gap-2">
                <ScrollText
                  className="text-primary"
                  size={22}
                  strokeWidth={1.5}
                />
                <p className="font-bold tracking-wide text-primary">
                  {t("tasks")}
                </p>
              </span>
              <span className="mx-2 flex gap-2">
                <Button
                  size="icon"
                  onClick={openCreateTaskDialog}
                  className="h-7 w-12 rounded-lg border text-muted-foreground"
                >
                  <Plus size={16} className="text-white" />
                </Button>
                <JobStatusMenu disabled={!backendReady} />
              </span>
            </div>
            <TaskTable />
          </div>

          <div className="flex min-h-0 min-w-0 flex-col pb-4 pr-4">
            <div className="grid h-9 shrink-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-1 px-2">
              <span className="flex min-w-0 items-center gap-1 whitespace-nowrap">
                <Pickaxe
                  className="shrink-0 text-primary"
                  size={22}
                  strokeWidth={1.5}
                />
                <p className="truncate font-bold text-primary">
                  {t("workers")}
                </p>
                {scheduler && (
                  <p className="shrink-0 text-xs tabular-nums text-muted-foreground">
                    {scheduler.effectiveConcurrency}/
                    {scheduler.desiredConcurrency}
                  </p>
                )}
              </span>
              <span className="flex shrink-0 gap-1">
                <Button
                  size="icon-sm"
                  className="size-7 rounded-lg border text-muted-foreground"
                  onClick={() => void adjustWorkerCount(1)}
                  disabled={
                    !backendReady ||
                    backendCommands.workerCountPending ||
                    (scheduler?.desiredConcurrency ?? maximumConcurrency) >=
                      maximumConcurrency
                  }
                >
                  <Plus size={16} className="text-white" />
                </Button>
                <Button
                  size="icon-sm"
                  className="size-7 rounded-lg border bg-background text-muted-foreground hover:bg-muted-foreground/10"
                  onClick={() => void adjustWorkerCount(-1)}
                  disabled={
                    !backendReady ||
                    backendCommands.workerCountPending ||
                    (scheduler?.desiredConcurrency ?? minimumConcurrency) <=
                      minimumConcurrency
                  }
                >
                  <Minus size={16} />
                </Button>
              </span>
            </div>
            <WorkerList />
            <Button
              className={`mt-4 shrink-0 transition-all ${
                schedulerRunning
                  ? "border border-red-600 bg-red-50 hover:bg-red-100"
                  : ""
              }`}
              onClick={handleToggleScheduler}
              disabled={
                !backendReady ||
                backendCommands.schedulerPending ||
                scheduler?.mode === "shutting_down"
              }
            >
              <p
                className={`mx-2 text-lg font-bold ${
                  schedulerStopState ? "text-red-600" : ""
                }`}
              >
                {schedulerButtonLabel}
              </p>
            </Button>
          </div>
        </div>

        <CreateTaskDialog />
        {dragOverlayVisible && (
          <DragDropOverlay
            onDismiss={() => {
              setDragOverlayVisible(false);
            }}
          />
        )}
      </main>
      <TitleBar />
    </div>
  );
}

function JobStatusMenu({ disabled }: { disabled: boolean }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          size="icon"
          disabled={disabled}
          className="h-7 w-12 rounded-lg border bg-background text-muted-foreground hover:bg-muted-foreground/10"
        >
          <CookingPot size={16} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem>
          <figure className="flex items-center justify-center">
            <div className="h-2 w-2 rounded-full bg-green-400" />
          </figure>
          <p className="mx-2">Success</p>
        </DropdownMenuItem>
        <DropdownMenuItem>
          <figure className="flex items-center justify-center">
            <div className="h-2 w-2 rounded-full bg-red-400" />
          </figure>
          <p className="mx-2">Error</p>
        </DropdownMenuItem>
        <DropdownMenuItem disabled>
          <p className="mx-4">ClearAll</p>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function DragDropOverlay({ onDismiss }: { onDismiss: () => void }) {
  return (
    <div className="absolute inset-x-0 bottom-0 top-14 z-20 bg-black/10 p-4">
      <div className="relative flex h-full w-full flex-col items-center justify-center rounded-lg border-2 border-dashed bg-white p-4">
        <Button
          variant="ghost"
          className="absolute right-2 top-2 px-2"
          onClick={onDismiss}
        >
          <X size={24} className="text-muted-foreground" />
        </Button>
        <Download
          className="text-muted-foreground"
          size={36}
          strokeWidth={1.6}
        />
        <p className="text-xl font-bold text-muted-foreground">Drop here </p>
      </div>
    </div>
  );
}

export default App;
