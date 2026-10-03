use crate::domain::{
    EncoderCapability, EncoderKind, MetadataCapability, OperationCapability, OperationKind,
    OptionCapability, OptionConstraints, OptionValueType,
};
use serde_json::{json, Value};

pub const RIMAGE_SOURCE_REPOSITORY: &str = "https://github.com/vlad-salone/rimage";
pub const RIMAGE_SOURCE_VERSION: &str = "0.14.0";
pub const RIMAGE_SOURCE_REVISION: &str = "978a87075ad60fbbd033ba851036fbe84e9e0f47";

#[derive(Clone, Debug, PartialEq)]
pub struct EngineCapabilitySet {
    pub encoders: Vec<EncoderCapability>,
    pub operations: Vec<OperationCapability>,
    pub metadata: MetadataCapability,
}

pub fn engine_capabilities() -> EngineCapabilitySet {
    EngineCapabilitySet {
        encoders: encoder_capabilities(),
        operations: operation_capabilities(),
        metadata: MetadataCapability {
            // Phase 1 can strip EXIF by omission, but cannot faithfully preserve
            // or auto-orient it yet. Do not advertise partial behavior as support.
            embedded_metadata: false,
            icc_profiles: true,
            auto_orient: false,
            processing_report: false,
        },
    }
}

fn encoder_capabilities() -> Vec<EncoderCapability> {
    vec![
        encoder_capability(
            EncoderKind::MozJpeg,
            &["jpg", "jpeg"],
            mozjpeg_options(),
            &[],
        ),
        encoder_capability(
            EncoderKind::Jpeg,
            &["jpg", "jpeg"],
            jpeg_options(),
            &["maximum_dimension_65535"],
        ),
        encoder_capability(
            EncoderKind::Avif,
            &["avif"],
            avif_options(),
            &["avif_static_sdr_only", "avif_input_normalizes_rgba8"],
        ),
        encoder_capability(
            EncoderKind::OxiPng,
            &["png"],
            oxipng_options(),
            &["may_losslessly_reduce_png_properties"],
        ),
        encoder_capability(EncoderKind::WebP, &["webp"], webp_options(), &[]),
        encoder_capability(
            EncoderKind::JpegXl,
            &["jxl"],
            Vec::new(),
            &["lossless_only"],
        ),
        encoder_capability(EncoderKind::Png, &["png"], Vec::new(), &[]),
        encoder_capability(
            EncoderKind::Farbfeld,
            &["ff", "farbfeld"],
            Vec::new(),
            &["normalizes_pixels_to_rgba16"],
        ),
        encoder_capability(
            EncoderKind::Ppm,
            &["ppm", "pnm"],
            Vec::new(),
            &["alpha_output_uses_pam_header"],
        ),
        encoder_capability(EncoderKind::Qoi, &["qoi"], Vec::new(), &[]),
    ]
}

fn encoder_capability(
    kind: EncoderKind,
    extensions: &[&str],
    options: Vec<OptionCapability>,
    codec_limitations: &[&str],
) -> EncoderCapability {
    let mut limitations = vec!["exif_preservation_not_implemented".to_owned()];
    limitations.push("animated_input_rejected".to_owned());
    if !matches!(kind, EncoderKind::MozJpeg | EncoderKind::OxiPng) {
        limitations.push("icc_profile_not_preserved".to_owned());
    }
    limitations.extend(codec_limitations.iter().map(|value| (*value).to_owned()));
    EncoderCapability {
        kind,
        available: true,
        output_extensions: extensions
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        options,
        limitations,
    }
}

fn mozjpeg_options() -> Vec<OptionCapability> {
    vec![
        number_option("encoder.options.quality", true, json!(75.0), 1.0, 100.0),
        number_option(
            "encoder.options.chromaQuality",
            false,
            Value::Null,
            1.0,
            100.0,
        ),
        boolean_option("encoder.options.progressive", true, true),
        boolean_option("encoder.options.optimizeCoding", true, true),
        integer_option("encoder.options.smoothing", true, json!(0), 0.0, 100.0),
        enum_option(
            "encoder.options.colorSpace",
            true,
            json!("ycbcr"),
            &["ycbcr", "rgb", "grayscale"],
        ),
        boolean_option("encoder.options.trellisMultipass", true, false),
        integer_option(
            "encoder.options.chromaSubsample",
            false,
            Value::Null,
            1.0,
            4.0,
        ),
        enum_option(
            "encoder.options.quantizationTable",
            false,
            Value::Null,
            &[
                "ahumada_watson_peterson",
                "annex_k",
                "flat",
                "klein_silverstein_carney",
                "msssim",
                "n_robidoux",
                "psnr_hvs",
                "peterson_ahumada_watson",
                "watson_taylor_borthwick",
            ],
        ),
    ]
}

fn jpeg_options() -> Vec<OptionCapability> {
    vec![
        number_option("encoder.options.quality", true, json!(80.0), 1.0, 100.0),
        boolean_option("encoder.options.progressive", true, false),
    ]
}

fn avif_options() -> Vec<OptionCapability> {
    vec![
        number_option("encoder.options.quality", true, json!(50.0), 1.0, 100.0),
        number_option(
            "encoder.options.alphaQuality",
            false,
            Value::Null,
            1.0,
            100.0,
        ),
        integer_option("encoder.options.speed", true, json!(6), 1.0, 10.0),
        enum_option(
            "encoder.options.colorSpace",
            true,
            json!("ycbcr"),
            &["ycbcr", "rgb"],
        ),
        enum_option(
            "encoder.options.alphaMode",
            true,
            json!("unassociated_clean"),
            &["unassociated_dirty", "unassociated_clean", "premultiplied"],
        ),
    ]
}

fn oxipng_options() -> Vec<OptionCapability> {
    vec![
        boolean_option("encoder.options.interlace", true, false),
        integer_option("encoder.options.effort", true, json!(2), 0.0, 6.0),
    ]
}

fn webp_options() -> Vec<OptionCapability> {
    let mut slight_loss = integer_option("encoder.options.slightLoss", true, json!(0), 0.0, 100.0);
    slight_loss
        .requires
        .push("encoder.options.lossless=true".to_owned());
    vec![
        boolean_option("encoder.options.lossless", true, false),
        number_option("encoder.options.quality", true, json!(75.0), 1.0, 100.0),
        slight_loss,
        boolean_option("encoder.options.exact", true, false),
    ]
}

fn operation_capabilities() -> Vec<OperationCapability> {
    vec![
        OperationCapability {
            kind: OperationKind::Resize,
            available: true,
            options: vec![
                enum_option(
                    "operations[].config.filter",
                    true,
                    json!("lanczos3"),
                    &[
                        "nearest",
                        "bilinear",
                        "hamming",
                        "catmull_rom",
                        "mitchell",
                        "lanczos3",
                    ],
                ),
                boolean_option("operations[].config.allowUpscale", true, false),
                boolean_option("operations[].config.allowDownscale", true, true),
            ],
            limitations: Vec::new(),
        },
        OperationCapability {
            kind: OperationKind::Quantize,
            available: true,
            options: vec![number_option(
                "operations[].config.quality",
                true,
                json!(75.0),
                1.0,
                100.0,
            )],
            limitations: vec!["normalizes_pixels_to_rgba8".to_owned()],
        },
        OperationCapability {
            kind: OperationKind::Dither,
            available: true,
            options: vec![number_option(
                "operations[].config.strength",
                false,
                Value::Null,
                0.0,
                1.0,
            )],
            limitations: vec!["must_immediately_follow_quantize".to_owned()],
        },
        OperationCapability {
            kind: OperationKind::PremultiplyAlpha,
            available: false,
            options: Vec::new(),
            limitations: vec!["operation_adapter_not_implemented".to_owned()],
        },
    ]
}

fn boolean_option(path: &str, required: bool, default: bool) -> OptionCapability {
    option(
        path,
        OptionValueType::Boolean,
        required,
        Some(json!(default)),
        OptionConstraints::default(),
    )
}

fn number_option(
    path: &str,
    required: bool,
    default: Value,
    minimum: f64,
    maximum: f64,
) -> OptionCapability {
    option(
        path,
        OptionValueType::Number,
        required,
        (default != Value::Null).then_some(default),
        OptionConstraints {
            minimum: Some(minimum),
            maximum: Some(maximum),
            step: None,
            allowed_values: Vec::new(),
        },
    )
}

fn integer_option(
    path: &str,
    required: bool,
    default: Value,
    minimum: f64,
    maximum: f64,
) -> OptionCapability {
    let mut value = number_option(path, required, default, minimum, maximum);
    value.value_type = OptionValueType::Integer;
    value.constraints.step = Some(1.0);
    value
}

fn enum_option(path: &str, required: bool, default: Value, allowed: &[&str]) -> OptionCapability {
    option(
        path,
        OptionValueType::Enum,
        required,
        (default != Value::Null).then_some(default),
        OptionConstraints {
            minimum: None,
            maximum: None,
            step: None,
            allowed_values: allowed.iter().map(|value| json!(value)).collect(),
        },
    )
}

fn option(
    path: &str,
    value_type: OptionValueType,
    required: bool,
    default_value: Option<Value>,
    constraints: OptionConstraints,
) -> OptionCapability {
    OptionCapability {
        path: path.to_owned(),
        value_type,
        required,
        default_value,
        constraints,
        requires: Vec::new(),
        conflicts_with: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MozJpegColorSpace;

    #[test]
    fn all_implemented_encoders_are_advertised() {
        let capabilities = engine_capabilities();
        let available = capabilities
            .encoders
            .iter()
            .filter(|encoder| encoder.available)
            .map(|encoder| encoder.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            available,
            vec![
                EncoderKind::MozJpeg,
                EncoderKind::Jpeg,
                EncoderKind::Avif,
                EncoderKind::OxiPng,
                EncoderKind::WebP,
                EncoderKind::JpegXl,
                EncoderKind::Png,
                EncoderKind::Farbfeld,
                EncoderKind::Ppm,
                EncoderKind::Qoi,
            ]
        );
    }

    #[test]
    fn unsupported_premultiply_is_not_a_ghost_capability() {
        let capabilities = engine_capabilities();
        let operation = capabilities
            .operations
            .iter()
            .find(|operation| operation.kind == OperationKind::PremultiplyAlpha)
            .expect("premultiply capability");
        assert!(!operation.available);
    }

    #[test]
    fn mozjpeg_colorspace_capability_uses_the_domain_tag() {
        let capabilities = engine_capabilities();
        let color_space = capabilities
            .encoders
            .iter()
            .find(|encoder| encoder.kind == EncoderKind::MozJpeg)
            .and_then(|encoder| {
                encoder
                    .options
                    .iter()
                    .find(|option| option.path == "encoder.options.colorSpace")
            })
            .expect("MozJPEG colorspace capability");
        let ycbcr =
            serde_json::to_value(MozJpegColorSpace::YCbCr).expect("serialize MozJPEG colorspace");

        assert_eq!(color_space.default_value.as_ref(), Some(&ycbcr));
        assert!(color_space.constraints.allowed_values.contains(&ycbcr));
    }
}
