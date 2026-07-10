use crate::domain::{
    ColorProfilePolicy, ImageFormat as DomainImageFormat, ImageProperties, MozJpegColorSpace,
    MozJpegConfig, MozJpegQuantizationTable, ResizeFilter, ResizeMode, ResizeOperation,
};
use mozjpeg::{qtable, ColorSpace as MozColorSpace};
use rimage::{
    codecs::mozjpeg::{MozJpegEncoder, MozJpegOptions},
    operations::{
        icc::ApplySRGB,
        quantize::Quantize,
        resize::{FilterType, Resize, ResizeAlg},
    },
};
use std::{io, path::Path};
use zune_core::{bit_depth::BitDepth, colorspace::ColorSpace};
use zune_image::{
    codecs::ImageFormat as ZuneImageFormat,
    errors::ImageErrors,
    image::Image,
    traits::{EncoderTrait, OperationsTrait},
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
    // zune-image owns the common decoder dispatch. rimage's custom AVIF/TIFF
    // adapters are intentionally not enabled in the Phase-1 feature set.
    Image::open(path)
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

pub(crate) fn encode_mozjpeg<W: zune_core::bytestream::ZByteWriterTrait>(
    image: &Image,
    config: &MozJpegConfig,
    writer: W,
) -> Result<usize, ImageErrors> {
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

    MozJpegEncoder::new_with_options(options).encode(image, writer)
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

pub(crate) fn mozjpeg_output_properties(
    image: &Image,
    config: &MozJpegConfig,
) -> Result<ImageProperties, io::Error> {
    let (width, height) = image.dimensions();
    Ok(ImageProperties {
        format: DomainImageFormat::Jpeg,
        width: u32::try_from(width)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "image width exceeds u32"))?,
        height: u32::try_from(height)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "image height exceeds u32"))?,
        bit_depth: Some(8),
        color_space: Some(
            match config.color_space {
                MozJpegColorSpace::YCbCr => "ycbcr",
                MozJpegColorSpace::Rgb => "rgb",
                MozJpegColorSpace::Grayscale => "grayscale",
            }
            .to_owned(),
        ),
        has_alpha: Some(false),
        frame_count: Some(1),
    })
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
