import { describe, expect, it } from "vitest";

import { inputFileName } from "./worker-display";

describe("inputFileName", () => {
  it("extracts a file name from a Windows path", () => {
    expect(inputFileName("C:\\images\\photo.png")).toBe("photo.png");
  });

  it("extracts a file name from a Unix path", () => {
    expect(inputFileName("/home/user/画像.avif")).toBe("画像.avif");
  });

  it("keeps an already bare file name", () => {
    expect(inputFileName("sample.webp")).toBe("sample.webp");
  });
});
