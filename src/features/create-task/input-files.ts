import { invoke } from "@tauri-apps/api/core";
import { basename } from "@tauri-apps/api/path";
import { stat } from "@tauri-apps/plugin-fs";
import mime from "mime";

export interface SelectedInputFile {
  readonly path: string;
  readonly fileName: string;
}

export async function expandDroppedPaths(
  paths: readonly string[],
  scanRecursively = false,
): Promise<string[]> {
  const filePaths: string[] = [];

  for (const path of paths) {
    const fileInfo = await stat(path);
    if (fileInfo.isFile) {
      filePaths.push(path);
    } else if (fileInfo.isDirectory && scanRecursively) {
      // Directory traversal stays behind the compatibility backend command so
      // platform filesystem behavior is not reimplemented in the UI.
      const scannedPaths = await invoke<string[]>("scan_dir", { path });
      filePaths.push(...scannedPaths);
    }
  }

  return filePaths;
}

export async function collectImageInputs(
  paths: readonly string[],
): Promise<SelectedInputFile[]> {
  const inputs = await Promise.all(
    paths.map(async (path): Promise<SelectedInputFile | null> => {
      const fileName = await basename(path);
      const mimeType = mime.getType(fileName);

      return mimeType?.startsWith("image/") ? { path, fileName } : null;
    }),
  );

  return inputs.filter((input): input is SelectedInputFile => input !== null);
}

export function mergeSelectedInputs(
  selectedInputs: readonly SelectedInputFile[],
  incomingInputs: readonly SelectedInputFile[],
): SelectedInputFile[] {
  const knownPaths = new Set(selectedInputs.map((input) => input.path));
  const uniqueIncomingInputs = incomingInputs.filter((input) => {
    if (knownPaths.has(input.path)) {
      return false;
    }

    knownPaths.add(input.path);
    return true;
  });

  return [...selectedInputs, ...uniqueIncomingInputs];
}
