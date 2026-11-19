use super::*;
use crate::source::{ByteRange, Piece};
use crate::testing::{XMP_PACKET, exif_block, icc_profile, image_facts, jpeg, jpeg_frame};
use crate::{MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &JpegDetail {
  match &facts.detail {
    ImageDetail::Jpeg(detail) => detail,
    other => panic!("not a JPEG's detail: {other:?}"),
  }
}

fn jfif(unit: u8, across: u16, down: u16) -> (u8, Vec<u8>) {
  (
    0xE0,
    [
      &b"JFIF\0\x01\x02"[..],
      &[unit],
      &across.to_be_bytes(),
      &down.to_be_bytes(),
      &[0, 0],
    ]
    .concat(),
  )
}

fn adobe(transform: u8) -> (u8, Vec<u8>) {
  (0xEE, [&b"Adobe\0\x64\0\0\0\0"[..], &[transform]].concat())
}

#[test]
fn the_frame_header_gives_the_size_and_the_channels() {
  let file = jpeg(&[jfif(1, 300, 300), (0xC0, jpeg_frame(640, 480, 3))]);
  let found = specs(&file);
  assert_eq!(found.media_type, MediaType::Jpeg);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (640, 480));
  assert_eq!(
    (facts.frames, facts.animated, facts.has_alpha),
    (1, false, Some(false))
  );
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Ycbcr, ColorSpace::Srgb)
  );
  assert_eq!((facts.orientation, &facts.icc_profile), (None, &None));
  let detail = detail(&facts);
  assert_eq!(
    (detail.precision, detail.components, detail.progressive),
    (8, 3, false)
  );
  assert_eq!((detail.jfif, detail.dpi), (true, Some((300.0, 300.0))));
}

#[test]
fn a_progressive_picture_and_one_in_shades_of_gray_say_so() {
  let progressive = image_facts(specs(&jpeg(&[(0xC2, jpeg_frame(32, 16, 3))])));
  assert!(detail(&progressive).progressive);
  assert!(!detail(&progressive).jfif);
  let gray = image_facts(specs(&jpeg(&[(0xC0, jpeg_frame(32, 16, 1))])));
  assert_eq!(
    (gray.color_model, gray.color_space),
    (ColorModel::Grayscale, ColorSpace::Gray)
  );
}

#[test]
fn adobe_says_what_was_done_to_the_color() {
  let of = |transform: u8, components: u8| {
    image_facts(specs(&jpeg(&[
      adobe(transform),
      (0xC0, jpeg_frame(8, 8, components)),
    ])))
  };
  assert_eq!(of(0, 3).color_model, ColorModel::Rgb);
  assert_eq!(of(1, 3).color_model, ColorModel::Ycbcr);
  let cmyk = of(0, 4);
  assert_eq!(
    (cmyk.color_model, cmyk.color_space),
    (ColorModel::Cmyk, ColorSpace::Cmyk)
  );
  let ycck = of(2, 4);
  assert_eq!(
    (ycck.color_model, ycck.color_space),
    (ColorModel::Ycck, ColorSpace::Cmyk)
  );
  assert_eq!(detail(&ycck).adobe_transform, Some(2));
}

#[test]
fn a_density_is_given_to_the_inch_whatever_unit_it_is_written_in() {
  let of = |unit: u8| {
    detail(&image_facts(specs(&jpeg(&[
      jfif(unit, 100, 100),
      (0xC0, jpeg_frame(8, 8, 3)),
    ]))))
    .dpi
  };
  assert_eq!(of(1), Some((100.0, 100.0)));
  assert_eq!(of(2), Some((254.0, 254.0)));
  // No unit: only the shape of a pixel, which is not a density.
  assert_eq!(of(0), None);
}

#[test]
fn what_the_camera_wrote_is_read_from_its_own_segment() {
  let block = [&b"Exif\0\0"[..], &exif_block(false)].concat();
  let facts = image_facts(specs(&jpeg(&[
    (0xE1, block),
    (0xC0, jpeg_frame(6000, 4000, 3)),
  ])));
  assert_eq!(facts.orientation, Some(6));
  // The date is in the directory the first one points at.
  assert_eq!(
    facts.exif_date_time_original.as_deref(),
    Some("2026-11-17T23:36:27-05:00")
  );
  assert_eq!(
    (facts.camera_make.as_deref(), facts.camera_model.as_deref()),
    (Some("Leica Camera AG"), Some("M11"))
  );
  assert_eq!(facts.lens_model.as_deref(), Some("Summilux-M 1:1.4/35"));
  assert!(facts.gps_lat.unwrap() < -33.0 && facts.gps_lon.unwrap() < -70.0);
}

#[test]
fn a_packet_and_a_profile_cut_into_pieces_are_read() {
  let packet = [
    &b"http://ns.adobe.com/xap/1.0/\0"[..],
    XMP_PACKET.as_bytes(),
  ]
  .concat();
  let profile = icc_profile(Some("Display P3"), true, b"RGB ");
  let (first, second) = profile.split_at(100);
  let piece =
    |number: u8, bytes: &[u8]| (0xE2, [&b"ICC_PROFILE\0"[..], &[number, 2], bytes].concat());
  // The second piece stands in front of the first.
  let file = jpeg(&[
    (0xE1, packet),
    piece(2, second),
    piece(1, first),
    (0xC0, jpeg_frame(8, 8, 3)),
  ]);
  let facts = image_facts(specs(&file));
  assert_eq!(facts.color_space, ColorSpace::DisplayP3);
  assert_eq!(facts.icc_profile.as_deref(), Some("Display P3"));
  assert_eq!(
    facts.icc.as_ref().map(|profile| profile.version.as_str()),
    Some("4.3.0")
  );
  assert_eq!(
    facts.xmp.as_ref().and_then(|xmp| xmp.title.as_deref()),
    Some("A harbor at dusk")
  );
}

#[test]
fn a_profile_that_cannot_be_read_is_still_said_to_be_there() {
  let broken = (0xE2, b"ICC_PROFILE\0\x01\x01not a profile".to_vec());
  let found = specs(&jpeg(&[broken, (0xC0, jpeg_frame(8, 8, 3))]));
  assert_eq!(found.warnings, ["the color profile could not be read"]);
  let facts = image_facts(found);
  assert_eq!(
    (facts.icc_profile.as_deref(), facts.color_space),
    (Some("embedded"), ColorSpace::Srgb)
  );
}

#[test]
fn markers_may_be_padded_and_a_file_with_no_frame_says_so() {
  let mut padded = jpeg(&[(0xC0, jpeg_frame(64, 48, 3))]);
  padded.splice(2..2, [0xFF, 0xFF]);
  assert_eq!(image_facts(specs(&padded)).width, 64);

  let frameless = specs(&jpeg(&[jfif(1, 72, 72)]));
  assert_eq!(
    (frameless.media_type, &frameless.facts),
    (MediaType::Jpeg, &crate::Facts::None)
  );
  assert_eq!(frameless.warnings, ["the file has no frame header"]);
}

#[test]
fn a_head_that_stops_before_the_frame_header_asks_for_a_block_from_where_it_stopped() {
  let note = (
    0xE1,
    [&b"Exif\0\0"[..], &exif_block(true), &[0; 3000]].concat(),
  );
  let file = jpeg(&[note, (0xC0, jpeg_frame(640, 480, 3))]);
  let head = |length: usize| {
    let pieces = [Piece {
      offset: 0,
      bytes: &file[..length],
    }];
    specs_source(&Source::from_pieces(&pieces, None).unwrap())
  };
  let short = head(1024);
  assert_eq!(short.facts, crate::Facts::None);
  // What is left of the segment it stopped in is asked for, made up to a block: the frame
  // header is seldom far behind it, and asking for each few bytes in turn takes a fetch for
  // every one.
  assert_eq!(short.needs, Some(ByteRange::forward(1024, 4096)));
  let whole = head(file.len());
  assert_eq!(
    (whole.needs, image_facts(whole).width),
    (None::<ByteRange>, 640)
  );
}

/// A JPEG-LS: a frame header of its own kind, and a scan header that says how far a sample may
/// be from what was recorded.
fn ls_file(components: u8, near: u8) -> Vec<u8> {
  let mut file = jpeg(&[(0xF7, jpeg_frame(640, 480, components))]);
  // In place of the scan the builder ends with.
  file.truncate(file.len() - 6);
  let selectors: Vec<u8> = (1..=components).flat_map(|id| [id, 0]).collect();
  file.extend([0xFF, 0xDA]);
  file.extend(u16::from(6 + 2 * components).to_be_bytes());
  file.push(components);
  file.extend(selectors);
  file.extend([near, 0, 0]);
  file.extend([0, 0, 0xFF, 0xD9]);
  file
}

#[test]
fn a_jpeg_ls_has_a_jpeg_s_facts_and_says_whether_it_lost_anything() {
  let found = specs(&ls_file(3, 0));
  assert_eq!(found.media_type, MediaType::Jls);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (640, 480));
  // It has no transform of color of its own: three channels are red, green and blue.
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Rgb, ColorSpace::Srgb)
  );
  assert!(detail(&facts).lossless);
  assert!(!detail(&image_facts(specs(&ls_file(3, 2)))).lossless);
  assert_eq!(
    image_facts(specs(&ls_file(1, 0))).color_model,
    ColorModel::Grayscale
  );
}

#[test]
fn a_jpeg_of_the_lossless_kind_says_so() {
  let kept = image_facts(specs(&jpeg(&[(0xC3, jpeg_frame(64, 64, 1))])));
  assert!(detail(&kept).lossless);
  let ordinary = image_facts(specs(&jpeg(&[(0xC0, jpeg_frame(64, 64, 3))])));
  assert!(!detail(&ordinary).lossless);
}

#[test]
fn a_jpeg_ls_whose_scan_header_was_not_given_asks_for_it() {
  let file = ls_file(3, 0);
  let head = file.len() - 12;
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..head],
  }];
  let size = u64::try_from(file.len()).unwrap();
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Jls);
  assert!(found.needs.is_some(), "{found:?}");
}
