use super::*;
use crate::source::Piece;
use crate::testing::{XMP_PACKET, boxed, ftyp, icc_profile, image_facts};
use crate::{ColorModel, ColorSpace, ImageDetail, MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &Jp2Detail {
  match &facts.detail {
    ImageDetail::Jp2(detail) => detail,
    other => panic!("not a JPEG 2000's detail: {other:?}"),
  }
}

const SIGNATURE: &[u8] = b"\0\0\0\x0CjP  \r\n\x87\n";

/// The box that gives the picture's size, its channels and the bits of a sample.
fn image_header(width: u32, height: u32, components: u16, bits: u8) -> Vec<u8> {
  let payload = [
    &height.to_be_bytes()[..],
    &width.to_be_bytes(),
    &components.to_be_bytes(),
    &[bits - 1, 7, 0, 0],
  ]
  .concat();
  boxed(b"ihdr", &payload)
}

/// A color space the format has a number for.
fn numbered_color(space: u32) -> Vec<u8> {
  boxed(b"colr", &[&[1, 0, 0][..], &space.to_be_bytes()].concat())
}

/// What each channel is: its number, whether it is color (0) or opacity (1), and the color it
/// belongs to.
fn channels(defined: &[(u16, u16, u16)]) -> Vec<u8> {
  let count = u16::try_from(defined.len()).unwrap();
  let list: Vec<u8> = defined
    .iter()
    .flat_map(|(channel, kind, of)| {
      [channel.to_be_bytes(), kind.to_be_bytes(), of.to_be_bytes()].concat()
    })
    .collect();
  boxed(b"cdef", &[&count.to_be_bytes()[..], &list].concat())
}

/// The picture itself as far as its size marker: the image area and where it starts on the
/// grid, the tiles, and the depth of each channel.
fn codestream(area: (u32, u32), origin: (u32, u32), depths: &[u8]) -> Vec<u8> {
  let count = u16::try_from(depths.len()).unwrap();
  let mut marker = [
    &0u16.to_be_bytes()[..],
    &area.0.to_be_bytes(),
    &area.1.to_be_bytes(),
    &origin.0.to_be_bytes(),
    &origin.1.to_be_bytes(),
  ]
  .concat();
  marker.extend([area.0.to_be_bytes(), area.1.to_be_bytes(), [0; 4], [0; 4]].concat());
  marker.extend(count.to_be_bytes());
  marker.extend(depths.iter().flat_map(|depth| [depth - 1, 1, 1]));
  let length = u16::try_from(marker.len() + 2).unwrap();
  [
    &[0xFF, 0x4F, 0xFF, 0x51][..],
    &length.to_be_bytes(),
    &marker,
    &[0xFF, 0x52, 0, 0],
  ]
  .concat()
}

/// A JPEG 2000 file: the signature, the brand, the header's boxes, any others, and the picture.
fn file(major: &[u8; 4], header: &[Vec<u8>], others: &[Vec<u8>]) -> Vec<u8> {
  [
    SIGNATURE.to_vec(),
    ftyp(major, &[major, b"jp2 "]),
    boxed(b"jp2h", &header.concat()),
    others.concat(),
    boxed(b"jp2c", &codestream((640, 480), (0, 0), &[8, 8, 8])),
  ]
  .concat()
}

#[test]
fn the_header_box_gives_the_size_the_channels_and_the_depth() {
  let found = specs(&file(
    b"jp2 ",
    &[image_header(640, 480, 3, 8), numbered_color(16)],
    &[],
  ));
  assert_eq!(found.media_type, MediaType::Jp2);
  assert_eq!(found.warnings, Vec::<String>::new());
  assert_eq!(found.needs, None);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (640, 480, 1));
  assert_eq!(
    (facts.color_model, facts.color_space, facts.has_alpha),
    (ColorModel::Rgb, ColorSpace::Srgb, Some(false))
  );
  assert_eq!(
    detail(&facts),
    &Jp2Detail {
      components: 3,
      bits_per_sample: Some(8),
      enumerated_color: Some(16),
      codestream: false,
    }
  );
}

#[test]
fn the_number_of_the_color_space_says_how_color_is_kept() {
  let kept = |space: u32, components: u16| {
    let facts = image_facts(specs(&file(
      b"jpx ",
      &[image_header(8, 8, components, 8), numbered_color(space)],
      &[],
    )));
    (facts.color_model, facts.color_space)
  };
  assert_eq!(kept(17, 1), (ColorModel::Grayscale, ColorSpace::Gray));
  assert_eq!(kept(18, 3), (ColorModel::Ycbcr, ColorSpace::Srgb));
  assert_eq!(kept(12, 4), (ColorModel::Cmyk, ColorSpace::Cmyk));
  assert_eq!(kept(14, 3), (ColorModel::Lab, ColorSpace::Lab));
  assert_eq!(kept(21, 3), (ColorModel::Rgb, ColorSpace::ProphotoRgb));
  // A number that is not one of these says nothing of the color.
  assert_eq!(kept(99, 3), (ColorModel::Unknown, ColorSpace::Unknown));
}

#[test]
fn a_channel_defined_as_opacity_is_alpha() {
  let defined = channels(&[(0, 0, 1), (1, 0, 2), (2, 0, 3), (3, 1, 0)]);
  let facts = image_facts(specs(&file(
    b"jp2 ",
    &[image_header(8, 8, 4, 8), numbered_color(16), defined],
    &[],
  )));
  assert_eq!(
    (facts.color_model, facts.has_alpha),
    (ColorModel::Rgba, Some(true))
  );
  let gray = channels(&[(0, 0, 1), (1, 2, 0)]);
  let facts = image_facts(specs(&file(
    b"jp2 ",
    &[image_header(8, 8, 2, 8), numbered_color(17), gray],
    &[],
  )));
  assert_eq!(
    (facts.color_model, facts.has_alpha),
    (ColorModel::GrayscaleAlpha, Some(true))
  );
}

#[test]
fn a_palette_makes_the_picture_indexed() {
  let palette = boxed(b"pclr", &[0, 2, 3, 7, 7, 7, 0, 0, 0, 255, 255, 255]);
  let facts = image_facts(specs(&file(
    b"jp2 ",
    &[image_header(8, 8, 1, 8), palette, numbered_color(16)],
    &[],
  )));
  assert_eq!(facts.color_model, ColorModel::Indexed);
}

#[test]
fn a_profile_in_the_header_is_read() {
  let profile = boxed(
    b"colr",
    &[
      &[2, 0, 0][..],
      &icc_profile(Some("Display P3"), false, b"RGB "),
    ]
    .concat(),
  );
  let facts = image_facts(specs(&file(
    b"jp2 ",
    &[image_header(8, 8, 3, 8), profile],
    &[],
  )));
  assert_eq!(facts.icc_profile.as_deref(), Some("Display P3"));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Rgb, ColorSpace::DisplayP3)
  );
  assert_eq!(detail(&facts).enumerated_color, None);
}

#[test]
fn channels_of_different_depths_have_no_one_depth() {
  let mut header = image_header(8, 8, 3, 8);
  // In place of a depth, the header says to look at each channel.
  let depth_at = header.len() - 4;
  header[depth_at] = 255;
  let facts = image_facts(specs(&file(b"jp2 ", &[header, numbered_color(16)], &[])));
  assert_eq!(detail(&facts).bits_per_sample, None);
}

#[test]
fn an_xmp_packet_in_a_box_of_its_own_is_read() {
  let packet = [
    &[
      0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF,
      0xAC,
    ][..],
    XMP_PACKET.as_bytes(),
  ]
  .concat();
  let found = specs(&file(
    b"jp2 ",
    &[image_header(8, 8, 3, 8), numbered_color(16)],
    &[boxed(b"uuid", &packet)],
  ));
  let facts = image_facts(found);
  assert!(facts.xmp.is_some());
}

#[test]
fn a_codestream_with_no_boxes_gives_its_size_from_its_own_marker() {
  // The picture is what is left of the area once its offset on the grid is taken off.
  let found = specs(&codestream((660, 500), (20, 20), &[8, 8, 8]));
  assert_eq!(found.media_type, MediaType::J2c);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (640, 480));
  assert_eq!(
    (facts.color_model, facts.color_space, facts.has_alpha),
    (ColorModel::Rgb, ColorSpace::Unknown, None)
  );
  assert_eq!(
    detail(&facts),
    &Jp2Detail {
      components: 3,
      bits_per_sample: Some(8),
      enumerated_color: None,
      codestream: true,
    }
  );
  let gray = image_facts(specs(&codestream((16, 16), (0, 0), &[12])));
  assert_eq!(
    (gray.color_model, detail(&gray).bits_per_sample),
    (ColorModel::Grayscale, Some(12))
  );
  let mixed = image_facts(specs(&codestream((16, 16), (0, 0), &[8, 8, 8, 1])));
  assert_eq!(detail(&mixed).bits_per_sample, None);
}

#[test]
fn a_file_with_no_header_box_is_read_from_its_picture() {
  let bare = [
    SIGNATURE.to_vec(),
    ftyp(b"jp2 ", &[b"jp2 "]),
    boxed(b"jp2c", &codestream((640, 480), (0, 0), &[8])),
  ]
  .concat();
  let found = specs(&bare);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (640, 480));
  assert_eq!(detail(&facts).components, 1);
}

#[test]
fn a_header_box_that_was_not_given_is_asked_for() {
  let whole = file(
    b"jp2 ",
    &[image_header(640, 480, 3, 8), numbered_color(16)],
    &[],
  );
  let pieces = [Piece {
    offset: 0,
    bytes: &whole[..40],
  }];
  let size = u64::try_from(whole.len()).unwrap();
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Jp2);
  assert_eq!(found.facts, crate::Facts::None);
  assert!(found.needs.is_some(), "{found:?}");
  assert_eq!(found.warnings, Vec::<String>::new());
}

#[test]
fn a_file_that_ends_before_its_picture_says_so() {
  let cut = [SIGNATURE.to_vec(), ftyp(b"jp2 ", &[b"jp2 "])].concat();
  let found = specs(&cut);
  assert_eq!(found.media_type, MediaType::Jp2);
  assert_eq!(found.facts, crate::Facts::None);
  assert_eq!(
    found.warnings,
    ["the file has no header box and no picture"]
  );
}
