import { useEffect } from "react";
import { proxy, ref, useSnapshot } from "valtio";
import type { TOptions } from "i18next";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";

import i18n from "@/i18n/config";
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
  lastErrorDetails: unknown;
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
  lastErrorDetails: null,
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
    throw Object.assign(
      new Error(`${source} uses unsupported schema version ${schemaVersion}.`),
      { messageKey: "errors.schemaVersionUnsupported" },
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
  backendRuntimeState.lastErrorDetails = null;
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

export function formatBackendError(
  error: unknown,
  translate: (key: string, options?: TOptions) => string = i18n.t,
  fallbackKey = "backendRequestFailed",
): string {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object") {
    const value = error as {
      error?: {
        messageKey?: unknown;
        messageArgs?: unknown;
        fallbackMessage?: unknown;
        message?: unknown;
      };
      messageKey?: unknown;
      messageArgs?: unknown;
      fallbackMessage?: unknown;
      message?: unknown;
    };
    const appError =
      value.error && typeof value.error === "object" ? value.error : value;
    const message =
      appError.fallbackMessage ?? appError.message ?? value.fallbackMessage ?? value.message;
    if (typeof appError.messageKey === "string") {
      const args = appError.messageArgs;
      return translate(appError.messageKey, {
        ...(args && typeof args === "object" && !Array.isArray(args) ? args : {}),
        defaultValue: typeof message === "string" ? message : translate(fallbackKey),
      });
    }
    if (typeof message === "string") {
      return message;
    }
  }

  return translate(fallbackKey);
}

function reportBackendError(
  syncStatus: BackendSyncStatus,
  title: string,
  error: unknown,
): void {
  const message = formatBackendError(error);
  backendRuntimeState.syncStatus = syncStatus;
  backendRuntimeState.lastError = message;
  backendRuntimeState.lastErrorDetails =
    error && typeof error === "object" ? ref(error) : error;
  toast.error(i18n.t(title), { description: message });
}

export function useBackendRuntimeSync(): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;

    backendRuntimeState.syncStatus = "syncing";
    backendRuntimeState.lastError = null;
    backendRuntimeState.lastErrorDetails = null;

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
            "backendSynchronizationFailed",
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
      reportBackendError("unavailable", "backendUnavailable", error);
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
