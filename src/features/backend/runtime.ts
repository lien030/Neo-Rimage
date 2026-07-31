import { useEffect } from "react";
import { proxy, useSnapshot } from "valtio";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";

import {
  backendClient,
  classifyRevision,
  IPC_SCHEMA_VERSION,
  type BackendCapabilities,
  type BackendSnapshot,
  type BackendSyncStatus,
  type Revision,
  type StateEventEnvelope,
} from "@/lib/ipc";

export type { BackendSyncStatus } from "@/lib/ipc";

interface BackendRuntimeState {
  capabilities: BackendCapabilities | null;
  snapshot: BackendSnapshot | null;
  syncStatus: BackendSyncStatus;
  lastAppliedRevision: Revision;
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
let requestedRevision: Revision = 0;

function assertSchemaVersion(schemaVersion: number, source: string): void {
  if (schemaVersion !== IPC_SCHEMA_VERSION) {
    throw new Error(
      `${source} uses unsupported schema version ${schemaVersion}.`,
    );
  }
}

function applyCapabilities(capabilities: BackendCapabilities): void {
  assertSchemaVersion(capabilities.schemaVersion, "Backend capabilities");
  backendRuntimeState.capabilities = capabilities;
}

function applySnapshot(snapshot: BackendSnapshot): void {
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
  requestedRevision = Math.max(
    requestedRevision,
    backendRuntimeState.lastAppliedRevision,
    minimumRevision,
  );

  if (!snapshotRequest) {
    // Coalesce concurrent refreshes while still honoring the highest revision
    // requested by an event or command acknowledgement.
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

async function handleStateEvent(event: StateEventEnvelope): Promise<void> {
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

function reportBackendError(
  syncStatus: BackendSyncStatus,
  title: string,
  error: unknown,
): void {
  const message = formatBackendError(error);
  backendRuntimeState.syncStatus = syncStatus;
  backendRuntimeState.lastError = message;
  toast.error(title, { description: message });
}

export function useBackendRuntimeSync(): void {
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
          reportBackendError(
            "needs_resync",
            "Backend synchronization failed",
            error,
          );
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
      reportBackendError("unavailable", "Backend unavailable", error);
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
