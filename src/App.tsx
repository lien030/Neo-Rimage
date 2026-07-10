import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "./components/ui/button";
import TitleBar from "./components/TitleBar";
import { generate as generateShortUUID } from "short-uuid";
import { useAppState, useTaskStore, useWorkerList } from "./lib/State";
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
import { WorkerStatusType } from "./lib/type";
import CreateTaskDialog from "./CreateTaskDialog";
function App() {
  const { t } = useTranslation();
  const appState = useAppState();
  const taskStore = useTaskStore();
  const workerList = useWorkerList();

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
    const workerId = generateShortUUID();
    const response = await invoke<boolean>("add_worker", { workerId });
    if (response) {
      workerList.push({
        id: workerId,
        status: WorkerStatusType.Idle,
        task: null,
      });
    }
  }

  async function handleRemoveWorker() {
    if (workerList.length === 0) return;
    const workerId = workerList.pop()?.id;
    if (workerId) {
      await invoke("remove_worker", { workerId: workerId });
    }
  }

  function handleClearAll() {
    taskStore.taskCache = [];
    taskStore.taskList = [];
  }

  function handleBreaker() {
    appState.running = !appState.running;
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
                      disabled={appState.running}
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
                    <DropdownMenuItem onClick={handleClearAll}>
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
              </span>
              <span className="flex gap-2 mx-2">
                <Button
                  size={"icon"}
                  className="text-muted-foreground border h-7 w-12 rounded-lg"
                  onClick={handleAddWorker}
                >
                  <Plus size={16} className="text-white" />
                </Button>
                <Button
                  size={"icon"}
                  onClick={handleRemoveWorker}
                  className="text-muted-foreground bg-background hover:bg-muted-foreground/10 border h-7 w-10 rounded-lg"
                >
                  <Minus size={16} />
                </Button>
              </span>
            </div>
            <WorkerList />
            <Button
              className={`mt-4 transition-all ${
                appState.running
                  ? "bg-red-50 hover:bg-red-100 border border-red-600"
                  : ""
              }`}
              onClick={handleBreaker}
            >
              {!appState.running && (
                <p className="mx-2 font-bold text-lg">GO</p>
              )}
              {appState.running && (
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
