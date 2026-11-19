use super::*;
use crate::source::Piece;
use crate::testing::{TiffValue, icc_profile, image_facts, tiff_at};
use crate::{ColorModel, ColorSpace, ImageDetail, MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &JxrDetail {
  match &facts.detail {
    ImageDetail::Jxr(detail) => detail,
    other => panic!("not a JPEG XR's detail: {other:?}"),
  }
}

/// The identifier of a pixel format: fifteen bytes every one shares, and the byte that says
/// which it is.
fn format(which: u8) -> TiffValue {
  let shared = [
    0x24, 0xC3, 0xDD, 0x6F, 0x03, 0x4E, 0xFE, 0x4B, 0xB1, 0x85, 0x3D, 0x77, 0x76, 0x8D, 0xC9,
  ];
  TiffValue::Bytes([&shared[..], &[which]].concat())
}

/// A JPEG XR: a TIFF's directory under a header of its own.
fn file(entries: Vec<(u16, TiffValue)>) -> Vec<u8> {
  let mut file = tiff_at(true, 32, &[entries]);
  file[2..4].copy_from_slice(&[0xBC, 0x01]);
  file
}

fn sized(which: u8, width: u32, height: u32) -> Vec<(u16, TiffValue)> {
  vec![
    (0xBC01, format(which)),
    (0xBC80, TiffValue::Long(vec![width])),
    (0xBC81, TiffValue::Long(vec![height])),
    (0xBCC0, TiffValue::Long(vec![400])),
    (0xBCC1, TiffValue::Long(vec![1000])),
  ]
}

#[test]
fn the_directory_gives_the_size_and_the_pixel_format() {
  let found = specs(&file(sized(0x0C, 640, 480)));
  assert_eq!(found.media_type, MediaType::Jxr);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (640, 480, 1));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Rgb, ColorSpace::Srgb)
  );
  assert_eq!((facts.has_alpha, facts.orientation), (Some(false), None));
  let detail = detail(&facts);
  assert_eq!(
    (detail.version, detail.pixel_format, detail.bits_per_pixel),
    (1, Some(0x0C), Some(24))
  );
}

#[test]
fn the_pixel_format_says_how_color_is_kept() {
  let kept = |which: u8| {
    let facts = image_facts(specs(&file(sized(which, 8, 8))));
    (
      facts.color_model,
      facts.color_space,
      facts.has_alpha,
      detail(&facts).bits_per_pixel,
    )
  };
  assert_eq!(
    kept(0x08),
    (
      ColorModel::Grayscale,
      ColorSpace::Gray,
      Some(false),
      Some(8)
    )
  );
  assert_eq!(
    kept(0x0B),
    (
      ColorModel::Grayscale,
      ColorSpace::Gray,
      Some(false),
      Some(16)
    )
  );
  assert_eq!(
    kept(0x0E),
    (ColorModel::Rgb, ColorSpace::Srgb, Some(false), Some(32))
  );
  assert_eq!(
    kept(0x0F),
    (ColorModel::Rgba, ColorSpace::Srgb, Some(true), Some(32))
  );
  assert_eq!(
    kept(0x15),
    (ColorModel::Rgb, ColorSpace::Srgb, Some(false), Some(48))
  );
  assert_eq!(
    kept(0x16),
    (ColorModel::Rgba, ColorSpace::Srgb, Some(true), Some(64))
  );
  assert_eq!(
    kept(0x1C),
    (ColorModel::Cmyk, ColorSpace::Cmyk, Some(false), Some(32))
  );
  assert_eq!(
    kept(0x2C),
    (ColorModel::Cmyk, ColorSpace::Cmyk, Some(true), Some(40))
  );
  // A format of channels that are not named colors, and one that is not known at all.
  assert_eq!(
    kept(0x21),
    (
      ColorModel::Unknown,
      ColorSpace::Unknown,
      Some(false),
      Some(32)
    )
  );
  assert_eq!(
    kept(0xEE),
    (ColorModel::Unknown, ColorSpace::Unknown, None, None)
  );
}

#[test]
fn a_plane_of_alpha_kept_apart_is_alpha_too() {
  let mut entries = sized(0x0C, 8, 8);
  entries.push((0xBCC2, TiffValue::Long(vec![1400])));
  entries.push((0xBCC3, TiffValue::Long(vec![200])));
  assert_eq!(image_facts(specs(&file(entries))).has_alpha, Some(true));
}

#[test]
fn how_the_picture_is_turned_is_given_as_exif_numbers_it() {
  // The format's own numbers, and what EXIF calls the same turn.
  for (written, turn) in [
    (0, 1),
    (1, 4),
    (2, 2),
    (3, 3),
    (4, 6),
    (5, 7),
    (6, 5),
    (7, 8),
  ] {
    let mut entries = sized(0x0C, 8, 8);
    entries.push((0xBC02, TiffValue::Long(vec![written])));
    assert_eq!(
      image_facts(specs(&file(entries))).orientation,
      Some(turn),
      "{written}"
    );
  }
}

#[test]
fn a_profile_and_who_took_the_picture_are_read_as_a_tiff_s_are() {
  let mut entries = sized(0x0C, 8, 8);
  entries.push((0x010F, TiffValue::Text("A Maker")));
  entries.push((
    0x8773,
    TiffValue::Bytes(icc_profile(Some("Display P3"), false, b"RGB ")),
  ));
  let facts = image_facts(specs(&file(entries)));
  assert_eq!(facts.camera_make.as_deref(), Some("A Maker"));
  assert_eq!(facts.icc_profile.as_deref(), Some("Display P3"));
  assert_eq!(facts.color_space, ColorSpace::DisplayP3);
}

#[test]
fn a_file_from_when_the_format_was_hd_photo_is_read_the_same() {
  let mut old = file(sized(0x08, 320, 200));
  old[3] = 0;
  let found = specs(&old);
  assert_eq!(found.media_type, MediaType::Jxr);
  let facts = image_facts(found);
  assert_eq!(
    (facts.width, facts.height, detail(&facts).version),
    (320, 200, 0)
  );
}

#[test]
fn a_directory_that_was_not_given_is_asked_for() {
  let whole = file(sized(0x0C, 640, 480));
  let pieces = [Piece {
    offset: 0,
    bytes: &whole[..16],
  }];
  let size = u64::try_from(whole.len()).unwrap();
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Jxr);
  assert_eq!(found.needs.map(|range| range.offset), Some(32));
}
