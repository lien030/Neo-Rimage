import type { BackendSnapshot, Revision } from "./contracts";
import { classifyRevision } from "./revision";

export type BackendSyncStatus =
  | "idle"
  | "syncing"
  | "ready"
  | "needs_resync"
  | "unavailable";

/**
 * Read-only backend observation cache. This is deliberately separate from
 * editable form values and local UI state.
 */
export interface BackendSnapshotState {
  readonly status: BackendSyncStatus;
  readonly snapshot: BackendSnapshot | null;
  readonly lastAppliedRevision: Revision;
}

export const EMPTY_BACKEND_SNAPSHOT_STATE: BackendSnapshotState = {
  status: "idle",
  snapshot: null,
  lastAppliedRevision: 0,
};

export function acceptFullSnapshot(
  state: BackendSnapshotState,
  snapshot: BackendSnapshot,
): BackendSnapshotState {
  if (snapshot.revision < state.lastAppliedRevision) {
    return state;
  }

  return {
    status: "ready",
    snapshot,
    lastAppliedRevision: snapshot.revision,
  };
}

export function eventSyncStatus(
  currentRevision: Revision,
  incomingRevision: Revision,
): "ignore" | "apply" | "resync" {
  const relation = classifyRevision(currentRevision, incomingRevision);
  if (relation === "stale") {
    return "ignore";
  }

  return relation === "next" ? "apply" : "resync";
}
