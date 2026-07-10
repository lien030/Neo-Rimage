use super::{
    output::{CommitPolicy, OutputTransaction},
    pipeline::{self, ResizeEffect},
    runtime::EngineContext,
    validation::{request_context, same_path, validate_request},
};
use crate::domain::{
    AppError, CancellationProbe, CollisionPolicy, EmbeddedMetadataPolicy, EncoderConfig,
    EngineOutcome, EngineProgressEvent, EngineRequest, EngineResult, EngineWarning, ErrorCategory,
    ImageFormat, MetadataOutcome, Operation, ProcessingStage, ProgressMeasure, ProgressReporter,
};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

pub trait Engine: Send + Sync {
    fn execute(&self, request: &EngineRequest, context: &EngineContext<'_>) -> EngineOutcome;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalEngine;

impl LocalEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn process(
        &self,
        request: &EngineRequest,
        progress: &dyn ProgressReporter,
        cancellation: &dyn CancellationProbe,
    ) -> EngineOutcome {
        self.execute(request, &EngineContext::new(progress, cancellation))
    }

    fn execute_inner(&self, request: &EngineRequest, context: &EngineContext<'_>) -> EngineOutcome {
        let total_start = Instant::now();
        let mut durations = BTreeMap::new();
        let mut warnings = Vec::new();

        run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Preflight,
            || {
                check_cancel(request, ProcessingStage::Preflight, context.cancellation)?;
                validate_request(request)
            },
        )?;

        let input_bytes = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Inspect,
            || {
                check_cancel(request, ProcessingStage::Inspect, context.cancellation)?;
                let metadata = fs::metadata(&request.input_path).map_err(|_| {
                    engine_error(
                        request,
                        ProcessingStage::Inspect,
                        ErrorCategory::Input,
                        "input.metadata_unavailable",
                        "errors.inputUnavailable",
                        "The input file could not be inspected.",
                        false,
                        &request.input_path,
                    )
                })?;
                if !metadata.is_file() {
                    return Err(engine_error(
                        request,
                        ProcessingStage::Inspect,
                        ErrorCategory::Input,
                        "input.not_regular_file",
                        "errors.inputNotRegularFile",
                        "The input path is not a regular file.",
                        false,
                        &request.input_path,
                    ));
                }
                Ok(metadata.len())
            },
        )?;

        let mut image = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Decode,
            || {
                check_cancel(request, ProcessingStage::Decode, context.cancellation)?;
                let image = pipeline::decode(&request.input_path).map_err(|_| {
                    engine_error(
                        request,
                        ProcessingStage::Decode,
                        ErrorCategory::Input,
                        "input.decode_failed",
                        "errors.decodeFailed",
                        "The input is damaged or uses an unsupported image format.",
                        false,
                        &request.input_path,
                    )
                })?;
                if image.frames_len() > 1 {
                    return Err(engine_error(
                        request,
                        ProcessingStage::Decode,
                        ErrorCategory::Input,
                        "input.animation_unsupported",
                        "errors.animationUnsupported",
                        "MozJPEG does not support animated input without discarding frames.",
                        false,
                        &request.input_path,
                    ));
                }
                check_cancel(request, ProcessingStage::Decode, context.cancellation)?;
                Ok(image)
            },
        )?;

        let input_properties =
            pipeline::image_properties(&image, &request.input_path).map_err(|_| {
                engine_error(
                    request,
                    ProcessingStage::Inspect,
                    ErrorCategory::Input,
                    "input.properties_invalid",
                    "errors.inputPropertiesInvalid",
                    "The decoded image has unsupported dimensions or properties.",
                    false,
                    &request.input_path,
                )
            })?;
        warn_extension_mismatch(
            request,
            &input_properties.format,
            context.progress,
            &mut warnings,
        );

        let had_exif = image
            .metadata()
            .exif()
            .is_some_and(|fields| !fields.is_empty());
        let normalize = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Normalize,
            || {
                check_cancel(request, ProcessingStage::Normalize, context.cancellation)?;
                let outcome =
                    pipeline::normalize_color_profile(&mut image, request.metadata.color_profile)
                        .map_err(|_| {
                        engine_error(
                            request,
                            ProcessingStage::Normalize,
                            ErrorCategory::Processing,
                            "processing.color_profile_failed",
                            "errors.colorProfileFailed",
                            "The image color profile could not be normalized.",
                            false,
                            &request.input_path,
                        )
                    })?;
                check_cancel(request, ProcessingStage::Normalize, context.cancellation)?;
                Ok(outcome)
            },
        )?;

        if had_exif && request.metadata.embedded == EmbeddedMetadataPolicy::PreserveWhenSupported {
            push_warning(
                context.progress,
                &mut warnings,
                warning(
                    "metadata.exif_not_preserved",
                    Some(ProcessingStage::Normalize),
                    "warnings.exifNotPreserved",
                    "EXIF preservation is not implemented for the Phase-1 MozJPEG adapter.",
                ),
            );
        }

        run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Operations,
            || apply_operations(request, context, &mut image, &mut warnings),
        )?;

        let EncoderConfig::MozJpeg(encoder_config) = &request.encoder else {
            return Err(engine_error(
                request,
                ProcessingStage::Encode,
                ErrorCategory::Validation,
                "encoder.unsupported",
                "errors.encoderUnsupported",
                "The selected encoder is not available in this build.",
                false,
                &request.output.output_path,
            ));
        };

        let output_properties = pipeline::mozjpeg_output_properties(&image, encoder_config)
            .map_err(|_| {
                engine_error(
                    request,
                    ProcessingStage::Encode,
                    ErrorCategory::Encoding,
                    "encoding.properties_invalid",
                    "errors.outputPropertiesInvalid",
                    "The processed image has unsupported output dimensions.",
                    false,
                    &request.output.output_path,
                )
            })?;

        let mut transaction = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Encode,
            || {
                check_cancel(request, ProcessingStage::Encode, context.cancellation)?;
                let mut transaction =
                    OutputTransaction::begin(&request.output.output_path, request.item_id.as_str())
                        .map_err(|_| {
                            engine_error(
                                request,
                                ProcessingStage::Encode,
                                ErrorCategory::Output,
                                "output.temp_create_failed",
                                "errors.outputTempCreateFailed",
                                "A temporary output file could not be created.",
                                true,
                                &request.output.output_path,
                            )
                        })?;

                pipeline::encode_mozjpeg(
                    &image,
                    encoder_config,
                    transaction.writer().map_err(|_| {
                        engine_error(
                            request,
                            ProcessingStage::Encode,
                            ErrorCategory::Output,
                            "output.temp_unavailable",
                            "errors.outputTempUnavailable",
                            "The temporary output file is unavailable.",
                            true,
                            &request.output.output_path,
                        )
                    })?,
                )
                .map_err(|_| {
                    engine_error(
                        request,
                        ProcessingStage::Encode,
                        ErrorCategory::Encoding,
                        "encoding.mozjpeg_failed",
                        "errors.mozJpegFailed",
                        "MozJPEG could not encode the processed image.",
                        false,
                        &request.output.output_path,
                    )
                })?;

                check_cancel(request, ProcessingStage::Encode, context.cancellation)?;
                Ok(transaction)
            },
        )?;

        let output_bytes = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::MetadataFinalize,
            || {
                check_cancel(
                    request,
                    ProcessingStage::MetadataFinalize,
                    context.cancellation,
                )?;
                let bytes = transaction.sync().map_err(|_| {
                    engine_error(
                        request,
                        ProcessingStage::MetadataFinalize,
                        ErrorCategory::Output,
                        "output.temp_validation_failed",
                        "errors.outputValidationFailed",
                        "The encoded temporary output did not pass validation.",
                        true,
                        transaction.temp_path(),
                    )
                })?;
                check_cancel(
                    request,
                    ProcessingStage::MetadataFinalize,
                    context.cancellation,
                )?;
                Ok(bytes)
            },
        )?;

        let commit_outcome = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Commit,
            || {
                check_cancel(request, ProcessingStage::Commit, context.cancellation)?;
                transaction.commit(commit_policy(request)).map_err(|error| {
                    let (code, fallback) = if error.kind() == std::io::ErrorKind::AlreadyExists {
                        (
                            "output.collision",
                            "The output target or backup already exists.",
                        )
                    } else {
                        (
                            "output.commit_failed",
                            "The temporary output could not be committed safely.",
                        )
                    };
                    engine_error(
                        request,
                        ProcessingStage::Commit,
                        ErrorCategory::Output,
                        code,
                        "errors.outputCommitFailed",
                        fallback,
                        true,
                        &request.output.output_path,
                    )
                })
            },
        )?;

        for cleanup_warning in commit_outcome.cleanup_warnings {
            let mut warning = warning(
                "output.cleanup_failed",
                Some(ProcessingStage::Commit),
                "warnings.outputCleanupFailed",
                "The output was committed, but temporary cleanup requires attention.",
            );
            warning
                .message_args
                .insert("detail".to_owned(), cleanup_warning);
            push_warning(context.progress, &mut warnings, warning);
        }

        run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Complete,
            || Ok(()),
        )?;

        Ok(EngineResult {
            job_id: request.job_id.clone(),
            item_id: request.item_id.clone(),
            attempt: request.attempt,
            input_path: request.input_path.clone(),
            output_path: request.output.output_path.clone(),
            input: input_properties,
            output: output_properties,
            input_bytes,
            output_bytes,
            duration_ms: elapsed_ms(total_start),
            metadata: MetadataOutcome {
                embedded_metadata_preserved: false,
                color_profile_preserved: normalize.had_color_profile
                    && !normalize.converted_to_srgb,
                converted_to_srgb: normalize.converted_to_srgb,
                auto_oriented: false,
            },
            warnings,
            stage_durations_ms: durations,
        })
    }
}

impl Engine for LocalEngine {
    fn execute(&self, request: &EngineRequest, context: &EngineContext<'_>) -> EngineOutcome {
        self.execute_inner(request, context)
    }
}

fn apply_operations(
    request: &EngineRequest,
    context: &EngineContext<'_>,
    image: &mut zune_image::image::Image,
    warnings: &mut Vec<EngineWarning>,
) -> Result<(), AppError> {
    let total = request.operations.len() as u64;
    let mut index = 0;
    while index < request.operations.len() {
        check_cancel(request, ProcessingStage::Operations, context.cancellation)?;

        match &request.operations[index] {
            Operation::Resize(config) => {
                let effect = pipeline::apply_resize(image, config).map_err(|_| {
                    engine_error(
                        request,
                        ProcessingStage::Operations,
                        ErrorCategory::Processing,
                        "processing.resize_failed",
                        "errors.resizeFailed",
                        "The resize operation failed.",
                        false,
                        &request.input_path,
                    )
                })?;
                let skipped = match effect {
                    ResizeEffect::SkippedUpscale => Some((
                        "operation.resize_upscale_skipped",
                        "warnings.resizeUpscaleSkipped",
                        "Resize was skipped because upscaling is disabled.",
                    )),
                    ResizeEffect::SkippedDownscale => Some((
                        "operation.resize_downscale_skipped",
                        "warnings.resizeDownscaleSkipped",
                        "Resize was skipped because downscaling is disabled.",
                    )),
                    ResizeEffect::Applied | ResizeEffect::NoChange => None,
                };
                if let Some((code, key, fallback)) = skipped {
                    push_warning(
                        context.progress,
                        warnings,
                        warning(code, Some(ProcessingStage::Operations), key, fallback),
                    );
                }
                index += 1;
            }
            Operation::Quantize(config) => {
                let dithering = request.operations.get(index + 1).and_then(|operation| {
                    if let Operation::Dither(config) = operation {
                        Some(config.strength)
                    } else {
                        None
                    }
                });
                let consumed_dither = dithering.is_some();
                pipeline::apply_quantize(image, config.quality, dithering.flatten()).map_err(
                    |_| {
                        engine_error(
                            request,
                            ProcessingStage::Operations,
                            ErrorCategory::Processing,
                            "processing.quantize_failed",
                            "errors.quantizeFailed",
                            "The quantization operation failed.",
                            false,
                            &request.input_path,
                        )
                    },
                )?;
                push_warning(
                    context.progress,
                    warnings,
                    warning(
                        "operation.quantize_normalized_rgba8",
                        Some(ProcessingStage::Operations),
                        "warnings.quantizeNormalizedRgba8",
                        "Quantization normalized pixels to eight-bit RGBA before processing.",
                    ),
                );
                index += if consumed_dither { 2 } else { 1 };
            }
            Operation::Dither(_) | Operation::PremultiplyAlpha => {
                return Err(engine_error(
                    request,
                    ProcessingStage::Operations,
                    ErrorCategory::Internal,
                    "engine.operation_plan_invalid",
                    "errors.internalOperationPlan",
                    "The validated operation plan became inconsistent.",
                    false,
                    &request.input_path,
                ));
            }
        }

        context.progress.report(EngineProgressEvent::StageProgress {
            progress: crate::domain::ItemProgress {
                stage: ProcessingStage::Operations,
                measure: ProgressMeasure::Fraction {
                    completed: index as u64,
                    total,
                },
            },
        });
        check_cancel(request, ProcessingStage::Operations, context.cancellation)?;
    }
    Ok(())
}

fn run_stage<T>(
    reporter: &dyn ProgressReporter,
    durations: &mut BTreeMap<ProcessingStage, u64>,
    stage: ProcessingStage,
    action: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    reporter.report(EngineProgressEvent::StageStarted { stage });
    let started = Instant::now();
    let result = action()?;
    let duration_ms = elapsed_ms(started);
    durations.insert(stage, duration_ms);
    reporter.report(EngineProgressEvent::StageCompleted { stage, duration_ms });
    Ok(result)
}

fn check_cancel(
    request: &EngineRequest,
    stage: ProcessingStage,
    cancellation: &dyn CancellationProbe,
) -> Result<(), AppError> {
    if !cancellation.is_cancel_requested() {
        return Ok(());
    }
    let mut error = AppError::cancelled(Some(stage));
    error.context = request_context(request, stage, &request.input_path);
    Err(error)
}

fn commit_policy(request: &EngineRequest) -> CommitPolicy {
    match request.output.collision {
        CollisionPolicy::Fail | CollisionPolicy::AutoRename => CommitPolicy::FailIfExists,
        CollisionPolicy::Replace => {
            let backup_path = if same_path(&request.input_path, &request.output.output_path) {
                request.output.source_backup_path.clone()
            } else {
                request.output.existing_output_backup_path.clone()
            };
            CommitPolicy::Replace { backup_path }
        }
    }
}

fn engine_error(
    request: &EngineRequest,
    stage: ProcessingStage,
    category: ErrorCategory,
    code: &str,
    message_key: &str,
    fallback_message: &str,
    retryable: bool,
    path: &Path,
) -> AppError {
    AppError::new(code, category, message_key, fallback_message)
        .with_retryable(retryable)
        .with_context(request_context(request, stage, path))
}

fn warning(
    code: &str,
    stage: Option<ProcessingStage>,
    message_key: &str,
    fallback_message: &str,
) -> EngineWarning {
    EngineWarning {
        code: code.to_owned(),
        stage,
        message_key: message_key.to_owned(),
        message_args: BTreeMap::new(),
        fallback_message: fallback_message.to_owned(),
    }
}

fn push_warning(
    reporter: &dyn ProgressReporter,
    warnings: &mut Vec<EngineWarning>,
    warning: EngineWarning,
) {
    reporter.report(EngineProgressEvent::WarningRaised {
        warning: warning.clone(),
    });
    warnings.push(warning);
}

fn warn_extension_mismatch(
    request: &EngineRequest,
    detected: &ImageFormat,
    reporter: &dyn ProgressReporter,
    warnings: &mut Vec<EngineWarning>,
) {
    let extension = pipeline::extension_format(&request.input_path);
    if extension == ImageFormat::Unknown
        || detected == &ImageFormat::Unknown
        || &extension == detected
    {
        return;
    }

    let mut warning = warning(
        "input.extension_content_mismatch",
        Some(ProcessingStage::Inspect),
        "warnings.extensionContentMismatch",
        "The input extension does not match the detected image content.",
    );
    warning
        .message_args
        .insert("extensionFormat".to_owned(), format!("{extension:?}"));
    warning
        .message_args
        .insert("detectedFormat".to_owned(), format!("{detected:?}"));
    push_warning(reporter, warnings, warning);
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            ColorProfilePolicy, EngineProgressEvent, ItemId, JobId, MetadataPolicy, MozJpegConfig,
            OutputPlan, ResizeFilter, ResizeMode, ResizeOperation, JOB_CONFIG_VERSION,
        },
        engine::{AtomicCancellationToken, NeverCancelled, NoopProgressReporter},
    };
    use std::{
        path::PathBuf,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "neo-rimage-local-engine-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct RecordingReporter(Mutex<Vec<EngineProgressEvent>>);

    impl ProgressReporter for RecordingReporter {
        fn report(&self, event: EngineProgressEvent) {
            self.0.lock().expect("reporter lock").push(event);
        }
    }

    struct CancelAfterEncodeReporter {
        cancellation: AtomicCancellationToken,
    }

    impl ProgressReporter for CancelAfterEncodeReporter {
        fn report(&self, event: EngineProgressEvent) {
            if matches!(
                event,
                EngineProgressEvent::StageCompleted {
                    stage: ProcessingStage::Encode,
                    ..
                }
            ) {
                self.cancellation.request_cancel();
            }
        }
    }

    fn request(directory: &TestDirectory) -> EngineRequest {
        let input = directory.path("input.ppm");
        // Binary PPM is a compact dependency-free fixture for zune-image decode.
        fs::write(
            &input,
            b"P6\n2 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\xff",
        )
        .expect("write input fixture");

        EngineRequest {
            job_id: JobId::new("job-1"),
            item_id: ItemId::new("item-1"),
            attempt: 1,
            config_version: JOB_CONFIG_VERSION,
            input_path: input,
            output: OutputPlan {
                output_path: directory.path("output.jpg"),
                collision: CollisionPolicy::Fail,
                source_backup_path: None,
                existing_output_backup_path: None,
            },
            encoder: EncoderConfig::MozJpeg(MozJpegConfig::default()),
            operations: vec![Operation::Resize(ResizeOperation {
                mode: ResizeMode::Exact {
                    width: 1,
                    height: 1,
                },
                filter: ResizeFilter::Nearest,
                allow_upscale: false,
                allow_downscale: true,
            })],
            metadata: MetadataPolicy {
                color_profile: ColorProfilePolicy::PreserveWhenSupported,
                ..MetadataPolicy::default()
            },
        }
    }

    #[test]
    fn processes_ppm_to_mozjpeg_without_tauri() {
        let directory = TestDirectory::new();
        let request = request(&directory);
        let reporter = RecordingReporter::default();

        let result = LocalEngine::new()
            .process(&request, &reporter, &NeverCancelled)
            .expect("process image");

        assert_eq!((result.output.width, result.output.height), (1, 1));
        assert!(result.output_bytes > 0);
        let bytes = fs::read(&result.output_path).expect("read JPEG output");
        assert_eq!(&bytes[..2], &[0xff, 0xd8]);
        assert!(reporter
            .0
            .lock()
            .expect("reporter lock")
            .iter()
            .any(|event| matches!(
                event,
                EngineProgressEvent::StageCompleted {
                    stage: ProcessingStage::Commit,
                    ..
                }
            )));
    }

    #[test]
    fn cancellation_before_preflight_creates_no_output() {
        let directory = TestDirectory::new();
        let request = request(&directory);
        let cancellation = AtomicCancellationToken::default();
        cancellation.request_cancel();

        let error = LocalEngine::new()
            .process(&request, &NoopProgressReporter, &cancellation)
            .expect_err("request should be cancelled");

        assert_eq!(error.category, ErrorCategory::Cancelled);
        assert!(!request.output.output_path.exists());
    }

    #[test]
    fn cancellation_after_encode_removes_temporary_output() {
        let directory = TestDirectory::new();
        let request = request(&directory);
        let cancellation = AtomicCancellationToken::default();
        let reporter = CancelAfterEncodeReporter {
            cancellation: cancellation.clone(),
        };

        let error = LocalEngine::new()
            .process(&request, &reporter, &cancellation)
            .expect_err("request should stop before commit");

        assert_eq!(error.category, ErrorCategory::Cancelled);
        assert!(!request.output.output_path.exists());
        assert!(fs::read_dir(&directory.0)
            .expect("read test directory")
            .all(|entry| !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".neo-rimage-")));
    }
}
