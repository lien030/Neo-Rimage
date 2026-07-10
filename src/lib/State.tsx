import { proxy, useSnapshot } from "valtio";
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
export const useAppState = () => useSnapshot(appState);

export const taskState = proxy<{ taskCache: TaskCacheType[] }>({
  taskCache: [],
});
export const useTaskStore = () => useSnapshot(taskState);
