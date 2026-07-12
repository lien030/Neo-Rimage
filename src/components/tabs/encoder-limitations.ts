const LIMITATION_I18N_KEYS: Record<string, string> = {
  exif_preservation_not_implemented: "limitationExifNotPreserved",
  animated_input_rejected: "limitationAnimatedInputRejected",
  icc_profile_not_preserved: "limitationIccNotPreserved",
  maximum_dimension_65535: "limitationMaximumDimension65535",
  avif_input_decode_unavailable: "limitationAvifInputDecodeUnavailable",
  may_losslessly_reduce_png_properties:
    "limitationMayLosslesslyReducePngProperties",
  lossless_only: "limitationLosslessOnly",
  normalizes_pixels_to_rgba16: "limitationNormalizesPixelsToRgba16",
  alpha_output_uses_pam_header: "limitationAlphaOutputUsesPamHeader",
};

export function describeEncoderLimitation(
  code: string,
  t: (key: string) => string,
): string {
  const key = LIMITATION_I18N_KEYS[code];
  return key ? t(key) : code;
}
