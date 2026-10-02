import { backendClient, generateCorrelationId, IPC_SCHEMA_VERSION } from "@/lib/ipc";

export interface SelectedInputFile {
  readonly path: string;
  readonly fileName: string;
}

export async function collectImageInputs(
  paths: readonly string[],
  scanRecursively = false,
): Promise<SelectedInputFile[]> {
  const response = await backendClient.scanInputs({
    schemaVersion: IPC_SCHEMA_VERSION,
    correlationId: generateCorrelationId("scan"),
    paths: [...paths],
    scanRecursively,
  });
  if (response.schemaVersion !== IPC_SCHEMA_VERSION) {
    throw new Error("Input scan uses unsupported schema version " + response.schemaVersion + ".");
  }
  return response.inputs;
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
