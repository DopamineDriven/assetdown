use super::*;
use crate::source::Piece;
use crate::testing::image_facts;
use crate::{ColorModel, ColorSpace, ImageDetail, MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &HdrDetail {
  match &facts.detail {
    ImageDetail::Hdr(detail) => detail,
    other => panic!("not a Radiance picture's detail: {other:?}"),
  }
}

/// A header, the line that gives the size, and a few bytes of picture.
fn file(header: &str, resolution: &str) -> Vec<u8> {
  [
    format!("#?RADIANCE\n{header}\n{resolution}\n").as_bytes(),
    &[2, 2, 2, 128, 0x80, 0x10],
  ]
  .concat()
}

#[test]
fn the_line_after_the_header_gives_the_size() {
  let found = specs(&file(
    "# Made with a test\nSOFTWARE=a test 1.0\nFORMAT=32-bit_rle_rgbe\n",
    "-Y 480 +X 640",
  ));
  assert_eq!(found.media_type, MediaType::Hdr);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (640, 480, 1));
  assert_eq!((facts.has_alpha, facts.orientation), (Some(false), Some(1)));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Rgb, ColorSpace::Unknown)
  );
  let detail = detail(&facts);
  assert_eq!(detail.encoding, HdrEncoding::Rgbe);
  assert_eq!(detail.software.as_deref(), Some("a test 1.0"));
  assert_eq!(detail.exposure, None);
}

#[test]
fn a_header_that_says_xyz_is_in_that_space() {
  let facts = image_facts(specs(&file("FORMAT=32-bit_rle_xyze\n", "-Y 8 +X 16")));
  assert_eq!(detail(&facts).encoding, HdrEncoding::Xyze);
  assert_eq!(facts.color_space, ColorSpace::Xyz);
  assert_eq!(HdrEncoding::Xyze.as_str(), "xyze");
  // A header that names no format is the usual one.
  assert_eq!(
    detail(&image_facts(specs(&file("", "-Y 8 +X 16")))).encoding,
    HdrEncoding::Rgbe
  );
}

#[test]
fn exposures_multiply() {
  let facts = image_facts(specs(&file(
    "EXPOSURE=2\nFORMAT=32-bit_rle_rgbe\nEXPOSURE=0.25\n",
    "-Y 8 +X 16",
  )));
  assert_eq!(detail(&facts).exposure, Some(0.5));
}

#[test]
fn the_order_and_signs_of_the_size_say_how_the_picture_is_turned() {
  // The first number counts the lines of pixels and the second the pixels in a line, so they
  // are the height and the width of what is stored whichever way it is to be turned.
  let turns = [
    ("-Y 480 +X 640", 1),
    ("-Y 480 -X 640", 2),
    ("+Y 480 -X 640", 3),
    ("+Y 480 +X 640", 4),
    ("+X 480 -Y 640", 5),
    ("-X 480 -Y 640", 6),
    ("-X 480 +Y 640", 7),
    ("+X 480 +Y 640", 8),
  ];
  for (resolution, turn) in turns {
    let facts = image_facts(specs(&file("", resolution)));
    assert_eq!(
      (facts.width, facts.height, facts.orientation),
      (640, 480, Some(turn)),
      "{resolution}"
    );
  }
}

#[test]
fn a_file_that_opens_the_older_way_is_read_the_same() {
  let old = b"#?RGBE\nFORMAT=32-bit_rle_rgbe\n\n-Y 2 +X 3\n\x01\x02";
  let found = specs(old);
  assert_eq!(found.media_type, MediaType::Hdr);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (3, 2));
}

#[test]
fn a_header_that_runs_past_what_was_given_is_asked_for() {
  let whole = file(
    &format!("# {}\nFORMAT=32-bit_rle_rgbe\n", "x".repeat(900)),
    "-Y 480 +X 640",
  );
  let pieces = [Piece {
    offset: 0,
    bytes: &whole[..300],
  }];
  let size = u64::try_from(whole.len()).unwrap();
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Hdr);
  assert_eq!(found.facts, crate::Facts::None);
  let needs = found.needs.expect("the rest of the header");
  assert_eq!(needs.offset, 300);
  assert_eq!(found.warnings, Vec::<String>::new());
}

#[test]
fn a_file_with_no_line_for_its_size_says_so() {
  let found = specs(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\nnot a size\n");
  assert_eq!(found.facts, crate::Facts::None);
  assert_eq!(
    found.warnings,
    ["the line that gives the picture's size is not one"]
  );
  let cut = specs(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n");
  assert_eq!(cut.warnings, ["the file ends inside its header"]);
}
