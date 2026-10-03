use std::io::Cursor;

use super::*;

#[test]
fn decode() {
    let mut data = Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut data)
        .unwrap()
        .write_image::<tiff::encoder::colortype::RGB8>(48, 80, &vec![128; 48 * 80 * 3])
        .unwrap();
    let decoder = TiffDecoder::try_new(Cursor::new(data.into_inner())).unwrap();

    let img = Image::from_decoder(decoder).unwrap();

    assert_eq!(img.dimensions(), (48, 80));
    assert_eq!(img.colorspace(), ColorSpace::RGB);
}
