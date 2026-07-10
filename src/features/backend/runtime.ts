import { useEffect } from "react";
import { proxy, useSnapshot } from "valtio";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";

import {
  backendClient,
  IPC_SCHEMA_VERSION,
  type BackendCapabilities,
  type BackendSnapshot,
  type StateEventEnvelope,
} from "@/lib/ipc";
import { classifyRevision } from "@/lib/ipc/revision";

export type BackendSyncStatus =
  | "idle"
  | "syncing"
  | "ready"
  | "needs_resync"
  | "unavailable";

interface BackendRuntimeState {
  capabilities: BackendCapabilities | null;
  snapshot: BackendSnapshot | null;
  syncStatus: BackendSyncStatus;
  lastAppliedRevision: number;
  lastError: string | null;
}

interface BackendCommandState {
  schedulerPending: boolean;
  workerCountPending: boolean;
}

export const backendRuntimeState = proxy<BackendRuntimeState>({
  capabilities: null,
  snapshot: null,
  syncStatus: "idle",
  lastAppliedRevision: 0,
  lastError: null,
});

export const backendCommandState = proxy<BackendCommandState>({
  schedulerPending: false,
  workerCountPending: false,
});

export function useBackendRuntimeState() {
  return useSnapshot(backendRuntimeState);
}

export function useBackendCommandState() {
  return useSnapshot(backendCommandState);
}

let snapshotRequest: Promise<BackendSnapshot> | null = null;
let capabilitiesRequest: Promise<BackendCapabilities> | null = null;
let requestedRevision = 0;

function assertSchemaVersion(schemaVersion: number, source: string) {
  if (schemaVersion !== IPC_SCHEMA_VERSION) {
    throw new Error(
      `${source} uses unsupported schema version ${schemaVersion}.`,
    );
  }
}

function applyCapabilities(capabilities: BackendCapabilities) {
  assertSchemaVersion(capabilities.schemaVersion, "Backend capabilities");
  backendRuntimeState.capabilities = capabilities;
}

function applySnapshot(snapshot: BackendSnapshot) {
  assertSchemaVersion(snapshot.schemaVersion, "Backend snapshot");
  if (snapshot.revision < backendRuntimeState.lastAppliedRevision) {
    return;
  }

  backendRuntimeState.snapshot = snapshot;
  backendRuntimeState.lastAppliedRevision = snapshot.revision;
  backendRuntimeState.syncStatus = "ready";
  backendRuntimeState.lastError = null;
}

export async function refreshBackendCapabilities(): Promise<BackendCapabilities> {
  if (!capabilitiesRequest) {
    capabilitiesRequest = backendClient
      .getCapabilities()
      .then((capabilities) => {
        applyCapabilities(capabilities);
        return capabilities;
      })
      .finally(() => {
        capabilitiesRequest = null;
      });
  }

  return capabilitiesRequest;
}

export async function refreshBackendSnapshot(
  minimumRevision = 0,
): Promise<BackendSnapshot> {
  requestedRevision = Math.max(requestedRevision, minimumRevision);

  if (!snapshotRequest) {
    snapshotRequest = (async () => {
      while (true) {
        const snapshot = await backendClient.getSnapshot();
        applySnapshot(snapshot);
        if (snapshot.revision >= requestedRevision) {
          return snapshot;
        }
      }
    })().finally(() => {
      snapshotRequest = null;
    });
  }

  return snapshotRequest;
}

async function handleStateEvent(event: StateEventEnvelope) {
  assertSchemaVersion(event.schemaVersion, "Backend state event");

  const relation = classifyRevision(
    backendRuntimeState.lastAppliedRevision,
    event.revision,
  );
  if (relation === "stale") {
    return;
  }

  // Events are bounded dirty hints. A full snapshot remains authoritative.
  backendRuntimeState.syncStatus =
    relation === "gap" ? "needs_resync" : "syncing";
  await refreshBackendSnapshot(event.revision);
}

export function formatBackendError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  if (error && typeof error === "object") {
    const value = error as {
      error?: { fallbackMessage?: unknown };
      fallbackMessage?: unknown;
      message?: unknown;
    };
    const message =
      value.error?.fallbackMessage ?? value.fallbackMessage ?? value.message;
    if (typeof message === "string") {
      return message;
    }
  }

  return "The backend request failed.";
}

export function useBackendRuntimeSync() {
  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;

    backendRuntimeState.syncStatus = "syncing";
    backendRuntimeState.lastError = null;

    const start = async () => {
      unlisten = await backendClient.listenToStateEvents((event) => {
        if (disposed) {
          return;
        }

        void handleStateEvent(event).catch((error: unknown) => {
          if (disposed) {
            return;
          }
          const message = formatBackendError(error);
          backendRuntimeState.syncStatus = "needs_resync";
          backendRuntimeState.lastError = message;
          toast.error("Backend synchronization failed", {
            description: message,
          });
        });
      });

      if (disposed) {
        unlisten();
        unlisten = null;
        return;
      }

      await Promise.all([
        refreshBackendCapabilities(),
        refreshBackendSnapshot(),
      ]);
    };

    void start().catch((error: unknown) => {
      if (disposed) {
        return;
      }
      const message = formatBackendError(error);
      backendRuntimeState.syncStatus = "unavailable";
      backendRuntimeState.lastError = message;
      toast.error("Backend unavailable", { description: message });
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
