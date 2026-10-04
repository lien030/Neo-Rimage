/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec;
use alloc::vec::Vec;

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_core::options::DecoderOptions;

use crate::errors::FarbFeldErrors;

const FARBFELD_COLORSPACE: ColorSpace = ColorSpace::RGBA;
const FARBFELD_BIT_DEPTH: BitDepth = BitDepth::Sixteen;

/// A simple Farbfeld lossless decoder.
///
/// One can modify the decoder accepted dimensions
/// via `DecoderOptions`
pub struct FarbFeldDecoder<T: ZByteReaderTrait> {
    stream:          ZReader<T>,
    width:           usize,
    height:          usize,
    decoded_headers: bool,
    options:         DecoderOptions
}

impl<T> FarbFeldDecoder<T>
where
    T: ZByteReaderTrait
{
    ///Create a new decoder.
    ///
    /// Data is the raw compressed farbfeld data
    pub fn new(data: T) -> FarbFeldDecoder<T> {
        Self::new_with_options(data, DecoderOptions::default())
    }
    /// Create a new decoder with non default options as opposed to
    /// `new`
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(data: T, option: DecoderOptions) -> FarbFeldDecoder<T> {
        FarbFeldDecoder {
            stream:          ZReader::new(data),
            height:          0,
            width:           0,
            decoded_headers: false,
            options:         option
        }
    }
    /// Decode a header for this specific image
    pub fn decode_headers(&mut self) -> Result<(), FarbFeldErrors> {
        if self.decoded_headers {
            return Ok(());
        }
        // read magic

        let magic_value = self.stream.get_u64_be_err()?.to_be_bytes();

        if &magic_value != b"farbfeld" {
            return Err(FarbFeldErrors::Generic("Farbfeld magic bytes not found"));
        }
        // 32 bit BE width
        self.width = self.stream.get_u32_be_err()? as usize;
        // 32 BE height
        self.height = self.stream.get_u32_be_err()? as usize;

        trace!("Image width: {}", self.width);
        trace!("Image height: {}", self.height);

        if self.height > self.options.max_height() {
            return Err(FarbFeldErrors::Generic("Image Height is greater than max height. Bump up max_height to support such images"));
        }
        if self.width > self.options.max_width() {
            return Err(FarbFeldErrors::Generic("Image width is greater than max width. Bump up max_width in options to support such images"));
        }

        self.decoded_headers = true;
        Ok(())
    }

    /// Return the minimum buffer size for which the buffer provided must be in order
    /// to store decoded bytes into
    ///
    /// ## Returns
    /// -  Some(usize) - The size expected for a buffer of `&[u8]` which can
    ///  hold the whole decoded bytes without overflow
    /// - None: Indicates the headers weren't decoded or width*height*8 would overflow a usize
    pub fn output_buffer_size(&self) -> Option<usize> {
        if self.decoded_headers {
            Some(
                (FARBFELD_COLORSPACE.num_components()/*RGBA*/)
                    .checked_mul(self.width)?
                    .checked_mul(self.height)?
                    .checked_mul(2 /*depth*/)?
            )
        } else {
            None
        }
    }
    /// Decode data writing it into the buffer as native endian
    ///
    /// It is an error if the sink has fewer than
    /// [`output_buffer_size()`](Self::output_buffer_size) / `size_of::<u16>()` elements.
    ///
    /// # Arguments
    /// - `sink`: The output buffer which we will fill with native-endian samples
    ///
    /// # Endianness
    ///
    /// Since Farbfeld uses 16 bit big endian samples, each two bytes
    /// represent a single pixel.
    ///
    /// The endianness of these is converted to native endian which means
    /// each two consecutive bytes represents the two bytes that make the u16
    pub fn decode_into(&mut self, sink: &mut [u16]) -> Result<(), FarbFeldErrors> {
        if !self.decoded_headers {
            self.decode_headers()?;
        }
        let expected_len = self
            .output_buffer_size()
            .ok_or(FarbFeldErrors::Generic("Overflowed int"))?
            / core::mem::size_of::<u16>();

        if sink.len() < expected_len {
            return Err(FarbFeldErrors::Generic("Too small output buffer size"));
        }

        let sink = &mut sink[..expected_len];

        // farbfeld uses big endian, and we want output in native endian
        // so we read data as big endian and then convert it to native endian
        // This should be a no-op in BE systems, a bswap in LE systems
        for datum in sink.iter_mut() {
            let pix = self.stream.get_u16_be_err()?;
            *datum = pix;
        }

        Ok(())
    }
    /// Decode a farbfeld data returning raw pixels or an error
    ///
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZCursor;
    /// use zune_farbfeld::FarbFeldDecoder;
    /// let mut decoder = FarbFeldDecoder::new(ZCursor::new(b"NOT A VALID FILE"));
    ///
    /// assert!(decoder.decode().is_err());
    /// ```
    pub fn decode(&mut self) -> Result<Vec<u16>, FarbFeldErrors> {
        self.decode_headers()?;

        let size = self
            .output_buffer_size()
            .ok_or(FarbFeldErrors::Generic("Overflowed int"))?
            / core::mem::size_of::<u16>();

        // NOTE: This can be done via data.align() + decode_into()
        // but that's unsafe, and doesn't please the Rust gods
        let mut data = vec![0; size];

        self.decode_into(&mut data)?;
        Ok(data)
    }

    /// Returns farbfeld default image colorspace.
    ///
    /// This is always RGBA
    pub const fn colorspace(&self) -> ColorSpace {
        FARBFELD_COLORSPACE
    }
    /// Return farbfeld default bit depth
    ///
    /// This is always 16
    pub const fn bit_depth(&self) -> BitDepth {
        FARBFELD_BIT_DEPTH
    }

    /// Return the width and height of the image
    ///
    /// Or none if the headers haven't been decoded
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_farbfeld::FarbFeldDecoder;
    /// let mut decoder = FarbFeldDecoder::new(ZCursor::new([]));
    ///
    ///
    /// decoder.decode_headers().unwrap();
    /// // get dimensions now.
    /// let (w,h)=decoder.dimensions().unwrap();
    /// ```
    pub const fn dimensions(&self) -> Option<(usize, usize)> {
        if self.decoded_headers {
            return Some((self.width, self.height));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zune_core::bytestream::ZCursor;

    fn fixture(width: u32, height: u32, samples: &[u16]) -> Vec<u8> {
        let mut bytes = b"farbfeld".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend(samples.iter().flat_map(|sample| sample.to_be_bytes()));
        bytes
    }

    #[test]
    fn decodes_rgba16_after_repeated_header_reads() {
        let samples = [0x0102, 0xabcd, 0x8001, 0, 0xffff, 1, 0x1234, 0xffff];
        let bytes = fixture(2, 1, &samples);
        let mut decoder = FarbFeldDecoder::new(ZCursor::new(&bytes));
        decoder.decode_headers().unwrap();
        decoder.decode_headers().unwrap();
        assert_eq!(decoder.dimensions(), Some((2, 1)));
        assert_eq!(decoder.output_buffer_size(), Some(16));
        assert_eq!(decoder.decode().unwrap(), samples);
    }

    #[test]
    fn decode_into_uses_sample_counts_and_preserves_the_unused_tail() {
        let samples = [0x0102, 0xabcd, 0x8001, 0xffff];
        let bytes = fixture(1, 1, &samples);
        let mut decoder = FarbFeldDecoder::new(ZCursor::new(&bytes));
        assert!(decoder.decode_into(&mut [0; 3]).is_err());
        let mut exact = [0; 4];
        decoder.decode_into(&mut exact).unwrap();
        assert_eq!(exact, samples);

        let mut decoder = FarbFeldDecoder::new(ZCursor::new(&bytes));
        let mut oversized = [0xbeef; 8];
        decoder.decode_into(&mut oversized).unwrap();
        assert_eq!(oversized[..4], samples);
        assert_eq!(oversized[4..], [0xbeef; 4]);
    }

    #[test]
    fn rejects_truncated_headers_and_pixel_data() {
        let bytes = fixture(1, 1, &[0x0102, 0xabcd, 0x8001, 0xffff]);
        for length in 0..bytes.len() {
            let mut decoder = FarbFeldDecoder::new(ZCursor::new(&bytes[..length]));
            assert!(decoder.decode().is_err(), "accepted {length} bytes");
        }
    }

    #[test]
    fn rejects_size_overflow_before_allocating() {
        let bytes = fixture(u32::MAX, u32::MAX, &[]);
        let options = DecoderOptions::default()
            .set_max_width(usize::MAX)
            .set_max_height(usize::MAX);
        let mut decoder = FarbFeldDecoder::new_with_options(ZCursor::new(&bytes), options);
        decoder.decode_headers().unwrap();
        assert_eq!(decoder.output_buffer_size(), None);
        assert!(matches!(decoder.decode(), Err(FarbFeldErrors::Generic("Overflowed int"))));
    }
}
