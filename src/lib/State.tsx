import { proxy } from "valtio";
import { useProxy } from "valtio/utils";
import { TaskCacheType } from "./type";

export {
  useBackendCommandState,
  useBackendRuntimeState,
} from "@/features/backend";

export const appState = proxy({
  recursiveFolders: true,
  isShowCreateTask: false,
  isShowDragDrop: false,
});
export const useAppState = () => useProxy(appState);

const taskConfig = proxy<{ taskCache: TaskCacheType[] }>({
  taskCache: [],
});
export const useTaskStore = () => useProxy(taskConfig);
