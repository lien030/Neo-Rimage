use crate::domain::{
    AvifAlphaMode, AvifColorSpace, AvifConfig, ColorProfilePolicy, EncoderConfig,
    ImageFormat as DomainImageFormat, ImageProperties, JpegConfig, MozJpegColorSpace,
    MozJpegConfig, MozJpegQuantizationTable, OxiPngConfig, ResizeFilter, ResizeMode,
    ResizeOperation, WebPConfig,
};
use jpeg_encoder::{ColorType as JpegColorType, Encoder as JpegEncoder};
use mozjpeg::{qtable, ColorSpace as MozColorSpace};
use rimage::{
    codecs::{
        mozjpeg::{MozJpegEncoder, MozJpegOptions},
        oxipng::{OxiPngEncoder, OxiPngOptions},
        webp::{WebPDecoder, WebPEncoder, WebPOptions},
    },
    operations::{
        icc::ApplySRGB,
        quantize::Quantize,
        resize::{FilterType, Resize, ResizeAlg},
    },
};
use std::{fs::File, io, io::Write, path::Path};
use zune_core::{bit_depth::BitDepth, colorspace::ColorSpace};
use zune_image::{
    codecs::{
        farbfeld::FarbFeldEncoder, jpeg_xl::JxlEncoder, png::PngEncoder, ppm::PPMEncoder,
        qoi::QoiEncoder, ImageFormat as ZuneImageFormat,
    },
    errors::{ImageErrors, ImgEncodeErrors},
    image::Image,
    traits::{DecoderTrait, EncoderTrait, OperationsTrait},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResizeEffect {
    Applied,
    NoChange,
    SkippedUpscale,
    SkippedDownscale,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NormalizeOutcome {
    pub had_color_profile: bool,
    pub converted_to_srgb: bool,
}

pub(crate) fn decode(path: &Path) -> Result<Image, ImageErrors> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("webp") => WebPDecoder::try_new(File::open(path)?)?.decode(),
        _ => Image::open(path),
    }
}

pub(crate) fn normalize_color_profile(
    image: &mut Image,
    policy: ColorProfilePolicy,
) -> Result<NormalizeOutcome, ImageErrors> {
    let had_color_profile = image.metadata().icc_chunk().is_some();
    match policy {
        ColorProfilePolicy::PreserveWhenSupported => Ok(NormalizeOutcome {
            had_color_profile,
            converted_to_srgb: false,
        }),
        ColorProfilePolicy::ConvertToSrgb => {
            if had_color_profile {
                ApplySRGB.execute(image)?;
            }
            Ok(NormalizeOutcome {
                had_color_profile,
                converted_to_srgb: had_color_profile,
            })
        }
        ColorProfilePolicy::StripAfterConversion => unreachable!(
            "strip-after-conversion is rejected during preflight until zune exposes safe ICC removal"
        ),
    }
}

pub(crate) fn apply_resize(
    image: &mut Image,
    config: &ResizeOperation,
) -> Result<ResizeEffect, ImageErrors> {
    let (source_width, source_height) = image.dimensions();
    let (target_width, target_height) =
        resize_dimensions(source_width, source_height, &config.mode).map_err(|message| {
            ImageErrors::GenericString(format!("invalid resize dimensions: {message}"))
        })?;

    if (source_width, source_height) == (target_width, target_height) {
        return Ok(ResizeEffect::NoChange);
    }

    let requires_upscale = target_width > source_width || target_height > source_height;
    let requires_downscale = target_width < source_width || target_height < source_height;
    if requires_upscale && !config.allow_upscale {
        return Ok(ResizeEffect::SkippedUpscale);
    }
    if requires_downscale && !config.allow_downscale {
        return Ok(ResizeEffect::SkippedDownscale);
    }

    let algorithm = match config.filter {
        ResizeFilter::Nearest => ResizeAlg::Nearest,
        ResizeFilter::Bilinear => ResizeAlg::Convolution(FilterType::Bilinear),
        ResizeFilter::Hamming => ResizeAlg::Convolution(FilterType::Hamming),
        ResizeFilter::CatmullRom => ResizeAlg::Convolution(FilterType::CatmullRom),
        ResizeFilter::Mitchell => ResizeAlg::Convolution(FilterType::Mitchell),
        ResizeFilter::Lanczos3 => ResizeAlg::Convolution(FilterType::Lanczos3),
    };

    Resize::new(target_width, target_height, algorithm).execute(image)?;
    Ok(ResizeEffect::Applied)
}

pub(crate) fn apply_quantize(
    image: &mut Image,
    quality: f32,
    dithering: Option<f32>,
) -> Result<(), ImageErrors> {
    if image.depth() != BitDepth::Eight {
        image.convert_depth(BitDepth::Eight)?;
    }
    if image.colorspace() != ColorSpace::RGBA {
        image.convert_color(ColorSpace::RGBA)?;
    }

    Quantize::new(quality.round() as u8, dithering).execute(image)
}

fn encode_mozjpeg<W: Write>(
    image: &Image,
    config: &MozJpegConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
    let mut prepared = image.clone();
    if prepared.depth() != BitDepth::Eight {
        prepared.convert_depth(BitDepth::Eight)?;
    }
    let input_color_space = match config.color_space {
        MozJpegColorSpace::Grayscale => ColorSpace::Luma,
        MozJpegColorSpace::YCbCr | MozJpegColorSpace::Rgb => ColorSpace::RGB,
    };
    if prepared.colorspace() != input_color_space {
        prepared.convert_color(input_color_space)?;
    }

    let (luma_qtable, chroma_qtable) = quantization_tables(config);
    let options = MozJpegOptions {
        quality: config.quality,
        progressive: config.progressive,
        optimize_coding: config.optimize_coding,
        smoothing: config.smoothing,
        color_space: match config.color_space {
            MozJpegColorSpace::YCbCr => MozColorSpace::JCS_YCbCr,
            MozJpegColorSpace::Rgb => MozColorSpace::JCS_EXT_RGB,
            MozJpegColorSpace::Grayscale => MozColorSpace::JCS_GRAYSCALE,
        },
        trellis_multipass: config.trellis_multipass,
        chroma_subsample: config.chroma_subsample,
        luma_qtable,
        chroma_qtable,
    };

    MozJpegEncoder::new_with_options(options).encode(&prepared, writer)
}

pub(crate) fn encode<W: Write>(
    image: &Image,
    config: &EncoderConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
    match config {
        EncoderConfig::MozJpeg(config) => encode_mozjpeg(image, config, writer),
        EncoderConfig::Jpeg(config) => encode_jpeg(image, config, writer),
        EncoderConfig::Avif(config) => encode_avif(image, config, writer),
        EncoderConfig::OxiPng(config) => encode_oxipng(image, config, writer),
        EncoderConfig::WebP(config) => encode_webp(image, config, writer),
        EncoderConfig::JpegXl => JxlEncoder::new().encode(image, writer),
        EncoderConfig::Png => PngEncoder::new().encode(image, writer),
        EncoderConfig::Farbfeld => FarbFeldEncoder::new().encode(image, writer),
        EncoderConfig::Ppm => PPMEncoder::new().encode(image, writer),
        EncoderConfig::Qoi => QoiEncoder::new().encode(image, writer),
    }
}

fn encode_jpeg<W: Write>(
    image: &Image,
    config: &JpegConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
    // zune-core 0.5.1 accidentally maps its progressive setter to Huffman
    // optimization. Use zune-image's underlying jpeg-encoder directly so the
    // strongly typed `progressive` option is effective rather than a no-op.
    let mut prepared = image.clone();
    if prepared.depth() != BitDepth::Eight {
        prepared.convert_depth(BitDepth::Eight)?;
    }
    let target_color_space = match prepared.colorspace() {
        ColorSpace::Luma => ColorSpace::Luma,
        ColorSpace::RGB => ColorSpace::RGB,
        ColorSpace::RGBA => ColorSpace::RGBA,
        _ if prepared.colorspace().has_alpha() => ColorSpace::RGBA,
        _ => ColorSpace::RGB,
    };
    if prepared.colorspace() != target_color_space {
        prepared.convert_color(target_color_space)?;
    }

    let (width, height) = prepared.dimensions();
    let width = u16::try_from(width).map_err(|_| {
        ImageErrors::EncodeErrors(ImgEncodeErrors::ImageEncodeErrors(
            "JPEG width exceeds 65535 pixels".to_owned(),
        ))
    })?;
    let height = u16::try_from(height).map_err(|_| {
        ImageErrors::EncodeErrors(ImgEncodeErrors::ImageEncodeErrors(
            "JPEG height exceeds 65535 pixels".to_owned(),
        ))
    })?;
    let color_type = match target_color_space {
        ColorSpace::Luma => JpegColorType::Luma,
        ColorSpace::RGBA => JpegColorType::Rgba,
        _ => JpegColorType::Rgb,
    };
    let pixels = &prepared.flatten_frames::<u8>()[0];
    let mut writer = CountingWriter::new(writer);
    let mut encoder = JpegEncoder::new(&mut writer, config.quality.round() as u8);
    encoder.set_progressive(config.progressive);
    encoder
        .encode(pixels, width, height, color_type)
        .map_err(|error| {
            ImageErrors::EncodeErrors(ImgEncodeErrors::ImageEncodeErrors(error.to_string()))
        })?;
    Ok(writer.bytes_written)
}

fn encode_avif<W: Write>(
    image: &Image,
    config: &AvifConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
    let prepared = rgb_family_image(image)?;
    let (width, height) = prepared.dimensions();
    let pixels = &prepared.flatten_to_u8()[0];
    let encoder = ravif::Encoder::new()
        .with_quality(config.quality)
        .with_alpha_quality(config.alpha_quality.unwrap_or(config.quality))
        .with_speed(config.speed)
        .with_internal_color_model(match config.color_space {
            AvifColorSpace::YCbCr => ravif::ColorModel::YCbCr,
            AvifColorSpace::Rgb => ravif::ColorModel::RGB,
        })
        .with_alpha_color_mode(match config.alpha_mode {
            AvifAlphaMode::UnassociatedDirty => ravif::AlphaColorMode::UnassociatedDirty,
            AvifAlphaMode::UnassociatedClean => ravif::AlphaColorMode::UnassociatedClean,
            AvifAlphaMode::Premultiplied => ravif::AlphaColorMode::Premultiplied,
        });
    let encoded = match prepared.colorspace() {
        ColorSpace::RGBA => {
            let pixels = pixels
                .chunks_exact(4)
                .map(|pixel| ravif::RGBA8::new(pixel[0], pixel[1], pixel[2], pixel[3]))
                .collect::<Vec<_>>();
            encoder.encode_rgba(ravif::Img::new(&pixels, width, height))
        }
        ColorSpace::RGB => {
            let pixels = pixels
                .chunks_exact(3)
                .map(|pixel| ravif::RGB8::new(pixel[0], pixel[1], pixel[2]))
                .collect::<Vec<_>>();
            encoder.encode_rgb(ravif::Img::new(&pixels, width, height))
        }
        _ => unreachable!("rgb_family_image always returns RGB or RGBA"),
    }
    .map_err(|error| {
        ImageErrors::EncodeErrors(ImgEncodeErrors::ImageEncodeErrors(error.to_string()))
    })?;
    let length = encoded.avif_file.len();
    let mut writer = writer;
    writer.write_all(&encoded.avif_file).map_err(|error| {
        ImageErrors::EncodeErrors(ImgEncodeErrors::ImageEncodeErrors(error.to_string()))
    })?;
    Ok(length)
}

fn encode_oxipng<W: Write>(
    image: &Image,
    config: &OxiPngConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
    let mut options = OxiPngOptions::from_preset(config.effort);
    options.interlace = Some(config.interlace);
    OxiPngEncoder::new_with_options(options).encode(image, writer)
}

fn encode_webp<W: Write>(
    image: &Image,
    config: &WebPConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
    let prepared = rgb_family_image(image)?;
    let mut options = WebPOptions::new().map_err(|()| {
        ImageErrors::EncodeErrors(ImgEncodeErrors::ImageEncodeErrors(
            "libwebp could not initialize an encoder configuration".to_owned(),
        ))
    })?;
    options.lossless = i32::from(config.lossless);
    options.quality = config.quality;
    options.near_lossless = 100 - i32::from(config.slight_loss);
    options.exact = i32::from(config.exact);
    WebPEncoder::new_with_options(options).encode(prepared.as_ref(), writer)
}

fn rgb_family_image(image: &Image) -> Result<std::borrow::Cow<'_, Image>, ImageErrors> {
    if matches!(image.colorspace(), ColorSpace::RGB | ColorSpace::RGBA) {
        return Ok(std::borrow::Cow::Borrowed(image));
    }

    let mut converted = image.clone();
    converted.convert_color(if image.colorspace().has_alpha() {
        ColorSpace::RGBA
    } else {
        ColorSpace::RGB
    })?;
    Ok(std::borrow::Cow::Owned(converted))
}

struct CountingWriter<W> {
    inner: W,
    bytes_written: usize,
}

impl<W> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            bytes_written: 0,
        }
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.bytes_written = self.bytes_written.saturating_add(written);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn image_properties(image: &Image, path: &Path) -> Result<ImageProperties, io::Error> {
    let (width, height) = image.dimensions();
    Ok(ImageProperties {
        format: detected_format(image, path),
        width: u32::try_from(width)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "image width exceeds u32"))?,
        height: u32::try_from(height)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "image height exceeds u32"))?,
        bit_depth: bit_depth(image.depth()),
        color_space: Some(color_space_name(image.colorspace()).to_owned()),
        has_alpha: Some(image.colorspace().has_alpha()),
        frame_count: u32::try_from(image.frames_len()).ok(),
    })
}

pub(crate) fn output_properties(
    image: &Image,
    config: &EncoderConfig,
) -> Result<ImageProperties, io::Error> {
    let (width, height) = image.dimensions();
    let (color_space, has_alpha) = output_color_space(image, config);
    Ok(ImageProperties {
        format: encoder_format(config),
        width: u32::try_from(width)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "image width exceeds u32"))?,
        height: u32::try_from(height)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "image height exceeds u32"))?,
        bit_depth: output_bit_depth(image, config),
        color_space: color_space.map(str::to_owned),
        has_alpha,
        frame_count: Some(if matches!(config, EncoderConfig::WebP(_)) {
            u32::try_from(image.frames_len()).unwrap_or(u32::MAX)
        } else {
            1
        }),
    })
}

pub(crate) fn encoder_output_extension(config: &EncoderConfig) -> &'static str {
    match config {
        EncoderConfig::MozJpeg(_) | EncoderConfig::Jpeg(_) => "jpg",
        EncoderConfig::Avif(_) => "avif",
        EncoderConfig::OxiPng(_) | EncoderConfig::Png => "png",
        EncoderConfig::WebP(_) => "webp",
        EncoderConfig::JpegXl => "jxl",
        EncoderConfig::Farbfeld => "ff",
        EncoderConfig::Ppm => "ppm",
        EncoderConfig::Qoi => "qoi",
    }
}

pub(crate) fn encoder_output_extensions(config: &EncoderConfig) -> &'static [&'static str] {
    match config {
        EncoderConfig::MozJpeg(_) | EncoderConfig::Jpeg(_) => &["jpg", "jpeg"],
        EncoderConfig::Avif(_) => &["avif"],
        EncoderConfig::OxiPng(_) | EncoderConfig::Png => &["png"],
        EncoderConfig::WebP(_) => &["webp"],
        EncoderConfig::JpegXl => &["jxl"],
        EncoderConfig::Farbfeld => &["ff", "farbfeld"],
        EncoderConfig::Ppm => &["ppm", "pnm"],
        EncoderConfig::Qoi => &["qoi"],
    }
}

fn encoder_format(config: &EncoderConfig) -> DomainImageFormat {
    match config {
        EncoderConfig::MozJpeg(_) | EncoderConfig::Jpeg(_) => DomainImageFormat::Jpeg,
        EncoderConfig::Avif(_) => DomainImageFormat::Avif,
        EncoderConfig::OxiPng(_) | EncoderConfig::Png => DomainImageFormat::Png,
        EncoderConfig::WebP(_) => DomainImageFormat::WebP,
        EncoderConfig::JpegXl => DomainImageFormat::JpegXl,
        EncoderConfig::Farbfeld => DomainImageFormat::Farbfeld,
        EncoderConfig::Ppm => DomainImageFormat::Ppm,
        EncoderConfig::Qoi => DomainImageFormat::Qoi,
    }
}

fn output_bit_depth(image: &Image, config: &EncoderConfig) -> Option<u8> {
    match config {
        EncoderConfig::MozJpeg(_)
        | EncoderConfig::Jpeg(_)
        | EncoderConfig::WebP(_)
        | EncoderConfig::Qoi => Some(8),
        // ravif's `Auto` mode emits ten-bit AV1 even from the adapter's RGBA8
        // input. Keep the reported property aligned with the coded stream.
        EncoderConfig::Avif(_) => Some(10),
        EncoderConfig::Farbfeld => Some(16),
        // OxiPNG may reduce bit depth while optimizing, so the exact coded
        // depth cannot be promised from the pre-encode image alone.
        EncoderConfig::OxiPng(_) => None,
        EncoderConfig::JpegXl | EncoderConfig::Png | EncoderConfig::Ppm => match image.depth() {
            BitDepth::Sixteen | BitDepth::Float32 => Some(16),
            _ => Some(8),
        },
    }
}

fn output_color_space(
    image: &Image,
    config: &EncoderConfig,
) -> (Option<&'static str>, Option<bool>) {
    match config {
        EncoderConfig::MozJpeg(config) => match config.color_space {
            MozJpegColorSpace::YCbCr => (Some("ycbcr"), Some(false)),
            MozJpegColorSpace::Rgb => (Some("rgb"), Some(false)),
            MozJpegColorSpace::Grayscale => (Some("grayscale"), Some(false)),
        },
        EncoderConfig::Jpeg(_) => match image.colorspace() {
            ColorSpace::Luma => (Some("luma"), Some(false)),
            _ => (Some("rgb"), Some(false)),
        },
        EncoderConfig::Avif(config) => (
            Some(match config.color_space {
                AvifColorSpace::YCbCr => "ycbcr",
                AvifColorSpace::Rgb => "rgb",
            }),
            Some(image.colorspace().has_alpha()),
        ),
        EncoderConfig::WebP(_) | EncoderConfig::Qoi => {
            if image.colorspace().has_alpha() {
                (Some("rgba"), Some(true))
            } else {
                (Some("rgb"), Some(false))
            }
        }
        EncoderConfig::Farbfeld => (Some("rgba"), Some(true)),
        // OxiPNG may losslessly reduce RGB to grayscale and/or remove an
        // unnecessary alpha channel during optimization.
        EncoderConfig::OxiPng(_) => (None, None),
        EncoderConfig::JpegXl | EncoderConfig::Png | EncoderConfig::Ppm => {
            match image.colorspace() {
                ColorSpace::Luma => (Some("luma"), Some(false)),
                ColorSpace::LumaA => (Some("luma_alpha"), Some(true)),
                ColorSpace::RGBA => (Some("rgba"), Some(true)),
                _ => (Some("rgb"), Some(false)),
            }
        }
    }
}

pub(crate) fn preserves_icc_profile(config: &EncoderConfig) -> bool {
    matches!(config, EncoderConfig::MozJpeg(_) | EncoderConfig::OxiPng(_))
}

pub(crate) fn extension_format(path: &Path) -> DomainImageFormat {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return DomainImageFormat::Unknown;
    };
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => DomainImageFormat::Jpeg,
        "png" => DomainImageFormat::Png,
        "webp" => DomainImageFormat::WebP,
        "jxl" => DomainImageFormat::JpegXl,
        "ff" | "farbfeld" => DomainImageFormat::Farbfeld,
        "ppm" | "pnm" => DomainImageFormat::Ppm,
        "qoi" => DomainImageFormat::Qoi,
        "avif" => DomainImageFormat::Avif,
        "tif" | "tiff" => DomainImageFormat::Tiff,
        _ => DomainImageFormat::Unknown,
    }
}

fn detected_format(image: &Image, path: &Path) -> DomainImageFormat {
    match image.metadata().image_format() {
        Some(ZuneImageFormat::JPEG) => DomainImageFormat::Jpeg,
        Some(ZuneImageFormat::PNG) => DomainImageFormat::Png,
        Some(ZuneImageFormat::WEBP) => DomainImageFormat::WebP,
        Some(ZuneImageFormat::JPEG_XL) => DomainImageFormat::JpegXl,
        Some(ZuneImageFormat::Farbfeld) => DomainImageFormat::Farbfeld,
        Some(ZuneImageFormat::PPM) => DomainImageFormat::Ppm,
        Some(ZuneImageFormat::QOI) => DomainImageFormat::Qoi,
        Some(_) | None => extension_format(path),
    }
}

fn bit_depth(depth: BitDepth) -> Option<u8> {
    match depth {
        BitDepth::Eight => Some(8),
        BitDepth::Sixteen => Some(16),
        BitDepth::Float32 => Some(32),
        BitDepth::Unknown => None,
        _ => None,
    }
}

fn color_space_name(color_space: ColorSpace) -> &'static str {
    match color_space {
        ColorSpace::RGB => "rgb",
        ColorSpace::RGBA => "rgba",
        ColorSpace::YCbCr => "ycbcr",
        ColorSpace::Luma => "luma",
        ColorSpace::LumaA => "luma_alpha",
        ColorSpace::YCCK => "ycck",
        ColorSpace::CMYK => "cmyk",
        ColorSpace::BGR => "bgr",
        ColorSpace::BGRA => "bgra",
        ColorSpace::ARGB => "argb",
        ColorSpace::HSL => "hsl",
        ColorSpace::HSV => "hsv",
        ColorSpace::Unknown => "unknown",
        ColorSpace::MultiBand(_) => "multi_band",
        _ => "unknown",
    }
}

fn resize_dimensions(
    source_width: usize,
    source_height: usize,
    mode: &ResizeMode,
) -> Result<(usize, usize), &'static str> {
    if source_width == 0 || source_height == 0 {
        return Err("source dimensions are zero");
    }

    let (width, height) = match *mode {
        ResizeMode::Exact { width, height } => (f64::from(width), f64::from(height)),
        ResizeMode::FitWidth { width } => {
            let scale = f64::from(width) / source_width as f64;
            (f64::from(width), source_height as f64 * scale)
        }
        ResizeMode::FitHeight { height } => {
            let scale = f64::from(height) / source_height as f64;
            (source_width as f64 * scale, f64::from(height))
        }
        ResizeMode::Percentage { percent } => {
            let scale = f64::from(percent) / 100.0;
            (source_width as f64 * scale, source_height as f64 * scale)
        }
        ResizeMode::Scale { factor } => {
            let scale = f64::from(factor);
            (source_width as f64 * scale, source_height as f64 * scale)
        }
    };

    dimension_from_float(width)
        .and_then(|width| dimension_from_float(height).map(|height| (width, height)))
}

fn dimension_from_float(value: f64) -> Result<usize, &'static str> {
    if !value.is_finite() || value <= 0.0 || value > usize::MAX as f64 {
        return Err("calculated dimension is outside the supported range");
    }
    Ok(value.round().max(1.0) as usize)
}

fn quantization_tables(config: &MozJpegConfig) -> (Option<qtable::QTable>, Option<qtable::QTable>) {
    let Some(table) = config.quantization_table else {
        return (None, None);
    };
    let luma_quality = config.quality;
    let chroma_quality = config.chroma_quality.unwrap_or(config.quality);

    let scale = |table: &qtable::QTable, quality: f32| table.scaled(quality, quality);
    match table {
        MozJpegQuantizationTable::AhumadaWatsonPeterson => (
            Some(scale(&qtable::AhumadaWatsonPeterson, luma_quality)),
            Some(scale(&qtable::AhumadaWatsonPeterson, chroma_quality)),
        ),
        MozJpegQuantizationTable::AnnexK => (
            Some(scale(&qtable::AnnexK_Luma, luma_quality)),
            Some(scale(&qtable::AnnexK_Chroma, chroma_quality)),
        ),
        MozJpegQuantizationTable::Flat => (
            Some(scale(&qtable::Flat, luma_quality)),
            Some(scale(&qtable::Flat, chroma_quality)),
        ),
        MozJpegQuantizationTable::KleinSilversteinCarney => (
            Some(scale(&qtable::KleinSilversteinCarney, luma_quality)),
            Some(scale(&qtable::KleinSilversteinCarney, chroma_quality)),
        ),
        MozJpegQuantizationTable::Msssim => (
            Some(scale(&qtable::MSSSIM_Luma, luma_quality)),
            Some(scale(&qtable::MSSSIM_Chroma, chroma_quality)),
        ),
        MozJpegQuantizationTable::NRobidoux => (
            Some(scale(&qtable::NRobidoux, luma_quality)),
            Some(scale(&qtable::NRobidoux, chroma_quality)),
        ),
        MozJpegQuantizationTable::PsnrHvs => (
            Some(scale(&qtable::PSNRHVS_Luma, luma_quality)),
            Some(scale(&qtable::PSNRHVS_Chroma, chroma_quality)),
        ),
        MozJpegQuantizationTable::PetersonAhumadaWatson => (
            Some(scale(&qtable::PetersonAhumadaWatson, luma_quality)),
            Some(scale(&qtable::PetersonAhumadaWatson, chroma_quality)),
        ),
        MozJpegQuantizationTable::WatsonTaylorBorthwick => (
            Some(scale(&qtable::WatsonTaylorBorthwick, luma_quality)),
            Some(scale(&qtable::WatsonTaylorBorthwick, chroma_quality)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_modes_preserve_aspect_ratio() {
        assert_eq!(
            resize_dimensions(400, 200, &ResizeMode::FitWidth { width: 100 }).unwrap(),
            (100, 50)
        );
        assert_eq!(
            resize_dimensions(400, 200, &ResizeMode::FitHeight { height: 100 }).unwrap(),
            (200, 100)
        );
        assert_eq!(
            resize_dimensions(400, 200, &ResizeMode::Percentage { percent: 25.0 }).unwrap(),
            (100, 50)
        );
    }

    #[test]
    fn tiny_scaled_dimensions_are_clamped_to_one_pixel() {
        assert_eq!(
            resize_dimensions(2, 2, &ResizeMode::Scale { factor: 0.1 }).unwrap(),
            (1, 1)
        );
    }
}
