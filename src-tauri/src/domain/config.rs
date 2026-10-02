use super::{ItemId, JobId, TimestampMs, IPC_SCHEMA_VERSION, JOB_CONFIG_VERSION};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobRequest {
    pub schema_version: u16,
    pub inputs: Vec<InputResource>,
    pub operations: Vec<Operation>,
    pub encoder: EncoderConfig,
    pub output: OutputPolicy,
    pub metadata: MetadataPolicy,
    pub input_acceptance: InputAcceptancePolicy,
    pub scheduling: Option<SchedulingHint>,
}

impl CreateJobRequest {
    pub fn new(inputs: Vec<InputResource>, encoder: EncoderConfig, output: OutputPolicy) -> Self {
        Self {
            schema_version: IPC_SCHEMA_VERSION,
            inputs,
            operations: Vec::new(),
            encoder,
            output,
            metadata: MetadataPolicy::default(),
            input_acceptance: InputAcceptancePolicy::RejectAll,
            scheduling: None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputResource {
    pub path: String,
    pub kind: InputResourceKind,
    #[serde(default)]
    pub scan_recursively: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputResourceKind {
    File,
    Directory,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputAcceptancePolicy {
    #[default]
    RejectAll,
    AcceptValid,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case")]
pub enum Operation {
    Resize(ResizeOperation),
    Quantize(QuantizeOperation),
    Dither(DitherOperation),
    PremultiplyAlpha,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeOperation {
    pub mode: ResizeMode,
    pub filter: ResizeFilter,
    #[serde(default)]
    pub allow_upscale: bool,
    #[serde(default = "default_true")]
    pub allow_downscale: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ResizeMode {
    Exact { width: u32, height: u32 },
    FitWidth { width: u32 },
    FitHeight { height: u32 },
    Percentage { percent: f32 },
    Scale { factor: f32 },
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeFilter {
    Nearest,
    Bilinear,
    Hamming,
    CatmullRom,
    Mitchell,
    Lanczos3,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuantizeOperation {
    pub quality: f32,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DitherOperation {
    pub strength: Option<f32>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "options")]
pub enum EncoderConfig {
    #[serde(rename = "mozjpeg")]
    MozJpeg(MozJpegConfig),
    #[serde(rename = "jpeg")]
    Jpeg(JpegConfig),
    #[serde(rename = "avif")]
    Avif(AvifConfig),
    #[serde(rename = "oxipng")]
    OxiPng(OxiPngConfig),
    #[serde(rename = "webp")]
    WebP(WebPConfig),
    #[serde(rename = "jpeg_xl")]
    JpegXl,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "farbfeld")]
    Farbfeld,
    #[serde(rename = "ppm")]
    Ppm,
    #[serde(rename = "qoi")]
    Qoi,
}

impl EncoderConfig {
    pub fn kind(&self) -> EncoderKind {
        match self {
            Self::MozJpeg(_) => EncoderKind::MozJpeg,
            Self::Jpeg(_) => EncoderKind::Jpeg,
            Self::Avif(_) => EncoderKind::Avif,
            Self::OxiPng(_) => EncoderKind::OxiPng,
            Self::WebP(_) => EncoderKind::WebP,
            Self::JpegXl => EncoderKind::JpegXl,
            Self::Png => EncoderKind::Png,
            Self::Farbfeld => EncoderKind::Farbfeld,
            Self::Ppm => EncoderKind::Ppm,
            Self::Qoi => EncoderKind::Qoi,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum EncoderKind {
    #[serde(rename = "mozjpeg")]
    MozJpeg,
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(rename = "avif")]
    Avif,
    #[serde(rename = "oxipng")]
    OxiPng,
    #[serde(rename = "webp")]
    WebP,
    #[serde(rename = "jpeg_xl")]
    JpegXl,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "farbfeld")]
    Farbfeld,
    #[serde(rename = "ppm")]
    Ppm,
    #[serde(rename = "qoi")]
    Qoi,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MozJpegConfig {
    pub quality: f32,
    pub chroma_quality: Option<f32>,
    pub progressive: bool,
    pub optimize_coding: bool,
    pub smoothing: u8,
    pub color_space: MozJpegColorSpace,
    pub trellis_multipass: bool,
    pub chroma_subsample: Option<u8>,
    pub quantization_table: Option<MozJpegQuantizationTable>,
}

impl Default for MozJpegConfig {
    fn default() -> Self {
        Self {
            quality: 75.0,
            chroma_quality: None,
            progressive: true,
            optimize_coding: true,
            smoothing: 0,
            color_space: MozJpegColorSpace::YCbCr,
            trellis_multipass: false,
            chroma_subsample: None,
            quantization_table: None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MozJpegColorSpace {
    #[serde(rename = "ycbcr")]
    YCbCr,
    #[serde(rename = "rgb")]
    Rgb,
    #[serde(rename = "grayscale")]
    Grayscale,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MozJpegQuantizationTable {
    #[serde(rename = "ahumada_watson_peterson")]
    AhumadaWatsonPeterson,
    #[serde(rename = "annex_k")]
    AnnexK,
    #[serde(rename = "flat")]
    Flat,
    #[serde(rename = "klein_silverstein_carney")]
    KleinSilversteinCarney,
    #[serde(rename = "msssim")]
    Msssim,
    #[serde(rename = "n_robidoux")]
    NRobidoux,
    #[serde(rename = "psnr_hvs")]
    PsnrHvs,
    #[serde(rename = "peterson_ahumada_watson")]
    PetersonAhumadaWatson,
    #[serde(rename = "watson_taylor_borthwick")]
    WatsonTaylorBorthwick,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpegConfig {
    pub quality: f32,
    pub progressive: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvifConfig {
    pub quality: f32,
    pub alpha_quality: Option<f32>,
    pub speed: u8,
    pub color_space: AvifColorSpace,
    pub alpha_mode: AvifAlphaMode,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AvifColorSpace {
    #[serde(rename = "ycbcr")]
    YCbCr,
    #[serde(rename = "rgb")]
    Rgb,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvifAlphaMode {
    UnassociatedDirty,
    UnassociatedClean,
    Premultiplied,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OxiPngConfig {
    pub interlace: bool,
    pub effort: u8,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebPConfig {
    pub lossless: bool,
    pub quality: f32,
    pub slight_loss: u8,
    pub exact: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPolicy {
    pub location: OutputLocation,
    #[serde(default)]
    pub preserve_structure: bool,
    #[serde(default)]
    pub suffix: String,
    pub collision: CollisionPolicy,
    pub source_backup: BackupPolicy,
    pub existing_output_backup: BackupPolicy,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum OutputLocation {
    SameDirectory,
    Directory(String),
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    Fail,
    Replace,
    AutoRename,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupPolicy {
    #[default]
    Disabled,
    Enabled,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPolicy {
    pub embedded: EmbeddedMetadataPolicy,
    pub color_profile: ColorProfilePolicy,
    pub report: ProcessingReportPolicy,
}

impl Default for MetadataPolicy {
    fn default() -> Self {
        Self {
            embedded: EmbeddedMetadataPolicy::PreserveWhenSupported,
            color_profile: ColorProfilePolicy::PreserveWhenSupported,
            report: ProcessingReportPolicy::Disabled,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedMetadataPolicy {
    PreserveWhenSupported,
    Strip,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorProfilePolicy {
    PreserveWhenSupported,
    ConvertToSrgb,
    StripAfterConversion,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum ProcessingReportPolicy {
    Disabled,
    Json(String),
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulingHint {
    pub requested_concurrency: Option<u16>,
}

/// Immutable, normalized configuration stored by JobManager after acceptance.
#[derive(Clone, Debug, PartialEq)]
pub struct JobSpec {
    pub id: JobId,
    pub created_at: TimestampMs,
    pub config_version: u16,
    pub operations: Vec<Operation>,
    pub encoder: EncoderConfig,
    pub output: OutputPolicy,
    pub metadata: MetadataPolicy,
    pub scheduling: Option<SchedulingHint>,
}

impl JobSpec {
    pub fn with_defaults(id: JobId, created_at: TimestampMs, request: CreateJobRequest) -> Self {
        Self {
            id,
            created_at,
            config_version: JOB_CONFIG_VERSION,
            operations: request.operations,
            encoder: request.encoder,
            output: request.output,
            metadata: request.metadata,
            scheduling: request.scheduling,
        }
    }
}

/// A single normalized input and its reserved output plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemSpec {
    pub id: ItemId,
    pub job_id: JobId,
    pub sequence: u32,
    pub attempt: u32,
    pub input_path: PathBuf,
    pub scan_root: Option<PathBuf>,
    pub output: OutputPlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputPlan {
    pub output_path: PathBuf,
    pub collision: CollisionPolicy,
    pub source_backup_path: Option<PathBuf>,
    pub existing_output_backup_path: Option<PathBuf>,
}

/// Tauri-independent request consumed by the local engine.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineRequest {
    pub job_id: JobId,
    pub item_id: ItemId,
    pub attempt: u32,
    pub config_version: u16,
    pub input_path: PathBuf,
    pub output: OutputPlan,
    pub encoder: EncoderConfig,
    pub operations: Vec<Operation>,
    pub metadata: MetadataPolicy,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{
        AvifColorSpace, EncoderConfig, EncoderKind, MozJpegColorSpace, MozJpegConfig,
        MozJpegQuantizationTable, OutputLocation,
    };
    use serde_json::json;

    #[test]
    fn encoder_uses_stable_string_tag() {
        let value = serde_json::to_value(EncoderConfig::MozJpeg(MozJpegConfig::default()))
            .expect("serialize encoder");
        assert_eq!(value["kind"], json!("mozjpeg"));
        assert_eq!(value["options"]["colorSpace"], json!("ycbcr"));
    }

    #[test]
    fn output_location_is_explicitly_tagged() {
        let value = serde_json::to_value(OutputLocation::Directory("D:/out".into()))
            .expect("serialize output location");
        assert_eq!(value, json!({"kind": "directory", "path": "D:/out"}));
    }

    #[test]
    fn acronym_bearing_values_have_product_tags() {
        assert_eq!(
            serde_json::to_value(EncoderKind::MozJpeg).unwrap(),
            json!("mozjpeg")
        );
        assert_eq!(
            serde_json::to_value(EncoderKind::OxiPng).unwrap(),
            json!("oxipng")
        );
        assert_eq!(
            serde_json::to_value(EncoderKind::WebP).unwrap(),
            json!("webp")
        );
        assert_eq!(
            serde_json::to_value(EncoderKind::JpegXl).unwrap(),
            json!("jpeg_xl")
        );
        assert_eq!(
            serde_json::to_value(MozJpegColorSpace::YCbCr).unwrap(),
            json!("ycbcr")
        );
        assert_eq!(
            serde_json::to_value(AvifColorSpace::YCbCr).unwrap(),
            json!("ycbcr")
        );
        assert_eq!(
            serde_json::to_value(MozJpegQuantizationTable::PsnrHvs).unwrap(),
            json!("psnr_hvs")
        );
    }
}
