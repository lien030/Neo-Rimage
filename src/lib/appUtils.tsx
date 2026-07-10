import { basename } from "@tauri-apps/api/path";
import { stat } from "@tauri-apps/plugin-fs";
import { TaskCacheType } from "./type";
import mime from "mime";
import { invoke } from "@tauri-apps/api/core";

export async function fileFilter(paths: string[], recursive?: boolean) {
  const filePaths: string[] = [];

  for (const path of paths) {
    const fileInfo = await stat(path);
    if (fileInfo.isFile) {
      filePaths.push(path);
    } else if (fileInfo.isDirectory && recursive) {
      const scanPaths:string[] = await invoke("scan_dir", { path: path });
      filePaths.push(...scanPaths);
    }
  }
  return filePaths;
}

export async function createTaskList(paths: string[]) {
  // filter image files
  const tasks = await Promise.all(paths.map(async (path) => {
    const fileBaseName = await basename(path);
    const mimeType = mime.getType(fileBaseName);
    if (mimeType && mimeType.includes("image")) {
      return {
        path: path,
        fileName: fileBaseName,
      };
    }
    return null;
  }));

  return tasks.filter(task => task !== null);
}

export function mergeTask(appCache: TaskCacheType[], newTaskList: TaskCacheType[]) {
  const newCache = newTaskList.filter((task) => {
    return !appCache.some((cache) => cache.path === task.path);
  });

  return [...appCache, ...newCache];
}
