import { describe, expect, it } from "vitest";

import type { EncoderKind } from "@/lib/ipc/contracts";

import type { CreateTaskFormValues } from "./domain";
import {
  CreateTaskValidationError,
  buildCreateJobRequest,
} from "./request";
import { createDefaultCreateTaskForm } from "./store";

const ALL_ENCODERS: readonly EncoderKind[] = [
  "mozjpeg",
  "jpeg",
  "avif",
  "oxipng",
  "webp",
  "jpeg_xl",
  "png",
  "farbfeld",
  "ppm",
  "qoi",
];

const SAMPLE_INPUTS = [{ path: "C:/images/sample.png", fileName: "sample.png" }];

function draftWith(
  overrides: Partial<CreateTaskFormValues> = {},
): CreateTaskFormValues {
  return {
    ...createDefaultCreateTaskForm(),
    ...overrides,
  };
}

function expectValidationCode(
  fn: () => unknown,
  code: CreateTaskValidationError["code"],
) {
  try {
    fn();
    expect.unreachable("expected CreateTaskValidationError");
  } catch (error) {
    expect(error).toBeInstanceOf(CreateTaskValidationError);
    expect((error as CreateTaskValidationError).code).toBe(code);
  }
}

describe("buildCreateJobRequest", () => {
  it("requires at least one input", () => {
    expectValidationCode(
      () => buildCreateJobRequest(draftWith(), [], ALL_ENCODERS),
      "inputs_required",
    );
  });

  it("rejects empty input paths", () => {
    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith(),
          [{ path: "  ", fileName: "blank" }],
          ALL_ENCODERS,
        ),
      "input_path_required",
    );
  });

  it("rejects encoders missing from the available list", () => {
    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({ activeEncoder: "webp" }),
          SAMPLE_INPUTS,
          ["mozjpeg"],
        ),
      "encoder_unsupported",
    );
  });

  it("builds a mozjpeg request with defaults", () => {
    const request = buildCreateJobRequest(
      draftWith({ activeEncoder: "mozjpeg" }),
      SAMPLE_INPUTS,
      ALL_ENCODERS,
    );

    expect(request.encoder).toEqual({
      kind: "mozjpeg",
      options: {
        quality: 75,
        chromaQuality: null,
        progressive: true,
        optimizeCoding: true,
        smoothing: 0,
        colorSpace: "ycbcr",
        trellisMultipass: false,
        chromaSubsample: null,
        quantizationTable: null,
      },
    });
    expect(request.inputs).toEqual([
      { path: "C:/images/sample.png", kind: "file", scanRecursively: false },
    ]);
    expect(request.operations).toEqual([]);
  });

  it("builds jpeg / avif / oxipng option payloads", () => {
    expect(
      buildCreateJobRequest(
        draftWith({ activeEncoder: "jpeg" }),
        SAMPLE_INPUTS,
        ALL_ENCODERS,
      ).encoder,
    ).toEqual({
      kind: "jpeg",
      options: { quality: 80, progressive: false },
    });

    expect(
      buildCreateJobRequest(
        draftWith({
          activeEncoder: "avif",
          avif: {
            quality: "60",
            alphaQuality: "70",
            speed: "4",
            colorSpace: "rgb",
            alphaMode: "premultiplied",
          },
        }),
        SAMPLE_INPUTS,
        ALL_ENCODERS,
      ).encoder,
    ).toEqual({
      kind: "avif",
      options: {
        quality: 60,
        alphaQuality: 70,
        speed: 4,
        colorSpace: "rgb",
        alphaMode: "premultiplied",
      },
    });

    expect(
      buildCreateJobRequest(
        draftWith({
          activeEncoder: "oxipng",
          oxipng: { interlace: true, effort: "5" },
        }),
        SAMPLE_INPUTS,
        ALL_ENCODERS,
      ).encoder,
    ).toEqual({
      kind: "oxipng",
      options: { interlace: true, effort: 5 },
    });
  });

  it("builds webp lossy and lossless configs", () => {
    expect(
      buildCreateJobRequest(
        draftWith({
          activeEncoder: "webp",
          webp: {
            lossless: false,
            quality: "90",
            slightLoss: "0",
            exact: true,
          },
        }),
        SAMPLE_INPUTS,
        ALL_ENCODERS,
      ).encoder,
    ).toEqual({
      kind: "webp",
      options: {
        lossless: false,
        quality: 90,
        slightLoss: 0,
        exact: true,
      },
    });

    expect(
      buildCreateJobRequest(
        draftWith({
          activeEncoder: "webp",
          webp: {
            lossless: true,
            quality: "75",
            slightLoss: "40",
            exact: false,
          },
        }),
        SAMPLE_INPUTS,
        ALL_ENCODERS,
      ).encoder,
    ).toEqual({
      kind: "webp",
      options: {
        lossless: true,
        quality: 75,
        slightLoss: 40,
        exact: false,
      },
    });
  });

  it("rejects webp near-lossless without lossless", () => {
    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            activeEncoder: "webp",
            webp: {
              lossless: false,
              quality: "75",
              slightLoss: "10",
              exact: false,
            },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "webp_slight_loss_requires_lossless",
    );
  });

  it("builds optionless unit encoders", () => {
    for (const kind of [
      "jpeg_xl",
      "png",
      "farbfeld",
      "ppm",
      "qoi",
    ] as const) {
      expect(
        buildCreateJobRequest(
          draftWith({ activeEncoder: kind }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ).encoder,
      ).toEqual({ kind });
    }
  });

  it("validates encoder numeric ranges", () => {
    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            activeEncoder: "jpeg",
            jpeg: { quality: "0", progressive: false },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "jpeg_quality_invalid",
    );

    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            activeEncoder: "avif",
            avif: {
              quality: "50",
              alphaQuality: "",
              speed: "11",
              colorSpace: "ycbcr",
              alphaMode: "unassociated_clean",
            },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "avif_speed_invalid",
    );

    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            activeEncoder: "oxipng",
            oxipng: { interlace: false, effort: "7" },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "oxipng_effort_invalid",
    );

    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            activeEncoder: "webp",
            webp: {
              lossless: false,
              quality: "101",
              slightLoss: "0",
              exact: false,
            },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "webp_quality_invalid",
    );
  });

  it("includes resize operations when enabled", () => {
    const request = buildCreateJobRequest(
      draftWith({
        resize: {
          enabled: true,
          mode: "exact",
          width: "640",
          height: "480",
          percent: "100",
          factor: "1",
          filter: "lanczos3",
          allowUpscale: false,
          allowDownscale: true,
        },
      }),
      SAMPLE_INPUTS,
      ALL_ENCODERS,
    );

    expect(request.operations).toEqual([
      {
        kind: "resize",
        config: {
          mode: {
            kind: "exact",
            value: { width: 640, height: 480 },
          },
          filter: "lanczos3",
          allowUpscale: false,
          allowDownscale: true,
        },
      },
    ]);
  });

  it("serializes output and metadata policies", () => {
    const request = buildCreateJobRequest(
      draftWith({
        output: {
          locationMode: "directory",
          outputDirectory: "  C:/images/output  ",
          preserveStructure: true,
          suffix: "-converted",
          collision: "replace",
          sourceBackup: true,
          existingOutputBackup: true,
        },
        metadata: {
          embedded: "strip",
          colorProfile: "convert_to_srgb",
          reportEnabled: true,
          reportPath: "  C:/images/report.json  ",
        },
      }),
      SAMPLE_INPUTS,
      ALL_ENCODERS,
    );

    expect(request.output).toEqual({
      location: { kind: "directory", path: "C:/images/output" },
      preserveStructure: true,
      suffix: "-converted",
      collision: "replace",
      sourceBackup: "enabled",
      existingOutputBackup: "enabled",
    });
    expect(request.metadata).toEqual({
      embedded: "strip",
      colorProfile: "convert_to_srgb",
      report: { kind: "json", path: "C:/images/report.json" },
    });
  });

  it("validates output policy invariants", () => {
    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            output: {
              ...createDefaultCreateTaskForm().output,
              locationMode: "directory",
            },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "output_directory_required",
    );

    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            output: {
              ...createDefaultCreateTaskForm().output,
              suffix: "../invalid",
            },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "suffix_invalid",
    );

    expectValidationCode(
      () =>
        buildCreateJobRequest(
          draftWith({
            output: {
              ...createDefaultCreateTaskForm().output,
              sourceBackup: true,
            },
          }),
          SAMPLE_INPUTS,
          ALL_ENCODERS,
        ),
      "backup_requires_replace",
    );
  });
});
