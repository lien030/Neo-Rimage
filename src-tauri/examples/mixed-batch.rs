use neo_rimage_lib::{
    backend::{LocalEngineExecutor, RequestNormalizer},
    domain::*,
    jobs::*,
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct MeasuredExecutor {
    inner: LocalEngineExecutor,
    active: AtomicUsize,
    peak: AtomicUsize,
}

impl Executor for MeasuredExecutor {
    fn prepare(
        &self,
        task: &ExecutionTask,
        budget: u64,
        cancellation: &CancellationToken,
    ) -> PreparationOutcome {
        self.inner.prepare(task, budget, cancellation)
    }
    fn execute(
        &self,
        task: &ExecutionTask,
        plan: &PreparedExecution,
        context: &ExecutionContext,
    ) -> ExecutionOutcome {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        let outcome = self.inner.execute(task, plan, context);
        self.active.fetch_sub(1, Ordering::SeqCst);
        outcome
    }
}

fn main() {
    let concurrency = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "4".into())
        .parse::<usize>()
        .unwrap();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/mixed-batch");
    fs::create_dir_all(&directory).unwrap();
    let mut inputs = Vec::new();
    for index in 0..12 {
        let path = if index % 3 == 0 {
            let path = directory.join(format!("vector-{index}.svg"));
            fs::write(&path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024"><rect width="1024" height="1024" fill="red"/><circle cx="512" cy="512" r="200" fill="blue"/></svg>"#).unwrap();
            path
        } else {
            let side = if index % 3 == 1 { 1536 } else { 512 };
            let path = directory.join(format!("raster-{index}.ppm"));
            let mut bytes = format!("P6\n{side} {side}\n255\n").into_bytes();
            bytes.extend(
                (0..side * side)
                    .flat_map(|pixel| [(pixel % 255) as u8, ((pixel / side) % 255) as u8, 128]),
            );
            fs::write(&path, bytes).unwrap();
            path
        };
        inputs.push(InputResource {
            path: path.to_string_lossy().into_owned(),
            kind: InputResourceKind::File,
            scan_recursively: false,
        });
    }
    let job = RequestNormalizer::default()
        .normalize(CreateJobRequest {
            schema_version: IPC_SCHEMA_VERSION,
            inputs,
            encoder: EncoderConfig::Avif(AvifConfig {
                quality: 50.0,
                alpha_quality: None,
                speed: 8,
                color_space: AvifColorSpace::YCbCr,
                alpha_mode: AvifAlphaMode::UnassociatedClean,
            }),
            operations: Vec::new(),
            metadata: MetadataPolicy::default(),
            input_acceptance: InputAcceptancePolicy::RejectAll,
            output: OutputPolicy {
                location: OutputLocation::SameDirectory,
                suffix: format!("-workers-{concurrency}"),
                preserve_structure: false,
                collision: CollisionPolicy::Replace,
                source_backup: BackupPolicy::Disabled,
                existing_output_backup: BackupPolicy::Disabled,
            },
            scheduling: None,
        })
        .unwrap();
    let executor = Arc::new(MeasuredExecutor {
        inner: LocalEngineExecutor::default(),
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
    });
    let manager = JobManager::with_memory_budget(
        executor.clone(),
        Arc::new(SystemClock),
        concurrency,
        concurrency,
        512 * 1024 * 1024,
    )
    .unwrap();
    let started = Instant::now();
    let job_id = manager.submit(job.submission).unwrap();
    let snapshot = manager
        .wait_for_job_terminal(&job_id, Duration::from_secs(300))
        .unwrap();
    assert_eq!(snapshot.counts.succeeded, 12, "{snapshot:?}");
    println!("workers={concurrency} seconds={:.3} images_per_second={:.3} peak_executions={} budget_mib=512",
        started.elapsed().as_secs_f64(), 12.0 / started.elapsed().as_secs_f64(), executor.peak.load(Ordering::SeqCst));
    assert!(manager.shutdown(Duration::from_secs(5)).graceful);
}
