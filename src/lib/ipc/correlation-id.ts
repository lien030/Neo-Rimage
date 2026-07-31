import type { CorrelationId } from "./contracts";

/**
 * Creates an identifier used only to correlate an IPC command and response.
 * The fallback is intentionally uniqueness-oriented, not security-sensitive.
 */
export function generateCorrelationId(prefix: string): CorrelationId {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  const randomToken = Math.random().toString(36).slice(2);
  return `${prefix}-${Date.now()}-${randomToken}`;
}
