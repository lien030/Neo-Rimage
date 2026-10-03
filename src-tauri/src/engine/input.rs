use super::{pipeline, svg};
use crate::domain::{
    AppError, EngineRequest, ErrorCategory, ImageFormat, ImageProperties, Operation,
};
use rimage::{
    codecs::{
        avif::{self as avif, AvifDecoder, AvifHeader},
        svg::{SvgDecoder, SvgOptions},
        tiff::TiffDecoder,
        webp::WebPDecoder,
    },
    limits::{ImageFormatId, PipelineCost},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
use zune_core::{
    bit_depth::BitDepth, bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions,
};
use zune_image::{codecs::ImageFormat as ZuneFormat, image::Image, traits::DecoderTrait};

const MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;
const PARSER_BASE: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
enum InputKind {
    Avif(AvifHeader),
    Tiff,
    Svg,
    WebP,
    Zune(ZuneFormat),
}

#[derive(Clone, Debug)]
pub struct PreparedInput {
    pub memory_bytes: u64,
    pub properties: ImageProperties,
    pub input_bytes: u64,
    pub consumed_resize: bool,
    kind: InputKind,
    digest: [u8; 32],
    render_size: (u32, u32),
    resources: Vec<(PathBuf, [u8; 32])>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum PrepareError {
    NeedsBudget(u64),
    Failed(AppError),
}

pub(super) fn error(code: &str, key: &str, detail: impl ToString) -> PrepareError {
    PrepareError::Failed(
        AppError::new(code, ErrorCategory::Input, key, detail.to_string())
            .with_retryable(code == "input.changed"),
    )
}

fn invalid(detail: impl ToString) -> PrepareError {
    error("input.decode_failed", "errors.decodeFailed", detail)
}

fn require(required: u64, budget: u64) -> Result<(), PrepareError> {
    if required > budget {
        Err(PrepareError::NeedsBudget(required))
    } else {
        Ok(())
    }
}

fn read(path: &Path, budget: u64) -> Result<Vec<u8>, PrepareError> {
    let metadata = fs::metadata(path).map_err(|detail| {
        error(
            "input.metadata_unavailable",
            "errors.inputUnavailable",
            detail,
        )
    })?;
    if !metadata.is_file() {
        return Err(error(
            "input.not_regular_file",
            "errors.inputNotRegularFile",
            "Input is not a regular file",
        ));
    }
    if metadata.len() > MAX_INPUT_BYTES {
        return Err(error(
            "input.byte_limit",
            "errors.inputByteLimit",
            "Input exceeds 256 MiB",
        ));
    }
    require(
        metadata.len().saturating_mul(6).saturating_add(PARSER_BASE),
        budget,
    )?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(invalid)?
        .take(metadata.len() + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(error(
            "input.changed",
            "errors.inputChanged",
            "Input changed during inspection",
        ));
    }
    Ok(bytes)
}

pub fn prepare(request: &EngineRequest, budget: u64) -> Result<PreparedInput, PrepareError> {
    let bytes = read(&request.input_path, budget)?;
    inspect(request, &bytes, budget)
}

fn inspect(
    request: &EngineRequest,
    bytes: &[u8],
    budget: u64,
) -> Result<PreparedInput, PrepareError> {
    let mut resources = Vec::new();
    let mut preparation_bytes = bytes.len() as u64 * 6 + PARSER_BASE;
    let mut resource_pixels = 0;
    let (kind, format, width, height, bits, color) = if avif::is_avif(bytes) {
        let header = avif::probe(Cursor::new(bytes))
            .map_err(|detail| error("input.avif_unsupported", "errors.avifUnsupported", detail))?;
        if !header.still_picture {
            return Err(error(
                "input.animation_unsupported",
                "errors.animationUnsupported",
                "Animated AVIF is unsupported",
            ));
        }
        if matches!(header.transfer, 16 | 18)
            || header
                .container_transfers
                .iter()
                .any(|transfer| matches!(transfer, 16 | 18))
        {
            return Err(error(
                "input.avif_hdr",
                "errors.avifHdrUnsupported",
                "PQ/HLG HDR is unsupported",
            ));
        }
        let properties = (header.width, header.height, header.bit_depth);
        (
            InputKind::Avif(header),
            ImageFormat::Avif,
            properties.0,
            properties.1,
            properties.2,
            ColorSpace::RGBA,
        )
    } else if bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
        || bytes.starts_with(b"II+\0")
        || bytes.starts_with(b"MM\0+")
    {
        let mut decoder =
            TiffDecoder::try_new_with_limits(Cursor::new(bytes), budget as usize, 100_000_000)
                .map_err(invalid)?;
        let ((width, height), color, bits, multipage) = decoder.probe().map_err(invalid)?;
        if multipage {
            return Err(error(
                "input.tiff_multipage",
                "errors.tiffMultipageUnsupported",
                "Multi-page TIFF is unsupported",
            ));
        }
        if u64::from(width) * u64::from(height) > 100_000_000 {
            return Err(error(
                "input.pixel_limit",
                "errors.inputPixelLimit",
                "TIFF exceeds 100 MP",
            ));
        }
        (
            InputKind::Tiff,
            ImageFormat::Tiff,
            width,
            height,
            bits,
            color,
        )
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        reject_chunk_animation(bytes, true)?;
        let size = imagesize::blob_size(bytes).map_err(invalid)?;
        (
            InputKind::WebP,
            ImageFormat::WebP,
            size.width as u32,
            size.height as u32,
            8,
            ColorSpace::RGBA,
        )
    } else if let Some((format, _)) = ZuneFormat::guess_format(ZCursor::new(bytes)) {
        if format == ZuneFormat::PNG {
            reject_chunk_animation(bytes, false)?;
        }
        if format == ZuneFormat::JPEG_XL {
            let header = zune_image::codecs::jpeg_xl::jxl_oxide::JxlImage::builder()
                .read(Cursor::new(bytes))
                .map_err(invalid)?;
            if header.image_header().metadata.animation.is_some() {
                return Err(error(
                    "input.animation_unsupported",
                    "errors.animationUnsupported",
                    "Animated JPEG XL is unsupported",
                ));
            }
        }
        let mut decoder = format
            .decoder_with_options(ZCursor::new(bytes), decoder_options())
            .map_err(invalid)?;
        let metadata = decoder
            .read_headers()
            .map_err(invalid)?
            .ok_or_else(|| invalid("Missing image header"))?;
        let bits = match metadata.depth() {
            BitDepth::Eight => 8,
            BitDepth::Sixteen => 16,
            BitDepth::Float32 => 32,
            _ => return Err(invalid("Unknown sample type")),
        };
        let domain_format = match format {
            ZuneFormat::JPEG => ImageFormat::Jpeg,
            ZuneFormat::PNG => ImageFormat::Png,
            ZuneFormat::JPEG_XL => ImageFormat::JpegXl,
            ZuneFormat::Farbfeld => ImageFormat::Farbfeld,
            ZuneFormat::QOI => ImageFormat::Qoi,
            ZuneFormat::PPM => ImageFormat::Ppm,
            _ => ImageFormat::Unknown,
        };
        let (width, height) = metadata.dimensions();
        (
            InputKind::Zune(format),
            domain_format,
            u32::try_from(width).map_err(invalid)?,
            u32::try_from(height).map_err(invalid)?,
            bits,
            decoder.out_colorspace(),
        )
    } else {
        let loaded = svg::load(bytes, &request.input_path, budget)?;
        let size = loaded.tree.size();
        resources = loaded.resources;
        resource_pixels = loaded.raster_bytes;
        preparation_bytes = preparation_bytes.max(loaded.parsing_bytes);
        (
            InputKind::Svg,
            ImageFormat::Svg,
            size.width().round().max(1.0) as u32,
            size.height().round().max(1.0) as u32,
            8,
            ColorSpace::RGBA,
        )
    };
    if width == 0 || height == 0 {
        return Err(invalid("Zero image dimensions"));
    }
    let mut dimensions = (width as usize, height as usize);
    let mut consumed_resize = false;
    if kind == InputKind::Svg {
        if let Some(Operation::Resize(config)) = request.operations.first() {
            let target = pipeline::resize_dimensions(dimensions.0, dimensions.1, &config.mode)
                .map_err(invalid)?;
            if permitted_resize(dimensions, target, config) {
                dimensions = target;
                consumed_resize = true;
            }
        }
    }
    let render_size = (
        u32::try_from(dimensions.0).map_err(invalid)?,
        u32::try_from(dimensions.1).map_err(invalid)?,
    );
    let layout = u64::from(bits.div_ceil(8).max(1)) * color.num_components() as u64;
    let mut maximum_pixels = (dimensions.0 as u64).saturating_mul(dimensions.1 as u64);
    if kind != InputKind::Svg {
        maximum_pixels = maximum_pixels.max(u64::from(width) * u64::from(height));
    }
    for operation in request.operations.iter().skip(usize::from(consumed_resize)) {
        if let Operation::Resize(config) = operation {
            let target = pipeline::resize_dimensions(dimensions.0, dimensions.1, &config.mode)
                .map_err(invalid)?;
            if permitted_resize(dimensions, target, config) {
                dimensions = target;
                maximum_pixels =
                    maximum_pixels.max((target.0 as u64).saturating_mul(target.1 as u64));
            }
        }
    }
    let output = match request.encoder.kind() {
        crate::domain::EncoderKind::Avif => ImageFormatId::Avif,
        crate::domain::EncoderKind::MozJpeg | crate::domain::EncoderKind::Jpeg => {
            ImageFormatId::Jpeg
        }
        crate::domain::EncoderKind::Png | crate::domain::EncoderKind::OxiPng => ImageFormatId::Png,
        crate::domain::EncoderKind::WebP => ImageFormatId::WebP,
        _ => ImageFormatId::Other,
    };
    let mut cost = PipelineCost::for_encoder(output);
    cost.resize = if request
        .operations
        .iter()
        .any(|operation| matches!(operation, Operation::Resize(_)))
    {
        4
    } else {
        0
    };
    cost.quantize = if request
        .operations
        .iter()
        .any(|operation| matches!(operation, Operation::Quantize(_)))
    {
        8
    } else {
        0
    };
    let memory_bytes = preparation_bytes
        .saturating_add(resource_pixels)
        .saturating_add(
            maximum_pixels
                .saturating_mul(layout.max(4))
                .saturating_mul(cost.total().saturating_add(4)),
        );
    Ok(PreparedInput {
        memory_bytes,
        properties: ImageProperties {
            format,
            width,
            height,
            bit_depth: Some(bits),
            color_space: Some(format!("{color:?}").to_lowercase()),
            has_alpha: Some(match &kind {
                InputKind::Avif(header) => header.has_alpha,
                _ => color.has_alpha(),
            }),
            frame_count: Some(1),
        },
        input_bytes: bytes.len() as u64,
        kind,
        digest: Sha256::digest(bytes).into(),
        render_size,
        resources,
        consumed_resize,
    })
}

fn permitted_resize(
    source: (usize, usize),
    target: (usize, usize),
    config: &crate::domain::ResizeOperation,
) -> bool {
    !((target.0 > source.0 || target.1 > source.1) && !config.allow_upscale
        || (target.0 < source.0 || target.1 < source.1) && !config.allow_downscale)
}

fn decoder_options() -> DecoderOptions {
    DecoderOptions::default()
        .set_max_width(usize::MAX)
        .set_max_height(usize::MAX)
}

pub fn decode(request: &EngineRequest, plan: &PreparedInput) -> Result<Image, PrepareError> {
    let bytes = read(&request.input_path, plan.memory_bytes).map_err(|failure| match failure {
        PrepareError::NeedsBudget(_) => error(
            "input.changed",
            "errors.inputChanged",
            "Input grew after preparation",
        ),
        failure => failure,
    })?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != plan.digest {
        return Err(error(
            "input.changed",
            "errors.inputChanged",
            "Input changed after preparation",
        ));
    }
    if plan.kind == InputKind::Svg {
        for (path, expected_digest) in &plan.resources {
            let checked = (|| -> std::io::Result<[u8; 32]> {
                let mut file = fs::File::open(path)?.take(64 * 1024 * 1024 + 1);
                let mut buffer = [0; 64 * 1024];
                let mut digest = Sha256::new();
                let mut length = 0_u64;
                loop {
                    let count = file.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    length += count as u64;
                    if length > 64 * 1024 * 1024 {
                        return Err(std::io::Error::other("SVG resource grew beyond its limit"));
                    }
                    digest.update(&buffer[..count]);
                }
                Ok(digest.finalize().into())
            })();
            if checked.ok() != Some(*expected_digest) {
                return Err(error(
                    "input.changed",
                    "errors.inputChanged",
                    "SVG resource changed or disappeared after preparation",
                ));
            }
        }
        let loaded = svg::load_for_execution(&bytes, &request.input_path, plan.memory_bytes)
            .map_err(|failure| match failure {
                PrepareError::NeedsBudget(_) => error(
                    "input.changed",
                    "errors.inputChanged",
                    "SVG resource budget changed after preparation",
                ),
                failure => failure,
            })?;
        let size = loaded.tree.size();
        if loaded.resources != plan.resources
            || size.width().round().max(1.0) as u32 != plan.properties.width
            || size.height().round().max(1.0) as u32 != plan.properties.height
        {
            return Err(error(
                "input.changed",
                "errors.inputChanged",
                "SVG properties or resources changed",
            ));
        }
        return SvgDecoder::from_tree(
            loaded.tree,
            SvgOptions {
                resources_dir: None,
                target_size: Some(plan.render_size),
                pixel_budget: Some(u64::from(plan.render_size.0) * u64::from(plan.render_size.1)),
            },
        )
        .map_err(invalid)?
        .decode()
        .map_err(invalid);
    }
    let inspected = inspect(request, &bytes, plan.memory_bytes)?;
    if inspected.properties != plan.properties
        || inspected.resources != plan.resources
        || inspected.memory_bytes > plan.memory_bytes
    {
        return Err(error(
            "input.changed",
            "errors.inputChanged",
            "Input properties or resources changed after preparation",
        ));
    }
    let pixels = u64::from(plan.properties.width) * u64::from(plan.properties.height);
    let image = match &plan.kind {
        InputKind::Avif(_) => AvifDecoder::try_new_with_limit(
            Cursor::new(&bytes),
            pixels.min(u64::from(u32::MAX)) as u32,
        )
        .map_err(invalid)?
        .decode(),
        InputKind::Tiff => TiffDecoder::try_new_with_limits(
            Cursor::new(&bytes),
            plan.memory_bytes as usize,
            pixels,
        )
        .map_err(invalid)?
        .decode(),
        InputKind::WebP => WebPDecoder::try_new(Cursor::new(&bytes))
            .map_err(invalid)?
            .decode(),
        InputKind::Zune(format) => format
            .decoder_with_options(ZCursor::new(bytes.as_slice()), decoder_options())
            .map_err(invalid)?
            .decode(),
        InputKind::Svg => unreachable!(),
    }
    .map_err(invalid)?;
    let expected = (plan.properties.width, plan.properties.height);
    if image.dimensions() != (expected.0 as usize, expected.1 as usize) || image.frames_len() != 1 {
        return Err(error(
            "input.changed",
            "errors.inputChanged",
            "Decoded dimensions or frame count do not match preparation",
        ));
    }
    Ok(image)
}

#[cfg(test)]
impl PreparedInput {
    pub(crate) fn test_plan() -> Self {
        Self {
            memory_bytes: 1,
            properties: ImageProperties {
                format: ImageFormat::Ppm,
                width: 1,
                height: 1,
                bit_depth: Some(8),
                color_space: Some("rgb".into()),
                has_alpha: Some(false),
                frame_count: Some(1),
            },
            input_bytes: 0,
            consumed_resize: false,
            kind: InputKind::Zune(ZuneFormat::PPM),
            digest: [0; 32],
            render_size: (1, 1),
            resources: Vec::new(),
        }
    }
}

pub(super) fn reject_chunk_animation(bytes: &[u8], webp: bool) -> Result<(), PrepareError> {
    let mut offset = if webp { 12 } else { 8 };
    while offset + 8 <= bytes.len() {
        let size_bytes: [u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
        let (tag, size) = if webp {
            (
                &bytes[offset..offset + 4],
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize,
            )
        } else {
            (
                &bytes[offset + 4..offset + 8],
                u32::from_be_bytes(size_bytes) as usize,
            )
        };
        if tag == b"acTL"
            || tag == b"ANIM"
            || tag == b"ANMF"
            || (webp && tag == b"VP8X" && bytes.get(offset + 8).is_some_and(|flags| flags & 2 != 0))
        {
            return Err(error(
                "input.animation_unsupported",
                "errors.animationUnsupported",
                "Animated input is unsupported",
            ));
        }
        offset = offset
            .checked_add(size)
            .and_then(|end| end.checked_add(if webp { 8 + size % 2 } else { 12 }))
            .ok_or_else(|| invalid("Invalid chunk size"))?;
    }
    Ok(())
}
