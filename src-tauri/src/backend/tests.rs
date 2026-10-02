use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    domain::{
        AppError, AvifAlphaMode, AvifColorSpace, AvifConfig, BackupPolicy, CollisionPolicy,
        ColorProfilePolicy, CorrelationId, CreateJobCommand, EmbeddedMetadataPolicy, EncoderConfig,
        EngineOutcome, EngineRequest, ErrorCategory, InputAcceptancePolicy, InputResource,
        InputResourceKind, ItemId, ItemSpec, JobId, JobSpec, JpegConfig, MetadataPolicy,
        MozJpegConfig, OutputLocation, OutputPlan, OutputPolicy, OxiPngConfig,
        ProcessingReportPolicy, TimestampMs, WebPConfig, IPC_SCHEMA_VERSION, JOB_CONFIG_VERSION,
    },
    engine::{Engine, EngineContext},
    jobs::{
        CancellationToken, ExecutionContext, ExecutionOutcome, ExecutionTask, Executor,
        ProgressReporter,
    },
};

use super::{BackendService, LocalEngineExecutor, NormalizationOptions, RequestNormalizer};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "neo-rimage-backend-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn preserves_directory_structure_and_derives_mozjpeg_extension() {
    let directory = TestDirectory::new("paths");
    let input_root = directory.path("input");
    let nested = input_root.join("nested");
    fs::create_dir_all(&nested).expect("create nested input");
    fs::write(nested.join("photo.png"), b"fixture").expect("write input");
    let output_root = directory.path("output");
    let request = request(
        vec![InputResource {
            path: input_root.to_string_lossy().into_owned(),
            kind: InputResourceKind::Directory,
            scan_recursively: true,
        }],
        OutputPolicy {
            location: OutputLocation::Directory(output_root.to_string_lossy().into_owned()),
            preserve_structure: true,
            suffix: "-small".to_owned(),
            collision: CollisionPolicy::Fail,
            source_backup: BackupPolicy::Disabled,
            existing_output_backup: BackupPolicy::Disabled,
        },
    );

    let normalized = RequestNormalizer::default()
        .normalize(request)
        .expect("normalize request");

    assert_eq!(normalized.submission.items.len(), 1);
    assert_eq!(
        normalized.submission.items[0].output.output_path,
        output_root.join("nested").join("photo-small.jpg")
    );
}

#[test]
fn derives_the_canonical_output_extension_for_every_encoder() {
    let directory = TestDirectory::new("encoder-extensions");
    let input = directory.path("photo.png");
    fs::write(&input, b"fixture").expect("write input");
    let encoders = [
        (EncoderConfig::MozJpeg(MozJpegConfig::default()), "jpg"),
        (
            EncoderConfig::Jpeg(JpegConfig {
                quality: 80.0,
                progressive: false,
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
        let mut request = request(
            vec![InputResource {
                path: input.to_string_lossy().into_owned(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            }],
            same_directory_output("-encoded"),
        );
        request.encoder = encoder;
        let normalized = RequestNormalizer::default()
            .normalize(request)
            .expect("normalize encoder output");
        assert_eq!(
            normalized.submission.items[0]
                .output
                .output_path
                .extension()
                .and_then(|value| value.to_str()),
            Some(extension)
        );
    }
}

#[test]
fn canonical_duplicate_inputs_are_scheduled_once() {
    let directory = TestDirectory::new("deduplicate");
    let input_root = directory.path("input");
    fs::create_dir_all(&input_root).expect("create input directory");
    let file = input_root.join("photo.png");
    fs::write(&file, b"fixture").expect("write input");
    let request = request(
        vec![
            InputResource {
                path: file.to_string_lossy().into_owned(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            },
            InputResource {
                path: input_root.to_string_lossy().into_owned(),
                kind: InputResourceKind::Directory,
                scan_recursively: true,
            },
        ],
        same_directory_output("-copy"),
    );

    let normalized = RequestNormalizer::default()
        .normalize(request)
        .expect("normalize request");

    assert_eq!(normalized.submission.items.len(), 1);
    assert!(normalized.rejected_inputs.is_empty());
}

#[test]
fn explicit_input_resource_order_is_preserved() {
    let directory = TestDirectory::new("input-order");
    let first = directory.path("z-first.png");
    let second = directory.path("a-second.png");
    fs::write(&first, b"fixture").expect("write first input");
    fs::write(&second, b"fixture").expect("write second input");
    let request = request(
        vec![
            InputResource {
                path: first.to_string_lossy().into_owned(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            },
            InputResource {
                path: second.to_string_lossy().into_owned(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            },
        ],
        same_directory_output("-copy"),
    );

    let normalized = RequestNormalizer::default()
        .normalize(request)
        .expect("normalize request");

    assert_eq!(
        normalized.submission.items[0].input_path,
        fs::canonicalize(first).expect("canonical first input")
    );
    assert_eq!(
        normalized.submission.items[1].input_path,
        fs::canonicalize(second).expect("canonical second input")
    );
}

#[test]
fn invalid_schema_is_rejected_before_filesystem_access() {
    let mut request = request(
        vec![InputResource {
            path: "Z:/this/path/does/not/exist.png".to_owned(),
            kind: InputResourceKind::File,
            scan_recursively: false,
        }],
        same_directory_output("-copy"),
    );
    request.schema_version = IPC_SCHEMA_VERSION + 1;

    let error = RequestNormalizer::default()
        .normalize(request)
        .expect_err("schema must be rejected");

    assert_eq!(error.category, ErrorCategory::Protocol);
    assert_eq!(error.code.0, "protocol.schema_version_unsupported");
}

#[test]
fn executor_maps_engine_cancellation_to_manager_cancellation() {
    let engine = Arc::new(FakeEngine::new(Err(AppError::cancelled(None))));
    let executor = LocalEngineExecutor::from_shared(engine.clone());
    let task = execution_task();
    let context = ExecutionContext {
        cancellation: CancellationToken::new(),
        progress: ProgressReporter::new(|_| {}),
    };

    assert_eq!(
        executor.execute(&task, &context),
        ExecutionOutcome::Cancelled
    );
    let request = engine
        .request
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .expect("recorded engine request");
    assert_eq!(request.attempt, 2);
    assert_eq!(request.job_id, task.job.id);
    assert_eq!(request.item_id, task.item.id);
}

#[test]
fn backend_service_reports_its_configured_concurrency_capability() {
    let service = BackendService::new(6).expect("create backend service");

    assert_eq!(service.capabilities().concurrency.maximum, 6);
    assert_eq!(service.manager().snapshot().scheduler.max_concurrency, 6);
}

#[test]
fn backend_service_wraps_normalization_errors_with_command_correlation() {
    let service = BackendService::new(2).expect("create backend service");
    let correlation_id = CorrelationId::new("request-1");
    let mut invalid = request(Vec::new(), same_directory_output("-copy"));
    invalid.schema_version = IPC_SCHEMA_VERSION + 1;

    let envelope = service
        .create_job(CreateJobCommand {
            correlation_id: correlation_id.clone(),
            request: invalid,
        })
        .expect_err("invalid schema");

    assert_eq!(envelope.correlation_id, correlation_id);
    assert_eq!(envelope.schema_version, IPC_SCHEMA_VERSION);
    assert_eq!(envelope.error.code.0, "protocol.schema_version_unsupported");
}

#[test]
fn backend_service_processes_a_real_mozjpeg_job_end_to_end() {
    let directory = TestDirectory::new("vertical");
    let input = directory.path("input.ppm");
    fs::write(
        &input,
        b"P6\n2 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\xff",
    )
    .expect("write PPM fixture");
    let service = BackendService::new(1).expect("create backend service");
    let response = service
        .create_job(CreateJobCommand {
            correlation_id: CorrelationId::new("vertical-request"),
            request: request(
                vec![InputResource {
                    path: input.to_string_lossy().into_owned(),
                    kind: InputResourceKind::File,
                    scan_recursively: false,
                }],
                same_directory_output("-optimized"),
            ),
        })
        .expect("create job");

    let terminal = service
        .manager()
        .wait_for_job_terminal(&response.job.id, Duration::from_secs(5))
        .expect("job completes");
    assert_eq!(terminal.status, crate::domain::JobStatus::Succeeded);
    let jpeg = fs::read(directory.path("input-optimized.jpg")).expect("read JPEG output");
    assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
}

#[test]
fn output_planning_never_overwrites_another_input_in_the_job() {
    let directory = TestDirectory::new("cross-input-output");
    let first = directory.path("photo.png");
    let second = directory.path("photo-copy.jpg");
    fs::write(&first, b"fixture").expect("write first input");
    fs::write(&second, b"fixture").expect("write second input");
    let request = request(
        vec![
            InputResource {
                path: first.to_string_lossy().into_owned(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            },
            InputResource {
                path: second.to_string_lossy().into_owned(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            },
        ],
        same_directory_output("-copy"),
    );

    let error = RequestNormalizer::default()
        .normalize(request)
        .expect_err("output must not replace another input");

    assert_eq!(error.code.0, "output.overwrites_other_input");
}

#[test]
fn output_root_parent_segments_are_normalized_before_collision_checks() {
    let directory = TestDirectory::new("output-root-parent-segments");
    let input_root = directory.path("input");
    fs::create_dir_all(&input_root).expect("create input directory");
    let input = input_root.join("photo.png");
    fs::write(&input, b"fixture").expect("write input");

    // The missing component makes the raw spelling non-existent, but the
    // normalized destination is still the input directory.
    let output_root = input_root.join("future").join("..");
    let mut request = request(
        vec![InputResource {
            path: input.to_string_lossy().into_owned(),
            kind: InputResourceKind::File,
            scan_recursively: false,
        }],
        OutputPolicy {
            location: OutputLocation::Directory(output_root.to_string_lossy().into_owned()),
            preserve_structure: false,
            suffix: String::new(),
            collision: CollisionPolicy::Fail,
            source_backup: BackupPolicy::Disabled,
            existing_output_backup: BackupPolicy::Disabled,
        },
    );
    request.encoder = EncoderConfig::Png;

    let error = RequestNormalizer::default()
        .normalize(request)
        .expect_err("lexically equivalent output must be treated as in-place");

    assert_eq!(error.code.0, "output.in_place_requires_replace");
}

#[test]
fn in_place_replace_derives_a_separate_source_backup() {
    let directory = TestDirectory::new("source-backup");
    let input = directory.path("photo.jpg");
    fs::write(&input, b"fixture").expect("write input");
    let request = request(
        vec![InputResource {
            path: input.to_string_lossy().into_owned(),
            kind: InputResourceKind::File,
            scan_recursively: false,
        }],
        OutputPolicy {
            location: OutputLocation::SameDirectory,
            preserve_structure: false,
            suffix: String::new(),
            collision: CollisionPolicy::Replace,
            source_backup: BackupPolicy::Enabled,
            existing_output_backup: BackupPolicy::Disabled,
        },
    );

    let normalized = RequestNormalizer::default()
        .normalize(request)
        .expect("normalize in-place request");
    let plan = &normalized.submission.items[0].output;
    assert_eq!(
        plan.output_path,
        fs::canonicalize(&input).expect("canonical in-place input")
    );
    assert_eq!(
        plan.source_backup_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("photo.neo-rimage-source-backup.jpg")
    );
    assert!(plan.existing_output_backup_path.is_none());
}

#[test]
fn auto_rename_avoids_an_existing_output_deterministically() {
    let directory = TestDirectory::new("auto-rename");
    let input = directory.path("photo.png");
    fs::write(&input, b"fixture").expect("write input");
    fs::write(directory.path("photo.jpg"), b"existing").expect("write existing output");
    let request = request(
        vec![InputResource {
            path: input.to_string_lossy().into_owned(),
            kind: InputResourceKind::File,
            scan_recursively: false,
        }],
        OutputPolicy {
            collision: CollisionPolicy::AutoRename,
            ..same_directory_output("")
        },
    );

    let normalized = RequestNormalizer::default()
        .normalize(request)
        .expect("normalize auto-rename request");

    assert_eq!(
        normalized.submission.items[0]
            .output
            .output_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("photo (1).jpg")
    );
}

fn scan_command(paths: Vec<PathBuf>, recursive: bool) -> crate::domain::ScanInputsCommand {
    crate::domain::ScanInputsCommand {
        schema_version: IPC_SCHEMA_VERSION,
        correlation_id: CorrelationId::new("scan-request"),
        paths: paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        scan_recursively: recursive,
    }
}

#[test]
fn input_preview_reuses_discovery_order_deduplication_and_decoder_whitelist() {
    let directory = TestDirectory::new("scan-inputs");
    fs::create_dir_all(directory.path("nested")).unwrap();
    fs::write(directory.path("photo.PNG"), b"fixture").unwrap();
    fs::write(directory.path("nested/画像.ppm"), b"fixture").unwrap();
    fs::write(directory.path("unsupported.avif"), b"fixture").unwrap();
    let service = BackendService::new(1).unwrap();
    let initial_revision = service.manager().observation_revision().0;
    let response = service
        .scan_inputs(scan_command(
            vec![
                directory.path("photo.PNG"),
                directory.path("./photo.PNG"),
                directory.path(""),
                directory.path("unsupported.avif"),
                directory.path("missing.png"),
            ],
            true,
        ))
        .unwrap();
    assert_eq!(response.inputs.len(), 2);
    assert_eq!(response.inputs[0].file_name, "photo.PNG");
    #[cfg(windows)]
    assert_eq!(
        response.inputs[0].path,
        directory.path("photo.PNG").to_string_lossy()
    );
    assert_eq!(response.inputs[1].file_name, "画像.ppm");
    assert_eq!(response.rejected_inputs.len(), 2);
    assert_eq!(
        response.rejected_inputs[0].error.code.0,
        "input.extension_unsupported"
    );
    assert_eq!(response.rejected_inputs[1].error.code.0, "input.not_found");
    assert_eq!(response.correlation_id, CorrelationId::new("scan-request"));
    assert_eq!(service.manager().observation_revision().0, initial_revision);
    assert!(service.manager().snapshot().jobs.is_empty());
    assert!(!response.inputs[0].path.starts_with(r"\?\"));
}

#[test]
fn input_preview_honors_recursion_and_reports_empty_directories() {
    let directory = TestDirectory::new("scan-recursion");
    fs::create_dir_all(directory.path("nested")).unwrap();
    fs::write(directory.path("nested/photo.ppm"), b"fixture").unwrap();
    let normalizer = RequestNormalizer::default();
    let shallow = normalizer
        .scan_inputs(scan_command(vec![directory.path("")], false))
        .unwrap();
    assert!(shallow.inputs.is_empty());
    assert_eq!(
        shallow.rejected_inputs[0].error.code.0,
        "input.directory_no_supported_files"
    );
    let recursive = normalizer
        .scan_inputs(scan_command(vec![directory.path("")], true))
        .unwrap();
    assert_eq!(recursive.inputs.len(), 1);
    assert!(recursive.rejected_inputs.is_empty());
}

#[test]
fn input_preview_enforces_the_same_file_and_depth_limits() {
    let directory = TestDirectory::new("scan-limits");
    fs::create_dir_all(directory.path("nested")).unwrap();
    fs::write(directory.path("first.ppm"), b"fixture").unwrap();
    fs::write(directory.path("second.ppm"), b"fixture").unwrap();
    fs::write(directory.path("nested/third.ppm"), b"fixture").unwrap();
    let limited = RequestNormalizer::new(NormalizationOptions {
        maximum_files: 1,
        maximum_depth: 64,
    });
    let error = limited
        .scan_inputs(scan_command(vec![directory.path("")], true))
        .unwrap_err();
    assert_eq!(error.code.0, "input.file_limit_exceeded");
    let shallow = RequestNormalizer::new(NormalizationOptions {
        maximum_files: 2,
        maximum_depth: 1,
    });
    assert_eq!(
        shallow
            .scan_inputs(scan_command(vec![directory.path("")], true))
            .unwrap()
            .inputs
            .len(),
        2
    );
}

#[test]
fn input_preview_checks_the_schema_before_filesystem_access() {
    let mut command = scan_command(vec![PathBuf::from("missing.png")], false);
    command.schema_version = 0;
    let service = BackendService::new(1).unwrap();
    let error = service.scan_inputs(command).unwrap_err();
    assert_eq!(error.error.code.0, "protocol.schema_version_unsupported");
    assert_eq!(error.correlation_id, CorrelationId::new("scan-request"));
}

#[test]
fn job_creation_revalidates_inputs_after_a_successful_preview() {
    let directory = TestDirectory::new("scan-revalidation");
    let input = directory.path("photo.ppm");
    fs::write(&input, b"fixture").unwrap();
    let normalizer = RequestNormalizer::default();
    let preview = normalizer
        .scan_inputs(scan_command(vec![input.clone()], false))
        .unwrap();
    fs::remove_file(&input).unwrap();
    let error = normalizer
        .normalize(request(
            vec![InputResource {
                path: preview.inputs[0].path.clone(),
                kind: InputResourceKind::File,
                scan_recursively: false,
            }],
            same_directory_output("-optimized"),
        ))
        .unwrap_err();
    assert_eq!(error.code.0, "input.discovery_rejected");
}

struct FakeEngine {
    outcome: Mutex<Option<EngineOutcome>>,
    request: Mutex<Option<EngineRequest>>,
}

impl FakeEngine {
    fn new(outcome: EngineOutcome) -> Self {
        Self {
            outcome: Mutex::new(Some(outcome)),
            request: Mutex::new(None),
        }
    }
}

impl Engine for FakeEngine {
    fn execute(&self, request: &EngineRequest, _context: &EngineContext<'_>) -> EngineOutcome {
        *self
            .request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(request.clone());
        self.outcome
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .expect("one fake outcome")
    }
}

fn request(inputs: Vec<InputResource>, output: OutputPolicy) -> crate::domain::CreateJobRequest {
    crate::domain::CreateJobRequest {
        schema_version: IPC_SCHEMA_VERSION,
        inputs,
        operations: Vec::new(),
        encoder: EncoderConfig::MozJpeg(MozJpegConfig::default()),
        output,
        metadata: MetadataPolicy {
            embedded: EmbeddedMetadataPolicy::PreserveWhenSupported,
            color_profile: ColorProfilePolicy::PreserveWhenSupported,
            report: ProcessingReportPolicy::Disabled,
        },
        input_acceptance: InputAcceptancePolicy::RejectAll,
        scheduling: None,
    }
}

fn same_directory_output(suffix: &str) -> OutputPolicy {
    OutputPolicy {
        location: OutputLocation::SameDirectory,
        preserve_structure: false,
        suffix: suffix.to_owned(),
        collision: CollisionPolicy::Fail,
        source_backup: BackupPolicy::Disabled,
        existing_output_backup: BackupPolicy::Disabled,
    }
}

fn execution_task() -> ExecutionTask {
    let job_id = JobId::new("job");
    let item_id = ItemId::new("item");
    ExecutionTask {
        job: std::sync::Arc::new(JobSpec {
            id: job_id.clone(),
            created_at: TimestampMs(1),
            config_version: JOB_CONFIG_VERSION,
            operations: Vec::new(),
            encoder: EncoderConfig::MozJpeg(MozJpegConfig::default()),
            output: same_directory_output("-copy"),
            metadata: MetadataPolicy::default(),
            scheduling: None,
        }),
        item: std::sync::Arc::new(ItemSpec {
            id: item_id,
            job_id,
            sequence: 0,
            attempt: 1,
            input_path: PathBuf::from("input.png"),
            scan_root: None,
            output: OutputPlan {
                output_path: PathBuf::from("output.jpg"),
                collision: CollisionPolicy::Fail,
                source_backup_path: None,
                existing_output_backup_path: None,
            },
        }),
        attempt: 2,
    }
}
