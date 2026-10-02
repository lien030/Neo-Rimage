use super::{EncoderKind, IPC_SCHEMA_VERSION, JOB_CONFIG_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCapabilities {
    pub schema_version: u16,
    pub job_config_version: u16,
    pub backend_version: String,
    pub rimage_version: String,
    pub rimage_revision: Option<String>,
    pub encoders: Vec<EncoderCapability>,
    pub operations: Vec<OperationCapability>,
    pub metadata: MetadataCapability,
    pub concurrency: ConcurrencyCapability,
}

impl BackendCapabilities {
    pub fn empty(backend_version: impl Into<String>, rimage_version: impl Into<String>) -> Self {
        Self {
            schema_version: IPC_SCHEMA_VERSION,
            job_config_version: JOB_CONFIG_VERSION,
            backend_version: backend_version.into(),
            rimage_version: rimage_version.into(),
            rimage_revision: None,
            encoders: Vec::new(),
            operations: Vec::new(),
            metadata: MetadataCapability::default(),
            concurrency: ConcurrencyCapability {
                default: 1,
                minimum: 1,
                maximum: 1,
            },
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderCapability {
    pub kind: EncoderKind,
    pub available: bool,
    pub output_extensions: Vec<String>,
    pub options: Vec<OptionCapability>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Resize,
    Quantize,
    Dither,
    PremultiplyAlpha,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationCapability {
    pub kind: OperationKind,
    pub available: bool,
    pub options: Vec<OptionCapability>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionCapability {
    pub path: String,
    pub value_type: OptionValueType,
    pub required: bool,
    pub default_value: Option<Value>,
    pub constraints: OptionConstraints,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionValueType {
    Boolean,
    Integer,
    Number,
    String,
    Enum,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionConstraints {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub step: Option<f64>,
    #[serde(default)]
    pub allowed_values: Vec<Value>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataCapability {
    pub embedded_metadata: bool,
    pub icc_profiles: bool,
    pub auto_orient: bool,
    pub processing_report: bool,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcurrencyCapability {
    pub default: u16,
    pub minimum: u16,
    pub maximum: u16,
}
