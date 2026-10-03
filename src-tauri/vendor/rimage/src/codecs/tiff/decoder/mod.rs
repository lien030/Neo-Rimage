use std::io::{Read, Seek};

use zune_core::colorspace::ColorSpace;
use zune_image::{errors::ImageErrors, image::Image, traits::DecoderTrait};

/// Upper bound on the pixel count of a TIFF this decoder will accept.
///
/// The CLI pre-checks dimensions via `limits`, but the library entry point is
/// reachable directly. A file that declares gigantic dimensions would make
/// `read_image` allocate them before the tiff crate's own checks run. 100 MP
/// is generous for photography and blocks the gigapixel OOM attack.
const MAX_TIFF_PIXELS: u64 = 100_000_000;

/// A Tiff decoder
pub struct TiffDecoder<R: Read + Seek> {
    inner: tiff::decoder::Decoder<R>,
    dimensions: Option<(usize, usize)>,
    colorspace: ColorSpace,
    pixel_limit: u64,
}

impl<R: Read + Seek> TiffDecoder<R> {
    /// Create a new tiff decoder that reads data from `source`
    pub fn try_new(source: R) -> Result<Self, ImageErrors> {
        Self::try_new_with_limits(source, 256 * 1024 * 1024, MAX_TIFF_PIXELS)
    }

    /// Construct a TIFF decoder with bounded buffers and pixels.
    pub fn try_new_with_limits(
        source: R,
        buffer_bytes: usize,
        pixel_limit: u64,
    ) -> Result<Self, ImageErrors> {
        let inner = tiff::decoder::Decoder::new(source).map_err(|e| {
            ImageErrors::ImageDecodeErrors(format!("Unable to create TIFF decoder: {e}"))
        })?;

        let mut limits = tiff::decoder::Limits::default();
        limits.decoding_buffer_size = buffer_bytes;
        limits.intermediate_buffer_size = buffer_bytes.min(128 * 1024 * 1024);
        limits.ifd_value_size = buffer_bytes.min(1024 * 1024);
        Ok(Self {
            inner: inner.with_limits(limits),
            dimensions: None,
            colorspace: ColorSpace::Unknown,
            pixel_limit: pixel_limit.min(MAX_TIFF_PIXELS),
        })
    }
}

impl<R: Read + Seek> TiffDecoder<R> {
    /// Read dimensions, layout, sample bits, and page information without pixels.
    pub fn probe(&mut self) -> Result<((u32, u32), ColorSpace, u8, bool), ImageErrors> {
        let dimensions = self
            .inner
            .dimensions()
            .map_err(|error| ImageErrors::ImageDecodeErrors(error.to_string()))?;
        let (color, bits) = match self
            .inner
            .colortype()
            .map_err(|error| ImageErrors::ImageDecodeErrors(error.to_string()))?
        {
            tiff::ColorType::RGB(bits) => (ColorSpace::RGB, bits),
            tiff::ColorType::RGBA(bits) => (ColorSpace::RGBA, bits),
            tiff::ColorType::Gray(bits) => (ColorSpace::Luma, bits),
            tiff::ColorType::GrayA(bits) => (ColorSpace::LumaA, bits),
            tiff::ColorType::CMYK(bits) => (ColorSpace::CMYK, bits),
            tiff::ColorType::YCbCr(bits) => (ColorSpace::YCbCr, bits),
            other => {
                return Err(ImageErrors::ImageDecodeErrors(format!(
                    "Unsupported TIFF color type: {other:?}"
                )));
            }
        };
        if color == ColorSpace::YCbCr {
            self.validate_ycbcr(bits)?;
        }
        Ok((dimensions, color, bits, self.inner.more_images()))
    }

    fn validate_ycbcr(&mut self, bits: u8) -> Result<(), ImageErrors> {
        if bits != 8 {
            return Err(ImageErrors::ImageDecodeErrors(
                "Only 8-bit YCbCr TIFF is supported".into(),
            ));
        }
        for (tag, expected) in [
            (529, &[0.299, 0.587, 0.114][..]),
            (532, &[0.0, 255.0, 128.0, 255.0, 128.0, 255.0][..]),
        ] {
            if let Some(value) = self
                .inner
                .find_tag(tiff::tags::Tag::Unknown(tag))
                .map_err(|error| ImageErrors::ImageDecodeErrors(error.to_string()))?
            {
                let tiff::decoder::ifd::Value::List(values) = value else {
                    return Err(ImageErrors::ImageDecodeErrors(
                        "Invalid YCbCr TIFF colour parameters".into(),
                    ));
                };
                let values: Vec<_> = values
                    .into_iter()
                    .map(|value| match value {
                        tiff::decoder::ifd::Value::Rational(numerator, denominator)
                            if denominator > 0 =>
                        {
                            numerator as f64 / denominator as f64
                        }
                        _ => f64::NAN,
                    })
                    .collect();
                if values.len() != expected.len()
                    || values.iter().zip(expected).any(|(value, expected)| {
                        !value.is_finite() || (value - expected).abs() > 0.000001
                    })
                {
                    return Err(ImageErrors::ImageDecodeErrors(
                        "Custom YCbCr TIFF coefficients or reference ranges are unsupported".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl<R> DecoderTrait for TiffDecoder<R>
where
    R: Read + Seek,
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let (width, height) = self.inner.dimensions().map_err(|e| {
            ImageErrors::ImageDecodeErrors(format!("Unable to read dimensions - {e}"))
        })?;

        let (width, height) = (width as usize, height as usize);

        // Reject an oversized image before `read_image` allocates it. The
        // tiff crate trusts the declared dimensions.
        let pixels = (width as u64).checked_mul(height as u64);
        match pixels {
            Some(p) if p <= self.pixel_limit => {}
            _ => {
                return Err(ImageErrors::ImageDecodeErrors(format!(
                    "TIFF dimensions {width}x{height} exceed the {MAX_TIFF_PIXELS} pixel limit"
                )));
            }
        }

        self.dimensions = Some((width, height));

        let colortype = self.inner.colortype().map_err(|e| {
            ImageErrors::ImageDecodeErrors(format!("Unable to read colorspace - {e}"))
        })?;
        let colorspace = match colortype {
            tiff::ColorType::RGB(_) => ColorSpace::RGB,
            tiff::ColorType::RGBA(_) => ColorSpace::RGBA,
            tiff::ColorType::CMYK(_) => ColorSpace::CMYK,
            tiff::ColorType::Gray(_) => ColorSpace::Luma,
            tiff::ColorType::GrayA(_) => ColorSpace::LumaA,
            tiff::ColorType::YCbCr(_) => ColorSpace::YCbCr,
            other => {
                return Err(ImageErrors::ImageDecodeErrors(format!(
                    "Unsupported TIFF color type: {other:?}"
                )));
            }
        };

        self.colorspace = colorspace;
        if let tiff::ColorType::YCbCr(bits) = colortype {
            self.validate_ycbcr(bits)?;
        }

        let result = self.inner.read_image().map_err(|e| {
            ImageErrors::ImageDecodeErrors(format!("Unable to decode TIFF file - {e}"))
        })?;

        match result {
            tiff::decoder::DecodingResult::U8(data) => {
                if let tiff::ColorType::Gray(bits @ (1 | 2 | 4)) = colortype {
                    let stride = (width * bits as usize).div_ceil(8);
                    if data.len() != stride * height {
                        return Err(ImageErrors::ImageDecodeErrors(
                            "Invalid packed TIFF samples".into(),
                        ));
                    }
                    let mask = (1_u8 << bits) - 1;
                    let mut samples = Vec::with_capacity(width * height);
                    for row in data.chunks_exact(stride) {
                        for column in 0..width {
                            let offset = column * bits as usize;
                            let sample =
                                (row[offset / 8] >> (8 - bits as usize - offset % 8)) & mask;
                            samples.push((u16::from(sample) * 255 / u16::from(mask)) as u8);
                        }
                    }
                    return Ok(Image::from_u8(&samples, width, height, colorspace));
                }
                if colorspace == ColorSpace::YCbCr {
                    if data.len() != width * height * 3 {
                        return Err(ImageErrors::ImageDecodeErrors(
                            "Invalid YCbCr TIFF samples".into(),
                        ));
                    }
                    let luma: Vec<_> = data.chunks_exact(3).map(|pixel| pixel[0]).collect();
                    let cb: Vec<_> = data.chunks_exact(3).map(|pixel| pixel[1]).collect();
                    let cr: Vec<_> = data.chunks_exact(3).map(|pixel| pixel[2]).collect();
                    let planes = yuvutils_rs::YuvPlanarImage {
                        y_plane: &luma,
                        y_stride: width as u32,
                        u_plane: &cb,
                        u_stride: width as u32,
                        v_plane: &cr,
                        v_stride: width as u32,
                        width: width as u32,
                        height: height as u32,
                    };
                    let mut rgb = vec![0; width * height * 3];
                    yuvutils_rs::yuv444_to_rgb(
                        &planes,
                        &mut rgb,
                        width as u32 * 3,
                        yuvutils_rs::YuvRange::Full,
                        yuvutils_rs::YuvStandardMatrix::Bt601,
                    )
                    .map_err(|error| ImageErrors::ImageDecodeErrors(error.to_string()))?;
                    self.colorspace = ColorSpace::RGB;
                    return Ok(Image::from_u8(&rgb, width, height, ColorSpace::RGB));
                }
                Ok(Image::from_u8(&data, width, height, colorspace))
            }
            tiff::decoder::DecodingResult::U16(data) => {
                Ok(Image::from_u16(&data, width, height, colorspace))
            }
            tiff::decoder::DecodingResult::F32(data) => {
                Ok(Image::from_f32(&data, width, height, colorspace))
            }
            _ => Err(ImageErrors::ImageDecodeErrors(
                "Tiff Data format not supported".to_string(),
            )),
        }
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace
    }

    fn name(&self) -> &'static str {
        "tiff-decoder"
    }
}

#[cfg(test)]
mod tests;
