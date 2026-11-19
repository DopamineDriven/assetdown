use super::*;
use crate::testing::image_facts;
use crate::{ImageDetail, specs};

/// A bitmap whose second header is `header`, with the file header in front and no pixels.
fn bmp(header: &[u8]) -> Vec<u8> {
  let pixels_at = u32::try_from(14 + header.len()).unwrap();
  [
    &b"BM"[..],
    &pixels_at.to_le_bytes(),
    &[0; 4],
    &pixels_at.to_le_bytes(),
    header,
  ]
  .concat()
}

/// The header Windows has written since 3.1, lengthened with zeros to a later version's size.
fn header(bytes: u32, width: i32, height: i32, bits: u16, compression: u32) -> Vec<u8> {
  let mut header = [
    &bytes.to_le_bytes()[..],
    &width.to_le_bytes(),
    &height.to_le_bytes(),
    &1u16.to_le_bytes(),
    &bits.to_le_bytes(),
    &compression.to_le_bytes(),
    &[0; 4],
    &2835u32.to_le_bytes(),
    &2835u32.to_le_bytes(),
  ]
  .concat();
  header.resize(usize::try_from(bytes).unwrap(), 0);
  header
}

fn detail(facts: &ImageFacts) -> &BmpDetail {
  match &facts.detail {
    ImageDetail::Bmp(detail) => detail,
    other => panic!("not a bitmap's detail: {other:?}"),
  }
}

#[test]
fn an_ordinary_bitmap_is_stored_from_the_bottom_up_and_is_not_turned() {
  let facts = image_facts(specs(&bmp(&header(40, 100, 50, 24, 0))));
  assert_eq!((facts.width, facts.height), (100, 50));
  assert_eq!((facts.frames, facts.animated), (1, false));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Rgb, ColorSpace::Unknown)
  );
  assert_eq!(facts.has_alpha, Some(false));
  // The order of the rows is not a rotation.
  assert_eq!(facts.orientation, None);
  let detail = detail(&facts);
  assert_eq!((detail.header_bytes, detail.bits_per_pixel), (40, 24));
  assert_eq!(
    (detail.compression.as_str(), detail.top_down),
    ("rgb", false)
  );
  let (across, down) = detail.dpi.unwrap();
  assert!((across - 72.0).abs() < 0.01 && (down - 72.0).abs() < 0.01);
}

#[test]
fn a_negative_height_is_a_bitmap_stored_from_the_top_down() {
  let facts = image_facts(specs(&bmp(&header(40, 100, -50, 24, 0))));
  assert_eq!((facts.width, facts.height), (100, 50));
  assert!(detail(&facts).top_down);
}

#[test]
fn eight_bits_or_fewer_is_a_palette() {
  let facts = image_facts(specs(&bmp(&header(40, 16, 16, 8, 1))));
  assert_eq!(facts.color_model, ColorModel::Indexed);
  assert_eq!(detail(&facts).compression, "rle8");
}

#[test]
fn alpha_is_what_a_mask_says_it_is() {
  // 32 bits and no mask: the fourth byte is not said to be alpha.
  let plain = image_facts(specs(&bmp(&header(40, 8, 8, 32, 0))));
  assert_eq!(
    (plain.has_alpha, plain.color_model),
    (Some(false), ColorModel::Rgb)
  );

  // The header of Windows 95 and later carries the masks and names the color space.
  let mut later = header(124, 8, 8, 32, 3);
  later[52..56].copy_from_slice(&0xFF00_0000u32.to_le_bytes());
  later[56..60].copy_from_slice(b"BGRs");
  let masked = image_facts(specs(&bmp(&later)));
  assert_eq!(
    (masked.has_alpha, masked.color_model),
    (Some(true), ColorModel::Rgba)
  );
  assert_eq!(masked.color_space, ColorSpace::Srgb);
  assert_eq!(detail(&masked).compression, "bitfields");

  // The older header, with four masks after it.
  let after = [
    header(40, 8, 8, 32, 6),
    vec![0; 12],
    0xFF00_0000u32.to_le_bytes().to_vec(),
  ]
  .concat();
  assert_eq!(image_facts(specs(&bmp(&after))).has_alpha, Some(true));
}

#[test]
fn the_first_version_of_the_header_keeps_its_size_in_two_bytes_each() {
  let old = [
    &12u32.to_le_bytes()[..],
    &300u16.to_le_bytes(),
    &200u16.to_le_bytes(),
    &[1, 0, 4, 0],
  ]
  .concat();
  let facts = image_facts(specs(&bmp(&old)));
  assert_eq!((facts.width, facts.height), (300, 200));
  assert_eq!(facts.color_model, ColorModel::Indexed);
  assert_eq!(
    (detail(&facts).header_bytes, detail(&facts).bits_per_pixel),
    (12, 4)
  );
}
