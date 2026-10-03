import { describe, expect, it, vi } from "vitest";

import { CreateTaskValidationError } from "@/features/create-task";

import { createTaskErrorMessage } from "./create-task-error";

const translate = (key: string) => `translated:${key}`;

describe("createTaskErrorMessage", () => {
  it("maps validation codes to their existing translation keys", () => {
    expect(
      createTaskErrorMessage(
        new CreateTaskValidationError("backup_requires_replace"),
        translate,
      ),
    ).toBe("translated:createTaskErrorBackupPolicy");
  });

  it("prefers a nested backend fallback message", () => {
    expect(
      createTaskErrorMessage(
        { error: { fallbackMessage: "Backend rejected the job" } },
        translate,
      ),
    ).toBe("Backend rejected the job");
  });

  it("falls back to the translated unknown-error message", () => {
    expect(createTaskErrorMessage(null, translate)).toBe(
      "translated:createTaskErrorUnknown",
    );
  });

  it("uses backend message keys and interpolation arguments before the English fallback", () => {
    const translateBackend = vi.fn(() => "Localized backend error");
    const message = createTaskErrorMessage({
      error: {
        messageKey: "errors.decodeFailed",
        messageArgs: { path: "photo.png" },
        fallbackMessage: "Original backend diagnostic",
      },
    }, translateBackend);
    expect(message).toBe("Localized backend error");
    expect(translateBackend).toHaveBeenCalledWith("errors.decodeFailed", {
      path: "photo.png", defaultValue: "Original backend diagnostic",
    });
  });
});
