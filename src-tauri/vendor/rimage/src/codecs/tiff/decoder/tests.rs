use std::io::Cursor;

use super::*;

#[test]
fn decode() {
    let mut data = Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut data)
        .unwrap()
        .write_image::<tiff::encoder::colortype::RGB8>(48, 80, &vec![128; 48 * 80 * 3])
        .unwrap();
    let bytes = data.into_inner();
    let mut decoder = TiffDecoder::try_new(Cursor::new(&bytes)).unwrap();
    assert_eq!(
        decoder.probe().unwrap(),
        ((48, 80), ColorSpace::RGB, 8, false)
    );

    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (48, 80));
    assert_eq!(img.colorspace(), ColorSpace::RGB);

    let mut limited =
        TiffDecoder::try_new_with_limits(Cursor::new(&bytes), 256 * 1024 * 1024, 48 * 80 - 1)
            .unwrap();
    assert!(limited.decode().is_err());
}
