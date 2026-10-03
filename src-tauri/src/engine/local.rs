use super::{
    output::{CommitOutcome, CommitPolicy, OutputTransaction},
    pipeline::{self, ResizeEffect},
    runtime::EngineContext,
    validation::{request_context, same_path, validate_request},
};
use crate::domain::{
    AppError, CancellationProbe, CollisionPolicy, ColorProfilePolicy, EmbeddedMetadataPolicy,
    EngineOutcome, EngineProgressEvent, EngineRequest, EngineResult, EngineWarning, ErrorCategory,
    ImageFormat, MetadataOutcome, Operation, ProcessingStage, ProgressMeasure, ProgressReporter,
};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

pub trait Engine: Send + Sync {
    fn prepare(
        &self,
        request: &EngineRequest,
        budget: u64,
    ) -> Result<super::PreparedInput, super::PrepareError>;
    fn execute_prepared(
        &self,
        request: &EngineRequest,
        plan: &super::PreparedInput,
        context: &EngineContext<'_>,
    ) -> EngineOutcome;
    fn execute(&self, request: &EngineRequest, context: &EngineContext<'_>) -> EngineOutcome {
        validate_request(request)?;
        check_cancel(request, ProcessingStage::Preflight, context.cancellation)?;
        let budget = rimage::limits::SystemBudget::probe(1).per_image_bytes();
        let plan = self.prepare(request, budget).map_err(prepare_error)?;
        if plan.memory_bytes > budget {
            return Err(AppError::new(
                "input.memory_budget_exceeded",
                ErrorCategory::Input,
                "errors.memoryBudgetExceeded",
                "The image exceeds the shared memory budget.",
            ));
        }
        self.execute_prepared(request, &plan, context)
    }
}

fn prepare_error(error: super::PrepareError) -> AppError {
    match error {
        super::PrepareError::Failed(error) => error,
        super::PrepareError::NeedsBudget(_) => AppError::new(
            "input.memory_budget_exceeded",
            ErrorCategory::Input,
            "errors.memoryBudgetExceeded",
            "Preparation exceeds the granted memory budget.",
        ),
    }
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

    fn execute_inner(
        &self,
        request: &EngineRequest,
        plan: &super::PreparedInput,
        context: &EngineContext<'_>,
    ) -> EngineOutcome {
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
                inspect_input(request)
            },
        )?;

        let mut image = run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Decode,
            || {
                check_cancel(request, ProcessingStage::Decode, context.cancellation)?;
                let image = super::input::decode(request, plan).map_err(prepare_error)?;
                check_cancel(request, ProcessingStage::Decode, context.cancellation)?;
                Ok(image)
            },
        )?;

        let input_properties = plan.properties.clone();
        if input_properties.format == ImageFormat::Avif {
            push_warning(
                context.progress,
                &mut warnings,
                warning(
                    "input.avif_icc_limited",
                    Some(ProcessingStage::Decode),
                    "warnings.avifIccLimited",
                    "AVIF ICC profiles are not preserved by this decoder.",
                ),
            );
            if input_properties.bit_depth.is_some_and(|bits| bits > 8) {
                push_warning(
                    context.progress,
                    &mut warnings,
                    warning(
                        "input.avif_depth_reduced",
                        Some(ProcessingStage::Decode),
                        "warnings.avifDepthReduced",
                        "High-bit-depth SDR AVIF was converted to RGBA8.",
                    ),
                );
            }
        }
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
                    "EXIF preservation is not implemented by the current encoder adapters.",
                ),
            );
        }
        if normalize.had_color_profile
            && !normalize.converted_to_srgb
            && request.metadata.color_profile == ColorProfilePolicy::PreserveWhenSupported
            && !pipeline::preserves_icc_profile(&request.encoder)
        {
            push_warning(
                context.progress,
                &mut warnings,
                warning(
                    "metadata.icc_not_preserved",
                    Some(ProcessingStage::Normalize),
                    "warnings.iccNotPreserved",
                    "The selected encoder cannot preserve the source ICC profile.",
                ),
            );
        }

        run_stage(
            context.progress,
            &mut durations,
            ProcessingStage::Operations,
            || {
                apply_operations(
                    request,
                    context,
                    &mut image,
                    &mut warnings,
                    usize::from(plan.consumed_resize),
                )
            },
        )?;

        let output_properties =
            pipeline::output_properties(&image, &request.encoder).map_err(|_| {
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
                let transaction = encode_to_transaction(request, &image)?;
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
                let bytes = sync_transaction(request, &mut transaction)?;
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
                commit_transaction(request, transaction)
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
                    && !normalize.converted_to_srgb
                    && pipeline::preserves_icc_profile(&request.encoder),
                converted_to_srgb: normalize.converted_to_srgb,
                auto_oriented: false,
            },
            warnings,
            stage_durations_ms: durations,
        })
    }
}

impl Engine for LocalEngine {
    fn prepare(
        &self,
        request: &EngineRequest,
        budget: u64,
    ) -> Result<super::PreparedInput, super::PrepareError> {
        validate_request(request).map_err(super::PrepareError::Failed)?;
        super::input::prepare(request, budget)
    }
    fn execute_prepared(
        &self,
        request: &EngineRequest,
        plan: &super::PreparedInput,
        context: &EngineContext<'_>,
    ) -> EngineOutcome {
        self.execute_inner(request, plan, context)
    }
}

fn inspect_input(request: &EngineRequest) -> Result<u64, AppError> {
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
}

fn encode_to_transaction(
    request: &EngineRequest,
    image: &zune_image::image::Image,
) -> Result<OutputTransaction, AppError> {
    let mut transaction =
        OutputTransaction::begin(&request.output.output_path, request.item_id.as_str()).map_err(
            |_| {
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
            },
        )?;

    pipeline::encode(
        image,
        &request.encoder,
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
            "encoding.encoder_failed",
            "errors.encodingFailed",
            "The selected encoder could not encode the processed image.",
            false,
            &request.output.output_path,
        )
    })?;

    Ok(transaction)
}

fn sync_transaction(
    request: &EngineRequest,
    transaction: &mut OutputTransaction,
) -> Result<u64, AppError> {
    transaction.sync().map_err(|_| {
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
    })
}

fn commit_transaction(
    request: &EngineRequest,
    transaction: OutputTransaction,
) -> Result<CommitOutcome, AppError> {
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
}

fn apply_operations(
    request: &EngineRequest,
    context: &EngineContext<'_>,
    image: &mut zune_image::image::Image,
    warnings: &mut Vec<EngineWarning>,
    start_index: usize,
) -> Result<(), AppError> {
    let total = request.operations.len() as u64;
    let mut index = start_index;
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

// Keeping the complete stable error vocabulary visible at call sites is more
// useful here than hiding its fields behind a short-lived builder type.
#[allow(clippy::too_many_arguments)]
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
            AvifAlphaMode, AvifColorSpace, AvifConfig, ColorProfilePolicy, EncoderConfig,
            EngineProgressEvent, ItemId, JobId, JpegConfig, MetadataPolicy, MozJpegConfig,
            OutputPlan, OxiPngConfig, ResizeFilter, ResizeMode, ResizeOperation, WebPConfig,
            JOB_CONFIG_VERSION,
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
    fn every_advertised_encoder_produces_its_real_file_format() {
        let directory = TestDirectory::new();
        let encoders = [
            (
                "mozjpeg.jpg",
                EncoderConfig::MozJpeg(MozJpegConfig::default()),
                ImageFormat::Jpeg,
            ),
            (
                "jpeg.jpg",
                EncoderConfig::Jpeg(JpegConfig {
                    quality: 80.0,
                    progressive: true,
                }),
                ImageFormat::Jpeg,
            ),
            (
                "avif.avif",
                EncoderConfig::Avif(AvifConfig {
                    quality: 50.0,
                    alpha_quality: None,
                    speed: 10,
                    color_space: AvifColorSpace::YCbCr,
                    alpha_mode: AvifAlphaMode::UnassociatedClean,
                }),
                ImageFormat::Avif,
            ),
            (
                "oxipng.png",
                EncoderConfig::OxiPng(OxiPngConfig {
                    interlace: false,
                    effort: 0,
                }),
                ImageFormat::Png,
            ),
            (
                "webp.webp",
                EncoderConfig::WebP(WebPConfig {
                    lossless: false,
                    quality: 75.0,
                    slight_loss: 0,
                    exact: false,
                }),
                ImageFormat::WebP,
            ),
            ("jpeg-xl.jxl", EncoderConfig::JpegXl, ImageFormat::JpegXl),
            ("png.png", EncoderConfig::Png, ImageFormat::Png),
            (
                "farbfeld.ff",
                EncoderConfig::Farbfeld,
                ImageFormat::Farbfeld,
            ),
            ("ppm.ppm", EncoderConfig::Ppm, ImageFormat::Ppm),
            ("qoi.qoi", EncoderConfig::Qoi, ImageFormat::Qoi),
        ];

        for (name, encoder, expected_format) in encoders {
            let mut request = request(&directory);
            let expected_dimensions = if name == "farbfeld.ff" {
                let mut fixture = b"P6\n8 8\n255\n".to_vec();
                fixture.extend((0..64).flat_map(|index| {
                    [
                        (index * 3) as u8,
                        (255usize.saturating_sub(index * 2)) as u8,
                        (index * 5) as u8,
                    ]
                }));
                fs::write(&request.input_path, fixture).expect("write Farbfeld fixture");
                (8, 8)
            } else {
                (2, 2)
            };
            request.encoder = encoder;
            request.operations.clear();
            request.output.output_path = directory.path(name);

            let result = LocalEngine::new()
                .process(&request, &NoopProgressReporter, &NeverCancelled)
                .unwrap_or_else(|error| panic!("{name} failed: {error:?}"));
            assert_eq!(result.output.format, expected_format, "{name}");
            let bytes = fs::read(&result.output_path).expect("read encoded output");
            assert_format_signature(name, &bytes);
            // zune-farbfeld 0.5.2 rejects even a valid 8x8 output with
            // `Too small output buffer size`; validate its complete raw layout
            // below instead of turning that upstream decoder bug into a false
            // encoder failure.
            if name != "farbfeld.ff" {
                let mut decode_request = request.clone();
                decode_request.input_path = result.output_path.clone();
                decode_request.output.output_path = directory.path(&format!("redecoded-{name}"));
                decode_request.operations.clear();
                let plan = LocalEngine::new()
                    .prepare(&decode_request, 64 * 1024 * 1024)
                    .unwrap();
                let decoded = super::super::input::decode(&decode_request, &plan)
                    .unwrap_or_else(|error| panic!("{name} could not be decoded again: {error:?}"));
                assert_eq!(decoded.dimensions(), expected_dimensions, "{name}");
            }
        }
    }

    #[test]
    fn avif_twelve_bit_sdr_and_hdr_rejection_use_real_bitstreams() {
        use rav1e::prelude::*;
        for transfer in [
            TransferCharacteristics::SRGB,
            TransferCharacteristics::SMPTE2084,
            TransferCharacteristics::HLG,
        ] {
            let config = EncoderConfig {
                width: 16,
                height: 16,
                bit_depth: 12,
                still_picture: true,
                chroma_sampling: ChromaSampling::Cs444,
                color_description: Some(ColorDescription {
                    color_primaries: ColorPrimaries::BT709,
                    transfer_characteristics: transfer,
                    matrix_coefficients: MatrixCoefficients::BT709,
                }),
                ..EncoderConfig::with_speed_preset(10)
            };
            let mut context = Config::new()
                .with_encoder_config(config)
                .with_threads(1)
                .new_context::<u16>()
                .unwrap();
            let mut frame = context.new_frame();
            for plane in &mut frame.planes {
                plane.data.fill(2048);
            }
            context.send_frame(frame).unwrap();
            context.flush();
            let packet = loop {
                match context.receive_packet() {
                    Ok(packet) => break packet,
                    Err(EncoderStatus::Encoded | EncoderStatus::NeedMoreData) => continue,
                    Err(error) => panic!("{error:?}"),
                }
            };
            let bytes = avif_serialize::Aviffy::new().to_vec(&packet.data, None, 16, 16, 12);
            let directory = TestDirectory::new();
            let mut request = request(&directory);
            request.input_path = directory.path("input.avif");
            fs::write(&request.input_path, bytes).unwrap();
            let result = LocalEngine::new().prepare(&request, 64 * 1024 * 1024);
            if transfer == TransferCharacteristics::SRGB {
                let plan = result.unwrap();
                assert_eq!(plan.properties.bit_depth, Some(12));
                assert_eq!(
                    super::super::input::decode(&request, &plan)
                        .unwrap()
                        .depth(),
                    zune_core::bit_depth::BitDepth::Eight
                );
                let mut container = avif_serialize::Aviffy::new();
                container.set_transfer_characteristics(
                    avif_serialize::constants::TransferCharacteristics::Smpte2084,
                );
                let bytes = container.to_vec(&packet.data, None, 16, 16, 12);
                fs::write(&request.input_path, bytes).unwrap();
                assert!(
                    matches!(LocalEngine::new().prepare(&request, 64 * 1024 * 1024),
                    Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.avif_hdr")
                );
            } else {
                assert!(
                    matches!(result, Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.avif_hdr")
                );
            }
        }
    }

    #[test]
    fn svg_invalid_embedded_resources_and_missing_references_fail_explicitly() {
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("embedded.svg");
        for image in [
            r#"<image href="data:image/png;base64,%%%" width="10" height="10"/>"#,
            r#"<image href="data:image/png;base64," width="10" height="10"/>"#,
            r#"<image width="10" height="10"/>"#,
            r#"<image href="missing.png" display="none" width="10" height="10"/>"#,
        ] {
            fs::write(&request.input_path, format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{image}</svg>"#)).unwrap();
            assert!(
                matches!(LocalEngine::new().prepare(&request, 64 * 1024 * 1024),
                Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.svg_resource_invalid")
            );
        }
    }

    #[test]
    fn tiff_jpeg_and_fax_compression_decode_with_header_admission() {
        use std::io::Cursor;
        use tiff::{encoder::TiffEncoder, tags::Tag};
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("compressed.tif");
        request.operations.clear();
        let mut jpeg = Vec::new();
        jpeg_encoder::Encoder::new(&mut jpeg, 90)
            .encode(&[128; 16 * 16 * 3], 16, 16, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        let mut fax = fax::encoder::Encoder::new(fax::VecWriter::new());
        for row in 0..16 {
            fax.encode_line(
                (0..16).map(|column| {
                    if (column + row) % 2 == 0 {
                        fax::Color::White
                    } else {
                        fax::Color::Black
                    }
                }),
                16,
            )
            .unwrap();
        }
        let fax = fax.finish().unwrap().finish();
        for (compression, samples, bits, photometric, data) in [
            (7_u16, 3_u16, 8_u16, 6_u16, jpeg),
            (4_u16, 1_u16, 1_u16, 0_u16, fax),
        ] {
            request.output.output_path = directory.path(&format!("compression-{compression}.jpg"));
            let mut bytes = Cursor::new(Vec::new());
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.image_directory().unwrap();
            let offset = image.write_data(data.as_slice()).unwrap();
            image.write_tag(Tag::ImageWidth, 16_u32).unwrap();
            image.write_tag(Tag::ImageLength, 16_u32).unwrap();
            image
                .write_tag(Tag::BitsPerSample, vec![bits; samples as usize].as_slice())
                .unwrap();
            image.write_tag(Tag::Compression, compression).unwrap();
            image
                .write_tag(Tag::PhotometricInterpretation, photometric)
                .unwrap();
            image.write_tag(Tag::SamplesPerPixel, samples).unwrap();
            image.write_tag(Tag::RowsPerStrip, 16_u32).unwrap();
            image.write_tag(Tag::StripOffsets, offset as u32).unwrap();
            image
                .write_tag(Tag::StripByteCounts, data.len() as u32)
                .unwrap();
            image.write_tag(Tag::PlanarConfiguration, 1_u16).unwrap();
            if compression == 7 {
                image
                    .write_tag(
                        Tag::Unknown(529),
                        &[
                            tiff::encoder::Rational { n: 299, d: 1000 },
                            tiff::encoder::Rational { n: 587, d: 1000 },
                            tiff::encoder::Rational { n: 114, d: 1000 },
                        ][..],
                    )
                    .unwrap();
            }
            image.finish().unwrap();
            fs::write(&request.input_path, bytes.into_inner()).unwrap();
            let plan = LocalEngine::new()
                .prepare(&request, 64 * 1024 * 1024)
                .unwrap();
            let decoded = super::super::input::decode(&request, &plan).unwrap();
            assert_eq!(decoded.dimensions(), (16, 16));
            assert_eq!(decoded.flatten_to_u8()[0].len(), 16 * 16 * samples as usize);
            if compression == 4 {
                let pixels = decoded.flatten_to_u8();
                assert!(pixels[0].iter().all(|value| matches!(value, 0 | 255)));
                assert_ne!(pixels[0][0], pixels[0][1]);
            }
            LocalEngine::new()
                .execute_prepared(
                    &request,
                    &plan,
                    &EngineContext::new(&NoopProgressReporter, &NeverCancelled),
                )
                .unwrap_or_else(|error| panic!("compression {compression}: {error:?}"));
        }
    }

    fn assert_format_signature(name: &str, bytes: &[u8]) {
        match name {
            "mozjpeg.jpg" => assert_eq!(&bytes[..2], &[0xff, 0xd8]),
            "jpeg.jpg" => {
                assert_eq!(&bytes[..2], &[0xff, 0xd8]);
                assert!(
                    bytes.windows(2).any(|marker| marker == [0xff, 0xc2]),
                    "JPEG progressive option must emit a progressive SOF marker"
                );
            }
            "avif.avif" => {
                assert_eq!(&bytes[4..8], b"ftyp");
                assert!(bytes.windows(4).any(|brand| brand == b"avif"));
            }
            "oxipng.png" | "png.png" => {
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n")
            }
            "webp.webp" => {
                assert_eq!(&bytes[..4], b"RIFF");
                assert_eq!(&bytes[8..12], b"WEBP");
            }
            "jpeg-xl.jxl" => {
                assert!(
                    bytes.starts_with(&[0xff, 0x0a])
                        || bytes.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n")
                );
            }
            "farbfeld.ff" => {
                assert!(bytes.starts_with(b"farbfeld"));
                assert_eq!(u32::from_be_bytes(bytes[8..12].try_into().unwrap()), 8);
                assert_eq!(u32::from_be_bytes(bytes[12..16].try_into().unwrap()), 8);
                assert_eq!(bytes.len(), 16 + 8 * 8 * 8);
            }
            "ppm.ppm" => assert!(matches!(&bytes[..2], b"P5" | b"P6" | b"P7")),
            "qoi.qoi" => assert!(bytes.starts_with(b"qoif")),
            _ => panic!("missing signature assertion for {name}"),
        }
    }

    #[test]
    fn avif_grid_and_animation_fixtures_are_rejected_during_preparation() {
        for data in [
            include_bytes!("../../test-data/avif/colors-animated-8bpc.avif").as_slice(),
            include_bytes!("../../test-data/avif/sofa_grid1x5_420.avif").as_slice(),
        ] {
            let directory = TestDirectory::new();
            let mut request = request(&directory);
            fs::write(&request.input_path, data).unwrap();
            request.operations.clear();
            assert!(
                matches!(LocalEngine::new().prepare(&request, 64 * 1024 * 1024),
                Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.avif_unsupported" || error.code.0 == "input.animation_unsupported")
            );
            assert!(!request.output.output_path.exists());
        }
    }

    #[test]
    fn svg_local_resources_are_verified_and_gzip_cap_is_enforced() {
        use std::io::Write;
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("drawing.svg");
        fs::create_dir(directory.path("resources")).unwrap();
        fs::write(directory.path("resources/child.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#).unwrap();
        fs::write(&request.input_path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="resources/child.svg" width="10" height="10"/></svg>"#).unwrap();
        let plan = LocalEngine::new()
            .prepare(&request, 64 * 1024 * 1024)
            .unwrap();
        assert_eq!(
            super::super::input::decode(&request, &plan)
                .unwrap()
                .dimensions(),
            (1, 1)
        );
        fs::write(
            directory.path("resources/child.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#,
        )
        .unwrap();
        assert!(
            matches!(super::super::input::decode(&request, &plan), Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.changed")
        );
        fs::write(directory.path("resources/child.svg"), b"invalid resource").unwrap();
        assert!(matches!(super::super::input::decode(&request, &plan),
            Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.changed" && error.retryable));
        let mut compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let block = vec![b' '; 1024 * 1024];
        for _ in 0..65 {
            compressed.write_all(&block).unwrap();
        }
        fs::write(&request.input_path, compressed.finish().unwrap()).unwrap();
        assert!(
            matches!(LocalEngine::new().prepare(&request, 5 * 1024 * 1024 * 1024),
            Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.svg_resource_invalid")
        );
    }

    #[cfg(windows)]
    #[test]
    fn svg_directory_junction_cannot_escape_resource_root() {
        let directory = TestDirectory::new();
        let outside = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("drawing.svg");
        fs::write(
            outside.path("image.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#,
        )
        .unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(directory.path("escape"))
            .arg(&outside.0)
            .status()
            .unwrap();
        assert!(status.success());
        fs::write(&request.input_path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="escape/image.svg" width="10" height="10"/></svg>"#).unwrap();
        let result = LocalEngine::new().prepare(&request, 64 * 1024 * 1024);
        fs::remove_dir(directory.path("escape")).unwrap();
        assert!(
            matches!(result, Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.svg_resource_invalid")
        );
    }

    #[test]
    fn avif_transparency_and_high_depth_are_prepared_without_decoding() {
        for depth in [ravif::BitDepth::Eight, ravif::BitDepth::Ten] {
            let directory = TestDirectory::new();
            let mut request = request(&directory);
            let pixels = vec![ravif::RGBA8::new(255, 0, 0, 128); 16 * 16];
            let encoded = ravif::Encoder::new()
                .with_speed(10)
                .with_bit_depth(depth)
                .encode_rgba(ravif::Img::new(&pixels, 16, 16))
                .unwrap();
            request.input_path = directory.path("avif-disguised.png");
            fs::write(&request.input_path, &encoded.avif_file).unwrap();
            request.operations.clear();
            let plan = LocalEngine::new()
                .prepare(&request, 64 * 1024 * 1024)
                .unwrap();
            assert_eq!(plan.properties.format, ImageFormat::Avif);
            assert_eq!(plan.properties.has_alpha, Some(true));
            let result = LocalEngine::new()
                .execute_prepared(
                    &request,
                    &plan,
                    &EngineContext::new(&NoopProgressReporter, &NeverCancelled),
                )
                .unwrap();
            assert_eq!((result.input.width, result.input.height), (16, 16));
            assert!(result
                .warnings
                .iter()
                .any(|warning| warning.code == "input.avif_icc_limited"));
            assert_eq!(
                result
                    .warnings
                    .iter()
                    .any(|warning| warning.code == "input.avif_depth_reduced"),
                matches!(depth, ravif::BitDepth::Ten)
            );
        }
    }

    #[test]
    fn changed_file_and_animated_headers_fail_before_output_allocation() {
        let directory = TestDirectory::new();
        let request = request(&directory);
        let engine = LocalEngine::new();
        let plan = engine.prepare(&request, 64 * 1024 * 1024).unwrap();
        fs::write(&request.input_path, b"changed").unwrap();
        let error = engine
            .execute_prepared(
                &request,
                &plan,
                &EngineContext::new(&NoopProgressReporter, &NeverCancelled),
            )
            .unwrap_err();
        assert_eq!(error.code.0, "input.changed");
        assert!(error.retryable);
        assert!(!request.output.output_path.exists());
        let mut request = request;
        request.input_path = directory.path("animated.webp");
        let mut bytes = b"RIFF\x16\0\0\0WEBPVP8X\x0a\0\0\0\x02\0\0\0\0\0\0\0\0\0".to_vec();
        fs::write(&request.input_path, &bytes).unwrap();
        assert!(
            matches!(engine.prepare(&request, 64 * 1024 * 1024), Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.animation_unsupported")
        );
        bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x08acTL\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
        fs::write(&request.input_path, bytes).unwrap();
        assert!(
            matches!(engine.prepare(&request, 64 * 1024 * 1024), Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.animation_unsupported")
        );
    }

    #[test]
    fn tiff_depth_compression_and_multipage_are_inspected_before_pixels() {
        use std::io::Cursor;
        use tiff::encoder::{colortype, Compression, TiffEncoder};
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("input.tiff");
        request.operations.clear();
        for compression in [
            Compression::Uncompressed,
            Compression::Lzw,
            Compression::Deflate(Default::default()),
        ] {
            let mut bytes = Cursor::new(Vec::new());
            TiffEncoder::new(&mut bytes)
                .unwrap()
                .with_compression(compression)
                .write_image::<colortype::RGBA16>(4, 4, &[32000; 64])
                .unwrap();
            fs::write(&request.input_path, bytes.into_inner()).unwrap();
            let plan = LocalEngine::new()
                .prepare(&request, 64 * 1024 * 1024)
                .unwrap();
            assert_eq!(plan.properties.bit_depth, Some(16));
            assert_eq!(plan.properties.has_alpha, Some(true));
            super::super::input::decode(&request, &plan).unwrap();
        }
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .unwrap()
            .write_image::<colortype::RGB32Float>(4, 4, &[0.5; 48])
            .unwrap();
        fs::write(&request.input_path, bytes.into_inner()).unwrap();
        let plan = LocalEngine::new()
            .prepare(&request, 64 * 1024 * 1024)
            .unwrap();
        assert_eq!(plan.properties.bit_depth, Some(32));
        assert_eq!(
            super::super::input::decode(&request, &plan)
                .unwrap()
                .depth(),
            zune_core::bit_depth::BitDepth::Float32
        );
        let mut bytes = Cursor::new(Vec::new());
        let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
        for _ in 0..2 {
            encoder
                .write_image::<colortype::RGB8>(4, 4, &[128; 48])
                .unwrap();
        }
        fs::write(&request.input_path, bytes.into_inner()).unwrap();
        assert!(
            matches!(LocalEngine::new().prepare(&request, 64 * 1024 * 1024), Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.tiff_multipage")
        );
    }

    #[test]
    fn svg_renders_first_resize_once_and_keeps_intrinsic_properties() {
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("drawing.svg");
        fs::write(&request.input_path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="red"/><text x="10" y="20">中文 日本語 ABC</text></svg>"#).unwrap();
        request.operations = vec![
            Operation::Resize(ResizeOperation {
                mode: ResizeMode::Percentage { percent: 50.0 },
                filter: ResizeFilter::Lanczos3,
                allow_upscale: false,
                allow_downscale: true,
            }),
            Operation::Resize(ResizeOperation {
                mode: ResizeMode::Percentage { percent: 50.0 },
                filter: ResizeFilter::Lanczos3,
                allow_upscale: false,
                allow_downscale: true,
            }),
        ];
        let result = LocalEngine::new()
            .process(&request, &NoopProgressReporter, &NeverCancelled)
            .unwrap();
        assert_eq!(result.input.format, ImageFormat::Svg);
        assert_eq!((result.input.width, result.input.height), (200, 100));
        assert_eq!((result.output.width, result.output.height), (50, 25));
    }

    #[test]
    fn svg_resource_policy_rejects_network_traversal_scripts_missing_and_deep_resources() {
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("drawing.svg");
        for resource in [
            "https://example.com/image.png",
            "../outside.png",
            "missing.png",
        ] {
            fs::write(&request.input_path, format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="{resource}" width="10" height="10"/></svg>"#)).unwrap();
            assert!(
                matches!(LocalEngine::new().prepare(&request, 64 * 1024 * 1024), Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.svg_resource_invalid")
            );
        }
        fs::write(
            &request.input_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>bad()</script></svg>"#,
        )
        .unwrap();
        assert!(LocalEngine::new()
            .prepare(&request, 64 * 1024 * 1024)
            .is_err());
        for index in 0..10 {
            fs::write(directory.path(&format!("nested-{index}.svg")), format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="nested-{}.svg" width="10" height="10"/></svg>"#, index + 1)).unwrap();
        }
        request.input_path = directory.path("nested-0.svg");
        assert!(
            matches!(LocalEngine::new().prepare(&request, 64 * 1024 * 1024), Err(super::super::PrepareError::Failed(error)) if error.code.0 == "input.svg_resource_invalid")
        );
    }

    #[test]
    fn svgz_expansion_requests_more_preparation_budget_without_waiting() {
        use std::io::Write;
        let directory = TestDirectory::new();
        let mut request = request(&directory);
        request.input_path = directory.path("drawing.svgz");
        let mut data = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        data.write_all(br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><!--"#)
            .unwrap();
        data.write_all(&vec![b' '; 1024 * 1024]).unwrap();
        data.write_all(b"--></svg>").unwrap();
        fs::write(&request.input_path, data.finish().unwrap()).unwrap();
        assert!(
            matches!(LocalEngine::new().prepare(&request, 16 * 1024 * 1024), Err(super::super::PrepareError::NeedsBudget(required)) if required > 16 * 1024 * 1024)
        );
        let plan = LocalEngine::new()
            .prepare(&request, 128 * 1024 * 1024)
            .unwrap();
        assert_eq!(plan.properties.format, ImageFormat::Svg);
        assert_eq!(
            super::super::input::decode(&request, &plan)
                .unwrap()
                .dimensions(),
            (1, 1)
        );
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
