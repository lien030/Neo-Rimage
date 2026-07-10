import { useEffect } from "react";
import { Button } from "./components/ui/button";
import TitleBar from "./components/TitleBar";
import {
  useAppState,
  useBackendCommandState,
  useBackendRuntimeState,
  useTaskStore,
} from "./lib/State";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useTranslation } from "react-i18next";
import TaskTable from "./components/TaskTable";
import { createTaskList, fileFilter, mergeTask } from "./lib/appUtils";
import WorkerList from "./components/WorkerList";
import CreateTaskDialog from "./CreateTaskDialog";
import {
  formatBackendError,
  setSchedulerPaused,
  setWorkerCount,
  useBackendRuntimeSync,
} from "@/features/backend";
import { toast } from "sonner";

function App() {
  const { t } = useTranslation();
  const appState = useAppState();
  const taskStore = useTaskStore();
  const backend = useBackendRuntimeState();
  const backendCommands = useBackendCommandState();

  useBackendRuntimeSync();

  const scheduler = backend.snapshot?.scheduler;
  const schedulerRunning = scheduler?.mode === "running";
  const backendReady = backend.syncStatus === "ready" && scheduler !== undefined;
  const minimumConcurrency = backend.capabilities?.concurrency.minimum ?? 1;
  const maximumConcurrency =
    backend.capabilities?.concurrency.maximum ?? scheduler?.maxConcurrency ?? 1;

  useEffect(() => {
    const dragDrop = listen(
      "tauri://drag-drop",
      (e: { payload: { paths: string[] } }) => {
        appState.isShowCreateTask = true;
        appState.isShowDragDrop = false;
        if (e.payload) {
          handleDragDrop(e.payload.paths);
        }
      }
    );
    const dragEnter = listen("tauri://drag-enter", () => {
      if (appState.isShowCreateTask) return;
      appState.isShowDragDrop = true;
    });
    const dragLeave = listen("tauri://drag-leave", () => {
      appState.isShowDragDrop = false;
    });

    return () => {
      dragDrop.then((unlisten) => {
        unlisten();
      });
      dragEnter.then((dragEnter) => {
        dragEnter();
      });
      dragLeave.then((dragLeave) => {
        dragLeave();
      });
    };
  }, []);

  function DragDropActive() {
    return (
      <div className="absolute w-full h-full pt-14 bg-black/10 border flex justify-center items-center gap-2">
        <div className="w-[94%] h-[90%] bg-white p-4 border-2 border-dashed rounded-lg flex flex-col justify-center items-center relative">
          <Button
            variant={"ghost"}
            className="absolute right-2 top-2 px-2"
            onClick={() => (appState.isShowDragDrop = false)}
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

  function handleInputFile() {
    appState.isShowCreateTask = true;
  }

  async function handleDragDrop(paths: string[]) {
    const filepaths = await fileFilter(paths, appState.recursiveFolders);
    const tasklist = await createTaskList(filepaths);
    taskStore.taskCache = mergeTask(taskStore.taskCache, tasklist);
    appState.isShowCreateTask = true;
  }

  async function handleAddWorker() {
    if (!scheduler || backendCommands.workerCountPending) {
      return;
    }

    const desired = Math.min(
      scheduler.desiredConcurrency + 1,
      maximumConcurrency,
    );
    if (desired === scheduler.desiredConcurrency) {
      return;
    }

    try {
      await setWorkerCount(desired);
    } catch (error: unknown) {
      toast.error("Unable to increase concurrency", {
        description: formatBackendError(error),
      });
    }
  }

  async function handleRemoveWorker() {
    if (!scheduler || backendCommands.workerCountPending) {
      return;
    }

    const desired = Math.max(
      scheduler.desiredConcurrency - 1,
      minimumConcurrency,
    );
    if (desired === scheduler.desiredConcurrency) {
      return;
    }

    try {
      await setWorkerCount(desired);
    } catch (error: unknown) {
      toast.error("Unable to decrease concurrency", {
        description: formatBackendError(error),
      });
    }
  }

  async function handleBreaker() {
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
    <div className="inset-0 w-screen h-screen bg-transparent border relative overflow-hidden">
      <main className="h-full w-full flex flex-col relative">
        <div className="h-14" />
        <div className="w-full grow grid grid-cols-[auto_240px]">
          <div className="px-4 pb-4 flex flex-col">
            <div className="flex justify-between h-9">
              <span className="flex gap-2 mx-2">
                <ScrollText
                  className="text-primary"
                  size={22}
                  strokeWidth={1.5}
                />
                <p className="text-primary font-bold tracking-wide">
                  {t("tasks")}
                </p>
              </span>
              <span className="flex gap-2 mx-2">
                <Button
                  size={"icon"}
                  onClick={handleInputFile}
                  className="text-muted-foreground border h-7 w-12 rounded-lg"
                >
                  <Plus size={16} className="text-white" />
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      size={"icon"}
                      disabled={!backendReady}
                      className="text-muted-foreground bg-background hover:bg-muted-foreground/10 border h-7 w-12 rounded-lg"
                    >
                      <CookingPot size={16} />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent>
                    <DropdownMenuItem>
                      <figure className="flex justify-center items-center">
                        <div
                          className="h-2 w-2 rounded-full bg-green-400"
                        />
                      </figure>
                      <p className="mx-2">Success</p>
                    </DropdownMenuItem>
                    <DropdownMenuItem>
                      <figure className="flex justify-center items-center">
                        <div
                          className="h-2 w-2 rounded-full bg-red-400"
                        />
                      </figure>
                      <p className="mx-2">Error</p>
                    </DropdownMenuItem>
                    <DropdownMenuItem disabled>
                      <p className="mx-4">ClearAll</p>
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </span>
            </div>
            <TaskTable />
          </div>
          <div className="pb-4 pr-4 flex flex-col">
            <div className="flex justify-between h-9">
              <span className="flex gap-2 mx-2">
                <Pickaxe className="text-primary" size={22} strokeWidth={1.5} />
                <p className="text-primary font-bold tracking-wide">
                  {t("workers")}
                </p>
                {scheduler && (
                  <p className="text-xs text-muted-foreground self-center">
                    {scheduler.effectiveConcurrency}/
                    {scheduler.desiredConcurrency}
                  </p>
                )}
              </span>
              <span className="flex gap-2 mx-2">
                <Button
                  size={"icon"}
                  className="text-muted-foreground border h-7 w-12 rounded-lg"
                  onClick={handleAddWorker}
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
                  size={"icon"}
                  onClick={handleRemoveWorker}
                  className="text-muted-foreground bg-background hover:bg-muted-foreground/10 border h-7 w-10 rounded-lg"
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
              className={`mt-4 transition-all ${
                schedulerRunning
                  ? "bg-red-50 hover:bg-red-100 border border-red-600"
                  : ""
              }`}
              onClick={handleBreaker}
              disabled={
                !backendReady ||
                backendCommands.schedulerPending ||
                scheduler?.mode === "shutting_down"
              }
            >
              {!backendReady && (
                <p className="mx-2 font-bold text-lg">SYNC</p>
              )}
              {backendReady && !schedulerRunning && (
                <p className="mx-2 font-bold text-lg">GO</p>
              )}
              {backendReady && schedulerRunning && (
                <p className="mx-2 font-bold text-lg text-red-600">STOP</p>
              )}
            </Button>
          </div>
        </div>
        <CreateTaskDialog />
        {appState.isShowDragDrop && <DragDropActive />}
      </main>
      <TitleBar />
    </div>
  );
}

export default App;
