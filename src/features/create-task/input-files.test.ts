import { describe, expect, it } from "vitest";

import { mergeSelectedInputs } from "./input-files";

describe("mergeSelectedInputs", () => {
  it("preserves order while removing paths already selected", () => {
    const selected = [{ path: "a.png", fileName: "a.png" }];
    const incoming = [
      { path: "a.png", fileName: "duplicate.png" },
      { path: "b.png", fileName: "b.png" },
    ];

    expect(mergeSelectedInputs(selected, incoming)).toEqual([
      { path: "a.png", fileName: "a.png" },
      { path: "b.png", fileName: "b.png" },
    ]);
  });

  it("deduplicates repeated paths within the incoming list", () => {
    const incoming = [
      { path: "a.png", fileName: "a.png" },
      { path: "a.png", fileName: "duplicate.png" },
    ];

    expect(mergeSelectedInputs([], incoming)).toEqual([
      { path: "a.png", fileName: "a.png" },
    ]);
  });
});
