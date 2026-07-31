import { describe, expect, it } from "vitest";

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
});
