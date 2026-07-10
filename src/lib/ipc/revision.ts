import type { Revision } from "./contracts";

export type RevisionRelation = "stale" | "next" | "gap";

export function classifyRevision(
  current: Revision,
  incoming: Revision,
): RevisionRelation {
  if (incoming <= current) {
    return "stale";
  }

  return incoming === current + 1 ? "next" : "gap";
}

export function shouldApplyRevision(
  current: Revision,
  incoming: Revision,
): boolean {
  return classifyRevision(current, incoming) === "next";
}
