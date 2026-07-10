import { proxy } from "valtio";
import { useProxy } from "valtio/utils";
import { ProcessWorker, AppState, TaskStore } from "./type";

export const appState: AppState = proxy({
  recursiveFolders: true,
  isShowCreateTask: false,
  isShowDragDrop: false,
  running: false,
});
export const useAppState = () => useProxy(appState);

export const defaultWorkerData: ProcessWorker[] = [];
const workerConfig = proxy(defaultWorkerData);
export const useWorkerList = () => useProxy(workerConfig);

const taskConfig:TaskStore = proxy({
  taskList: [],
  taskCache: [],
});
export const useTaskStore = () => useProxy(taskConfig);
