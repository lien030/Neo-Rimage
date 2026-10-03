use std::io::Cursor;

use zune_core::colorspace::ColorSpace;
use zune_image::image::Image;

use super::{MAX_TARGET_PIXELS, SIZE_LIMIT_MARKER, SvgDecoder, SvgOptions, parse_size_limit};

#[test]
fn max_target_pixels_fits_the_decode_byte_budget() {
    let max_bytes = MAX_TARGET_PIXELS * 4 * 3;
    assert!(max_bytes <= 512 * 1024 * 1024);
}

#[test]
fn decode_simple_rect() {
    let file = fixture("rect.svg");

    let decoder = SvgDecoder::try_new(file).unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (100, 50));
    assert_eq!(img.colorspace(), ColorSpace::RGBA);

    let red = img.channels_ref(false)[0].reinterpret_as::<u8>().unwrap();
    assert_eq!(red[0], 255);

    let green = img.channels_ref(false)[1].reinterpret_as::<u8>().unwrap();
    assert_eq!(green[0], 0);

    let alpha = img.channels_ref(false)[3].reinterpret_as::<u8>().unwrap();
    assert_eq!(alpha[0], 255);
}

#[test]
fn probe_size_returns_intrinsic_size() {
    let file = fixture("rect.svg");

    let size = SvgDecoder::probe_size(file, None).unwrap();

    assert_eq!(size, (100.0, 50.0));
}

#[test]
fn decode_through_resize_callback() {
    let file = fixture("rect.svg");

    let decoder = SvgDecoder::try_new_with_resize(file, None, |size| {
        assert_eq!(size, (100, 50));
        Ok(Some((200, 100)))
    })
    .unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (200, 100));
}

#[test]
fn decode_with_target_size_renders_at_vector_quality() {
    let file = fixture("rect.svg");
    let options = SvgOptions {
        target_size: Some((200, 100)),
        ..Default::default()
    };

    let decoder = SvgDecoder::try_new_with_options(file, options).unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (200, 100));

    // The target-size render keeps full opacity and crisp vector edges, just
    // like the old --svg-scale path did.
    let red = img.channels_ref(false)[0].reinterpret_as::<u8>().unwrap();
    assert_eq!(red[0], 255);
}

#[test]
fn decode_with_target_aspect_ratio() {
    let file = fixture("rect.svg");
    let options = SvgOptions {
        target_size: Some((50, 25)),
        ..Default::default()
    };

    let decoder = SvgDecoder::try_new_with_options(file, options).unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (50, 25));
}

#[test]
fn decode_with_exact_target_size() {
    let file = fixture("rect.svg");
    let options = SvgOptions {
        target_size: Some((120, 40)),
        ..Default::default()
    };

    let decoder = SvgDecoder::try_new_with_options(file, options).unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (120, 40));
}

#[test]
fn decode_without_viewbox_uses_content_bbox() {
    let file = fixture("no-viewbox.svg");

    let decoder = SvgDecoder::try_new(file).unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    // The circle spans x 80..160 and y 40..120.
    assert_eq!(img.dimensions(), (160, 120));
}

#[test]
fn decode_text_renders_without_error() {
    let file = fixture("text-cjk.svg");

    let decoder = SvgDecoder::try_new(file).unwrap();
    let img = Image::from_decoder(decoder).unwrap();

    // the canvas comes from the SVG's width/height (200x100), the text
    // mixes simplified and traditional Han with kana and latin
    assert_eq!(img.dimensions(), (200, 100));
}

#[test]
fn decode_invalid_svg_errors() {
    let file = fixture("invalid.svg");

    // Parsing happens eagerly when creating the decoder.
    let decoder = SvgDecoder::try_new(file);

    assert!(decoder.is_err());
}

#[test]
fn decode_with_zero_target_size_errors() {
    let file = fixture("rect.svg");
    let options = SvgOptions {
        target_size: Some((0, 100)),
        ..Default::default()
    };

    let decoder = SvgDecoder::try_new_with_options(file, options);

    assert!(decoder.is_err());
}

#[test]
fn decode_with_huge_intrinsic_size_errors() {
    let file = fixture("huge-canvas.svg");

    // Parsing succeeds, the oversized render target is rejected instead of
    // attempting a multi-gigabyte allocation.
    let decoder = SvgDecoder::try_new(file);

    assert!(decoder.is_err());
}

#[test]
fn decode_with_oversized_target_size_errors() {
    let file = fixture("rect.svg");
    let options = SvgOptions {
        target_size: Some((u32::MAX, u32::MAX)),
        ..Default::default()
    };

    let decoder = SvgDecoder::try_new_with_options(file, options);

    assert!(decoder.is_err());
}

#[test]
fn decode_oversized_svg_carries_a_size_limit_marker() {
    // The huge-canvas fixture's intrinsic dimensions (200000 x 100000) push
    // the render target well past `MAX_TARGET_PIXELS`. The error must carry
    // the structured marker so `error::classify_input` can recover a
    // `SizeLimit` failure instead of a generic decode error.
    let file = fixture("huge-canvas.svg");
    let err = SvgDecoder::try_new(file)
        .err()
        .expect("expected size-limit error");
    let message = err.to_string();

    assert!(
        message.starts_with(SIZE_LIMIT_MARKER),
        "message must start with the structured marker; got: {message}"
    );
    let parsed = parse_size_limit(&message).expect("marker must be parseable");
    let (width, height, actual, allowed) = parsed;
    assert!(
        actual > allowed,
        "actual={actual} must exceed allowed={allowed}"
    );
    assert_eq!(width * height, actual);
}

#[test]
fn parse_size_limit_rejects_messages_without_the_marker() {
    assert!(parse_size_limit("plain text").is_none());
    assert!(parse_size_limit("100x100:10000:8000 trailing").is_none());
    assert!(parse_size_limit("").is_none());
}

fn fixture(name: &str) -> Cursor<Vec<u8>> {
    let svg = match name {
        "rect.svg" => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="100" height="50" fill="red"/></svg>"#
        }
        "no-viewbox.svg" => {
            r#"<svg xmlns="http://www.w3.org/2000/svg"><circle cx="120" cy="80" r="40"/></svg>"#
        }
        "text-cjk.svg" => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><text x="0" y="30">中文 日本語 ABC</text></svg>"#
        }
        "huge-canvas.svg" => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200000" height="100000"/>"#
        }
        "invalid.svg" => "<not-svg",
        _ => panic!("unknown fixture"),
    };
    Cursor::new(svg.as_bytes().to_vec())
}
