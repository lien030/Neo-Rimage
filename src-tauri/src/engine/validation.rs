use crate::domain::{
    AppError, CollisionPolicy, ColorProfilePolicy, EmbeddedMetadataPolicy, EncoderConfig,
    EngineRequest, ErrorCode, ErrorContext, FieldError, Operation, ProcessingStage, ResizeMode,
    JOB_CONFIG_VERSION,
};
use std::{collections::BTreeMap, path::Path};

pub(crate) fn validate_request(request: &EngineRequest) -> Result<(), AppError> {
    let mut fields = Vec::new();

    if request.job_id.is_empty() {
        fields.push(field_error(
            "jobId",
            "validation.required",
            "errors.required",
        ));
    }
    if request.item_id.is_empty() {
        fields.push(field_error(
            "itemId",
            "validation.required",
            "errors.required",
        ));
    }
    if request.config_version != JOB_CONFIG_VERSION {
        fields.push(field_error(
            "configVersion",
            "protocol.job_config_version_unsupported",
            "errors.unsupportedConfigVersion",
        ));
    }
    if request.input_path.as_os_str().is_empty() {
        fields.push(field_error(
            "inputPath",
            "validation.required",
            "errors.required",
        ));
    }
    if request.output.output_path.as_os_str().is_empty() {
        fields.push(field_error(
            "output.outputPath",
            "validation.required",
            "errors.required",
        ));
    }

    validate_encoder(request, &mut fields);
    validate_output(request, &mut fields);
    validate_operations(request, &mut fields);

    if fields.is_empty() {
        Ok(())
    } else {
        let mut error = AppError::validation("engine.request.invalid", fields);
        error.context = request_context(request, ProcessingStage::Preflight, &request.input_path);
        Err(error)
    }
}

fn validate_encoder(request: &EngineRequest, fields: &mut Vec<FieldError>) {
    match &request.encoder {
        EncoderConfig::MozJpeg(config) => {
            validate_range(
                fields,
                "encoder.options.quality",
                config.quality,
                1.0,
                100.0,
            );
            if let Some(quality) = config.chroma_quality {
                validate_range(fields, "encoder.options.chromaQuality", quality, 1.0, 100.0);
            }
            if config.smoothing > 100 {
                fields.push(field_error(
                    "encoder.options.smoothing",
                    "validation.out_of_range",
                    "errors.outOfRange",
                ));
            }
            if config
                .chroma_subsample
                .is_some_and(|subsample| !(1..=4).contains(&subsample))
            {
                fields.push(field_error(
                    "encoder.options.chromaSubsample",
                    "validation.out_of_range",
                    "errors.outOfRange",
                ));
            }
        }
        EncoderConfig::Jpeg(config) => validate_range(
            fields,
            "encoder.options.quality",
            config.quality,
            1.0,
            100.0,
        ),
        EncoderConfig::Avif(config) => {
            validate_range(
                fields,
                "encoder.options.quality",
                config.quality,
                1.0,
                100.0,
            );
            if let Some(quality) = config.alpha_quality {
                validate_range(fields, "encoder.options.alphaQuality", quality, 1.0, 100.0);
            }
            if !(1..=10).contains(&config.speed) {
                fields.push(field_error(
                    "encoder.options.speed",
                    "validation.out_of_range",
                    "errors.outOfRange",
                ));
            }
        }
        EncoderConfig::OxiPng(config) => {
            if config.effort > 6 {
                fields.push(field_error(
                    "encoder.options.effort",
                    "validation.out_of_range",
                    "errors.outOfRange",
                ));
            }
        }
        EncoderConfig::WebP(config) => {
            validate_range(
                fields,
                "encoder.options.quality",
                config.quality,
                1.0,
                100.0,
            );
            if config.slight_loss > 100 {
                fields.push(field_error(
                    "encoder.options.slightLoss",
                    "validation.out_of_range",
                    "errors.outOfRange",
                ));
            }
            if !config.lossless && config.slight_loss > 0 {
                fields.push(field_error(
                    "encoder.options.slightLoss",
                    "webp.slight_loss_requires_lossless",
                    "errors.webpSlightLossRequiresLossless",
                ));
            }
        }
        EncoderConfig::JpegXl
        | EncoderConfig::Png
        | EncoderConfig::Farbfeld
        | EncoderConfig::Ppm
        | EncoderConfig::Qoi => {}
    }

    if !matches!(
        request.metadata.color_profile,
        ColorProfilePolicy::PreserveWhenSupported | ColorProfilePolicy::ConvertToSrgb
    ) {
        fields.push(field_error(
            "metadata.colorProfile",
            "metadata.color_profile_policy_unsupported",
            "errors.metadataPolicyUnsupported",
        ));
    }
    if request.metadata.embedded == EmbeddedMetadataPolicy::Strip {
        fields.push(field_error(
            "metadata.embedded",
            "metadata.auto_orient_unavailable",
            "errors.metadataPolicyUnsupported",
        ));
    }
}

fn validate_output(request: &EngineRequest, fields: &mut Vec<FieldError>) {
    let output = &request.output;
    let in_place = same_path(&request.input_path, &output.output_path);

    let extension = output
        .output_path
        .extension()
        .and_then(|extension| extension.to_str());
    if !extension.is_some_and(|extension| {
        super::pipeline::encoder_output_extensions(&request.encoder)
            .iter()
            .any(|expected| extension.eq_ignore_ascii_case(expected))
    }) {
        fields.push(field_error(
            "output.outputPath",
            "output.extension_mismatch",
            "errors.outputExtensionMismatch",
        ));
    }

    if in_place && output.collision != CollisionPolicy::Replace {
        fields.push(field_error(
            "output.collision",
            "output.in_place_requires_replace",
            "errors.inPlaceRequiresReplace",
        ));
    }
    if !in_place && output.source_backup_path.is_some() {
        fields.push(field_error(
            "output.sourceBackupPath",
            "output.source_backup_requires_in_place",
            "errors.sourceBackupRequiresInPlace",
        ));
    }
    if output.collision != CollisionPolicy::Replace && output.existing_output_backup_path.is_some()
    {
        fields.push(field_error(
            "output.existingOutputBackupPath",
            "output.backup_requires_replace",
            "errors.backupRequiresReplace",
        ));
    }
    if in_place
        && output.source_backup_path.is_some()
        && output.existing_output_backup_path.is_some()
    {
        fields.push(field_error(
            "output.existingOutputBackupPath",
            "output.ambiguous_backup",
            "errors.ambiguousBackup",
        ));
    }

    for (field, backup) in [
        ("output.sourceBackupPath", &output.source_backup_path),
        (
            "output.existingOutputBackupPath",
            &output.existing_output_backup_path,
        ),
    ] {
        if let Some(backup) = backup {
            if same_path(backup, &request.input_path) || same_path(backup, &output.output_path) {
                fields.push(field_error(
                    field,
                    "output.backup_path_conflict",
                    "errors.backupPathConflict",
                ));
            }
        }
    }
}

fn validate_operations(request: &EngineRequest, fields: &mut Vec<FieldError>) {
    for (index, operation) in request.operations.iter().enumerate() {
        let path = |field: &str| format!("operations[{index}].{field}");
        match operation {
            Operation::Resize(config) => match config.mode {
                ResizeMode::Exact { width, height } if width == 0 || height == 0 => {
                    fields.push(field_error(
                        &path("config.mode"),
                        "resize.zero_dimension",
                        "errors.resizeZeroDimension",
                    ))
                }
                ResizeMode::FitWidth { width: 0 } => fields.push(field_error(
                    &path("config.mode"),
                    "resize.zero_dimension",
                    "errors.resizeZeroDimension",
                )),
                ResizeMode::FitHeight { height: 0 } => fields.push(field_error(
                    &path("config.mode"),
                    "resize.zero_dimension",
                    "errors.resizeZeroDimension",
                )),
                ResizeMode::Percentage { percent } if !percent.is_finite() || percent <= 0.0 => {
                    fields.push(field_error(
                        &path("config.mode"),
                        "resize.invalid_scale",
                        "errors.resizeInvalidScale",
                    ));
                }
                ResizeMode::Scale { factor } if !factor.is_finite() || factor <= 0.0 => {
                    fields.push(field_error(
                        &path("config.mode"),
                        "resize.invalid_scale",
                        "errors.resizeInvalidScale",
                    ));
                }
                _ => {}
            },
            Operation::Quantize(config) => {
                validate_range(fields, &path("config.quality"), config.quality, 1.0, 100.0)
            }
            Operation::Dither(config) => {
                if index == 0 || !matches!(request.operations[index - 1], Operation::Quantize(_)) {
                    fields.push(field_error(
                        &path("kind"),
                        "dither.requires_quantize",
                        "errors.ditherRequiresQuantize",
                    ));
                }
                if let Some(strength) = config.strength {
                    validate_range(fields, &path("config.strength"), strength, 0.0, 1.0);
                }
            }
            Operation::PremultiplyAlpha => fields.push(field_error(
                &path("kind"),
                "operation.unsupported",
                "errors.operationUnsupported",
            )),
        }
    }
}

fn validate_range(
    fields: &mut Vec<FieldError>,
    path: &str,
    value: f32,
    minimum: f32,
    maximum: f32,
) {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        fields.push(field_error(
            path,
            "validation.out_of_range",
            "errors.outOfRange",
        ));
    }
}

fn field_error(path: &str, code: &str, message_key: &str) -> FieldError {
    FieldError {
        field_path: path.to_owned(),
        code: ErrorCode::from(code),
        message_key: message_key.to_owned(),
        message_args: BTreeMap::new(),
    }
}

pub(crate) fn request_context(
    request: &EngineRequest,
    stage: ProcessingStage,
    path: &Path,
) -> ErrorContext {
    ErrorContext {
        job_id: Some(request.job_id.clone()),
        item_id: Some(request.item_id.clone()),
        stage: Some(stage),
        path: Some(path.display().to_string()),
    }
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AvifAlphaMode, AvifColorSpace, AvifConfig, EmbeddedMetadataPolicy, JobId, JpegConfig,
        MetadataPolicy, MozJpegConfig, OutputPlan, OxiPngConfig, ProcessingReportPolicy,
        WebPConfig,
    };
    use std::path::PathBuf;

    fn request() -> EngineRequest {
        EngineRequest {
            job_id: JobId::new("job"),
            item_id: "item".into(),
            attempt: 1,
            config_version: JOB_CONFIG_VERSION,
            input_path: PathBuf::from("D:/input.png"),
            output: OutputPlan {
                output_path: PathBuf::from("D:/output.jpg"),
                collision: CollisionPolicy::Fail,
                source_backup_path: None,
                existing_output_backup_path: None,
            },
            encoder: EncoderConfig::MozJpeg(MozJpegConfig::default()),
            operations: Vec::new(),
            metadata: MetadataPolicy {
                embedded: EmbeddedMetadataPolicy::PreserveWhenSupported,
                color_profile: ColorProfilePolicy::PreserveWhenSupported,
                report: ProcessingReportPolicy::Disabled,
            },
        }
    }

    #[test]
    fn accepts_mozjpeg_request() {
        assert!(validate_request(&request()).is_ok());
    }

    #[test]
    fn accepts_every_encoder_with_its_canonical_extension() {
        let encoders = [
            (
                EncoderConfig::Jpeg(JpegConfig {
                    quality: 80.0,
                    progressive: true,
                }),
                "jpg",
            ),
            (
                EncoderConfig::Avif(AvifConfig {
                    quality: 50.0,
                    alpha_quality: None,
                    speed: 6,
                    color_space: AvifColorSpace::YCbCr,
                    alpha_mode: AvifAlphaMode::UnassociatedClean,
                }),
                "avif",
            ),
            (
                EncoderConfig::OxiPng(OxiPngConfig {
                    interlace: false,
                    effort: 2,
                }),
                "png",
            ),
            (
                EncoderConfig::WebP(WebPConfig {
                    lossless: false,
                    quality: 75.0,
                    slight_loss: 0,
                    exact: false,
                }),
                "webp",
            ),
            (EncoderConfig::JpegXl, "jxl"),
            (EncoderConfig::Png, "png"),
            (EncoderConfig::Farbfeld, "ff"),
            (EncoderConfig::Ppm, "ppm"),
            (EncoderConfig::Qoi, "qoi"),
        ];

        for (encoder, extension) in encoders {
            let mut request = request();
            request.encoder = encoder;
            request.output.output_path = PathBuf::from(format!("D:/output.{extension}"));
            assert!(validate_request(&request).is_ok(), "extension {extension}");
        }
    }

    #[test]
    fn rejects_extension_mismatch_for_selected_encoder() {
        let mut request = request();
        request.encoder = EncoderConfig::Png;
        let error = validate_request(&request).expect_err("PNG output cannot use JPEG extension");
        assert!(error
            .field_errors
            .iter()
            .any(|field| field.code.0 == "output.extension_mismatch"));
    }

    #[test]
    fn validates_codec_specific_ranges_and_relationships() {
        let mut request = request();
        request.encoder = EncoderConfig::WebP(WebPConfig {
            lossless: false,
            quality: 75.0,
            slight_loss: 10,
            exact: false,
        });
        request.output.output_path = PathBuf::from("D:/output.webp");
        let error = validate_request(&request).expect_err("slight loss requires lossless WebP");
        assert!(error
            .field_errors
            .iter()
            .any(|field| field.code.0 == "webp.slight_loss_requires_lossless"));

        request.encoder = EncoderConfig::OxiPng(OxiPngConfig {
            interlace: false,
            effort: 7,
        });
        request.output.output_path = PathBuf::from("D:/output.png");
        let error = validate_request(&request).expect_err("OxiPNG effort must use a preset");
        assert!(error
            .field_errors
            .iter()
            .any(|field| field.field_path == "encoder.options.effort"));
    }

    #[test]
    fn in_place_output_requires_replace() {
        let mut request = request();
        request.output.output_path = request.input_path.clone();
        let error = validate_request(&request).expect_err("unsafe in-place output");
        assert!(error
            .field_errors
            .iter()
            .any(|field| field.code.0 == "output.in_place_requires_replace"));
    }
}
