use super::{AppError, ItemId, JobId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStage {
    Preflight,
    Inspect,
    Decode,
    Normalize,
    Operations,
    Encode,
    Commit,
    MetadataFinalize,
    Complete,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressMeasure {
    Indeterminate,
    Fraction { completed: u64, total: u64 },
}

impl ProgressMeasure {
    pub fn ratio(&self) -> Option<f64> {
        match self {
            Self::Indeterminate => None,
            Self::Fraction { completed, total } if *total > 0 => {
                Some((*completed as f64 / *total as f64).clamp(0.0, 1.0))
            }
            Self::Fraction { .. } => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemProgress {
    pub stage: ProcessingStage,
    pub measure: ProgressMeasure,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineWarning {
    pub code: String,
    pub stage: Option<ProcessingStage>,
    pub message_key: String,
    #[serde(default)]
    pub message_args: BTreeMap<String, String>,
    pub fallback_message: String,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineProgressEvent {
    StageStarted {
        stage: ProcessingStage,
    },
    StageProgress {
        progress: ItemProgress,
    },
    WarningRaised {
        warning: EngineWarning,
    },
    StageCompleted {
        stage: ProcessingStage,
        duration_ms: u64,
    },
}

/// Non-blocking reporting boundary supplied by JobManager to the local engine.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, event: EngineProgressEvent);
}

/// Cooperative cancellation boundary. Codecs are checked only at safe points.
pub trait CancellationProbe: Send + Sync {
    fn is_cancel_requested(&self) -> bool;
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ImageFormat {
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "avif")]
    Avif,
    #[serde(rename = "webp")]
    WebP,
    #[serde(rename = "jpeg_xl")]
    JpegXl,
    #[serde(rename = "farbfeld")]
    Farbfeld,
    #[serde(rename = "ppm")]
    Ppm,
    #[serde(rename = "qoi")]
    Qoi,
    #[serde(rename = "tiff")]
    Tiff,
    #[serde(rename = "unknown")]
    Unknown,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageProperties {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub bit_depth: Option<u8>,
    pub color_space: Option<String>,
    pub has_alpha: Option<bool>,
    pub frame_count: Option<u32>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOutcome {
    pub embedded_metadata_preserved: bool,
    pub color_profile_preserved: bool,
    pub converted_to_srgb: bool,
    pub auto_oriented: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineResult {
    pub job_id: JobId,
    pub item_id: ItemId,
    pub attempt: u32,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub input: ImageProperties,
    pub output: ImageProperties,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub duration_ms: u64,
    pub metadata: MetadataOutcome,
    #[serde(default)]
    pub warnings: Vec<EngineWarning>,
    #[serde(default)]
    pub stage_durations_ms: BTreeMap<ProcessingStage, u64>,
}

impl EngineResult {
    pub fn bytes_saved(&self) -> i128 {
        self.input_bytes as i128 - self.output_bytes as i128
    }

    pub fn compression_ratio(&self) -> Option<f64> {
        (self.input_bytes > 0).then(|| self.output_bytes as f64 / self.input_bytes as f64)
    }
}

pub type EngineOutcome = Result<EngineResult, AppError>;

#[cfg(test)]
mod tests {
    use super::ProgressMeasure;

    #[test]
    fn progress_ratio_is_bounded_and_honest() {
        assert_eq!(ProgressMeasure::Indeterminate.ratio(), None);
        assert_eq!(
            ProgressMeasure::Fraction {
                completed: 5,
                total: 0
            }
            .ratio(),
            None
        );
        assert_eq!(
            ProgressMeasure::Fraction {
                completed: 15,
                total: 10
            }
            .ratio(),
            Some(1.0)
        );
    }
}
