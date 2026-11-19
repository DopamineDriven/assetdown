use super::*;
use crate::source::Piece;
use crate::testing::{XMP_PACKET, boxed, exif_block, ftyp, image_facts};
use crate::{ColorModel, ColorSpace, ImageDetail, MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &JxlDetail {
  match &facts.detail {
    ImageDetail::Jxl(detail) => detail,
    other => panic!("not a JPEG XL's detail: {other:?}"),
  }
}

/// Bits as a JPEG XL header keeps them: the first in the lowest place of a byte.
#[derive(Default, Clone)]
struct Bits {
  bytes: Vec<u8>,
  used: usize,
}

impl Bits {
  fn put(mut self, value: u32, count: usize) -> Self {
    for place in 0..count {
      if self.used.is_multiple_of(8) {
        self.bytes.push(0);
      }
      let bit = u8::from((value >> place) & 1 == 1);
      *self.bytes.last_mut().unwrap() |= bit << (self.used % 8);
      self.used += 1;
    }
    self
  }

  fn flag(self, on: bool) -> Self {
    self.put(u32::from(on), 1)
  }

  /// One of the four ways a number can be written: which way, and then its bits.
  fn way(self, which: u32, value: u32, count: usize) -> Self {
    self.put(which, 2).put(value, count)
  }

  /// A size written out in full: the height, no fixed shape, the width.
  fn size(self, width: u32, height: u32) -> Self {
    self
      .flag(false)
      .way(1, height - 1, 13)
      .put(0, 3)
      .way(1, width - 1, 13)
  }

  /// A size whose sides are multiples of eight, with the width in a fixed shape to the height.
  fn small(self, height: u32, shape: u32) -> Self {
    self.flag(true).put(height / 8 - 1, 5).put(shape, 3)
  }

  /// The start of a description that is not all as usual and has none of the rarer fields:
  /// then the depth of a sample, here whole numbers of eight bits.
  fn described(self) -> Self {
    self
      .flag(false)
      .flag(false)
      .flag(false)
      .put(0, 2)
      .flag(true)
  }

  /// The codestream these bits open, with some of the picture after them: bytes that no text
  /// holds, as a packed picture's are.
  fn codestream(self) -> Vec<u8> {
    [
      &[0xFF, 0x0A][..],
      &self.bytes,
      &[0x00, 0x91, 0x07, 0xE3].repeat(6),
    ]
    .concat()
  }
}

fn start() -> Bits {
  Bits::default()
}

#[test]
fn the_packed_header_gives_the_size() {
  // A description that is all as usual: eight bits, sRGB, no alpha, shown as stored.
  let found = specs(&start().size(640, 480).flag(true).codestream());
  assert_eq!(found.media_type, MediaType::Jxl);
  assert_eq!(found.warnings, Vec::<String>::new());
  assert_eq!(found.needs, None);
  let facts = image_facts(found);
  assert_eq!(
    (facts.width, facts.height, facts.frames, facts.animated),
    (640, 480, 1, false)
  );
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Rgb, ColorSpace::Srgb)
  );
  assert_eq!((facts.has_alpha, facts.orientation), (Some(false), Some(1)));
  assert_eq!(
    detail(&facts),
    &JxlDetail {
      container: false,
      bits_per_sample: 8,
      float_samples: false,
      xyb: true,
      loops: None,
      jpeg_reconstruction: false,
    }
  );
}

#[test]
fn a_size_can_be_written_as_a_height_and_a_shape() {
  let sized = |bits: Bits| {
    let facts = image_facts(specs(&bits.flag(true).codestream()));
    (facts.width, facts.height)
  };
  // In eighths, with the width equal to the height.
  assert_eq!(sized(start().small(64, 1)), (64, 64));
  // In eighths, with a width of its own.
  assert_eq!(sized(start().small(64, 0).put(256 / 8 - 1, 5)), (256, 64));
  // In full, in each of the shapes the format has a number for.
  let shaped =
    |height: u32, shape: u32| sized(start().flag(false).way(1, height - 1, 13).put(shape, 3));
  assert_eq!(shaped(100, 2), (120, 100));
  assert_eq!(shaped(480, 3), (640, 480));
  assert_eq!(shaped(100, 4), (150, 100));
  assert_eq!(shaped(1080, 5), (1920, 1080));
  assert_eq!(shaped(100, 6), (125, 100));
  assert_eq!(shaped(100, 7), (200, 100));
  // The other ways a side's number can be written: in nine bits, eighteen and thirty.
  assert_eq!(
    sized(
      start()
        .flag(false)
        .way(0, 299, 9)
        .put(0, 3)
        .way(2, 99_999, 18)
    ),
    (100_000, 300)
  );
  assert_eq!(
    sized(start().flag(false).way(3, 4_999_999, 30).put(1, 3)),
    (5_000_000, 5_000_000)
  );
}

#[test]
fn the_rarer_fields_say_how_the_picture_is_turned_and_how_deep_a_sample_is() {
  let bits = start()
    .size(640, 480)
    // Not all as usual; the rarer fields follow; turned as EXIF's 6; no size of its own, no
    // smaller copy, no animation.
    .flag(false)
    .flag(true)
    .put(5, 3)
    .flag(false)
    .flag(false)
    .flag(false)
    // Whole numbers of sixteen bits.
    .flag(false)
    .way(3, 15, 6)
    .flag(true)
    // No channels beyond the color's; not in the format's own space; color as usual.
    .put(0, 2)
    .flag(false)
    .flag(true);
  let facts = image_facts(specs(&bits.codestream()));
  assert_eq!(facts.orientation, Some(6));
  let detail = detail(&facts);
  assert_eq!(
    (detail.bits_per_sample, detail.float_samples, detail.xyb),
    (16, false, false)
  );
}

#[test]
fn samples_that_are_floating_point_numbers_say_so() {
  let bits = start()
    .size(8, 8)
    .flag(false)
    .flag(false)
    .flag(true)
    .way(0, 0, 0)
    .put(7, 4)
    .flag(true)
    .put(0, 2)
    .flag(true)
    .flag(true);
  let facts = image_facts(specs(&bits.codestream()));
  let detail = detail(&facts);
  assert_eq!((detail.bits_per_sample, detail.float_samples), (32, true));
}

#[test]
fn a_channel_beyond_the_color_s_is_alpha_when_it_says_so() {
  // One more channel, which is as such a channel usually is: alpha.
  let usual = start()
    .size(8, 8)
    .described()
    .put(1, 2)
    .flag(true)
    .flag(true)
    .flag(true);
  let facts = image_facts(specs(&usual.codestream()));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(true), ColorModel::Rgba)
  );

  // Two more: a named channel of depth, and then alpha written out in full.
  let named = start()
    .size(8, 8)
    .described()
    .way(2, 0, 4)
    // Not as usual; depth; eight bits; not shrunk; a name of three letters.
    .flag(false)
    .put(1, 2)
    .flag(false)
    .put(0, 2)
    .put(0, 2)
    .way(1, 3, 4)
    .put(u32::from(b'f'), 8)
    .put(u32::from(b'a'), 8)
    .put(u32::from(b'r'), 8)
    // Not as usual; alpha; eight bits; not shrunk; no name; not multiplied into the color.
    .flag(false)
    .put(0, 2)
    .flag(false)
    .put(0, 2)
    .put(0, 2)
    .put(0, 2)
    .flag(false)
    .flag(true)
    .flag(true);
  let facts = image_facts(specs(&named.codestream()));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(true), ColorModel::Rgba)
  );

  // A channel of depth alone is not alpha.
  let depth = start()
    .size(8, 8)
    .described()
    .put(1, 2)
    .flag(false)
    .put(1, 2)
    .flag(false)
    .put(0, 2)
    .put(0, 2)
    .put(0, 2)
    .flag(true)
    .flag(true);
  let facts = image_facts(specs(&depth.codestream()));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(false), ColorModel::Rgb)
  );
}

#[test]
fn a_channel_of_black_makes_the_picture_cmyk() {
  // One more channel; not as usual; the fifth kind, written in the long way; eight bits.
  let bits = start()
    .size(8, 8)
    .described()
    .put(1, 2)
    .flag(false)
    .way(2, 2, 4)
    .flag(false)
    .put(0, 2)
    .put(0, 2)
    .put(0, 2)
    .flag(false)
    .flag(true);
  let facts = image_facts(specs(&bits.codestream()));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Cmyk, ColorSpace::Cmyk)
  );
}

/// A description with no channels beyond the color's, as far as how the color is kept.
fn colored() -> Bits {
  start()
    .size(8, 8)
    .described()
    .put(0, 2)
    .flag(false)
    .flag(false)
}

#[test]
fn how_color_is_kept_is_read_from_the_header() {
  let kept = |bits: Bits| {
    let facts = image_facts(specs(&bits.codestream()));
    (facts.color_model, facts.color_space, facts.icc_profile)
  };
  // No profile; gray; the white of daylight.
  assert_eq!(
    kept(colored().flag(false).put(1, 2).put(1, 2)),
    (ColorModel::Grayscale, ColorSpace::Gray, None)
  );
  // No profile; red, green and blue; the white of daylight; then which red, green and blue.
  let primaries = |which: u32| {
    colored()
      .flag(false)
      .put(0, 2)
      .put(1, 2)
      .way(2, which - 2, 4)
  };
  assert_eq!(
    kept(colored().flag(false).put(0, 2).put(1, 2).put(1, 2)),
    (ColorModel::Rgb, ColorSpace::Srgb, None)
  );
  assert_eq!(
    kept(primaries(11)),
    (ColorModel::Rgb, ColorSpace::DisplayP3, None)
  );
  assert_eq!(
    kept(primaries(9)),
    (ColorModel::Rgb, ColorSpace::Rec2020, None)
  );
  // A white of its own, written as two numbers, and then the usual red, green and blue.
  let custom = colored()
    .flag(false)
    .put(0, 2)
    .way(2, 0, 4)
    .way(0, 1000, 19)
    .way(0, 2000, 19)
    .put(1, 2);
  assert_eq!(kept(custom), (ColorModel::Rgb, ColorSpace::Unknown, None));
  // A profile, which is packed in a way that is not read here.
  assert_eq!(
    kept(colored().flag(true).put(0, 2)),
    (
      ColorModel::Rgb,
      ColorSpace::Unknown,
      Some("embedded".to_string())
    )
  );
}

#[test]
fn an_animation_says_how_often_it_plays_and_its_frames_are_not_counted() {
  let bits = start()
    .size(64, 64)
    // The rarer fields: shown as stored, no size of its own, no smaller copy, an animation.
    .flag(false)
    .flag(true)
    .put(0, 3)
    .flag(false)
    .flag(false)
    .flag(true)
    // A hundred ticks to the second, played three times, with no time codes.
    .put(0, 2)
    .put(0, 2)
    .way(1, 3, 3)
    .flag(false)
    .flag(false)
    .put(0, 2)
    .flag(true)
    .put(0, 2)
    .flag(true)
    .flag(true);
  let found = specs(&bits.codestream());
  assert_eq!(
    found.warnings,
    ["the frames of an animated JPEG XL are not counted"]
  );
  let facts = image_facts(found);
  assert_eq!((facts.animated, facts.frames), (true, 1));
  assert_eq!(detail(&facts).loops, Some(3));
}

const SIGNATURE: &[u8] = b"\0\0\0\x0CJXL \r\n\x87\n";

#[test]
fn a_codestream_in_a_file_of_boxes_is_read_the_same() {
  let codestream = start().size(640, 480).flag(true).codestream();
  let file = [
    SIGNATURE.to_vec(),
    ftyp(b"jxl ", &[b"jxl "]),
    boxed(b"jxlc", &codestream),
  ]
  .concat();
  let found = specs(&file);
  assert_eq!(found.media_type, MediaType::Jxl);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (640, 480));
  assert!(detail(&facts).container);
}

#[test]
fn a_codestream_kept_in_pieces_is_put_together() {
  let codestream = start().size(640, 480).flag(true).codestream();
  // Each piece opens with its place in the order; the last has its highest bit set.
  let first = boxed(
    b"jxlp",
    &[&0u32.to_be_bytes()[..], &codestream[..3]].concat(),
  );
  let last = boxed(
    b"jxlp",
    &[&0x8000_0001u32.to_be_bytes()[..], &codestream[3..]].concat(),
  );
  let file = [
    SIGNATURE.to_vec(),
    ftyp(b"jxl ", &[b"jxl "]),
    first,
    boxed(b"jxll", &[5]),
    last,
  ]
  .concat();
  let facts = image_facts(specs(&file));
  assert_eq!((facts.width, facts.height), (640, 480));
}

#[test]
fn what_a_camera_wrote_down_is_read_from_boxes_of_its_own() {
  let codestream = start().size(640, 480).flag(true).codestream();
  let file = [
    SIGNATURE.to_vec(),
    ftyp(b"jxl ", &[b"jxl "]),
    // Four bytes that say where the block starts, and the block.
    boxed(b"Exif", &[&[0, 0, 0, 0][..], &exif_block(true)].concat()),
    boxed(b"xml ", XMP_PACKET.as_bytes()),
    boxed(b"jbrd", &[1, 2, 3]),
    boxed(b"jxlc", &codestream),
  ]
  .concat();
  let facts = image_facts(specs(&file));
  assert!(facts.camera_make.is_some());
  assert!(facts.xmp.is_some());
  assert!(detail(&facts).jpeg_reconstruction);
  // How the picture is turned is the codestream's to say, whatever the camera wrote.
  assert_eq!(facts.orientation, Some(1));
}

#[test]
fn a_header_that_runs_past_what_was_given_is_asked_for() {
  let whole = start().size(640, 480).flag(true).codestream();
  let pieces = [Piece {
    offset: 0,
    bytes: &whole[..4],
  }];
  let size = u64::try_from(whole.len()).unwrap();
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Jxl);
  assert_eq!(found.facts, crate::Facts::None);
  assert_eq!(found.needs.map(|range| range.offset), Some(4));
  assert_eq!(found.warnings, Vec::<String>::new());
}

#[test]
fn a_file_that_ends_inside_its_header_says_so() {
  let found = specs(&[0xFF, 0x0A, 0x00, 0x01]);
  assert_eq!(found.media_type, MediaType::Jxl);
  assert_eq!(found.facts, crate::Facts::None);
  assert_eq!(found.warnings, ["the file ends inside its header"]);
}

#[test]
fn what_a_camera_wrote_down_and_the_file_keeps_compressed_is_not_read_and_says_so() {
  let codestream = start().size(640, 480).flag(true).codestream();
  // A box that holds another, compressed: the name of the one it holds, and then its bytes.
  let compressed = boxed(b"brob", &[&b"Exif"[..], &[0x1B, 0x40, 0x00, 0x91]].concat());
  let file = [
    SIGNATURE.to_vec(),
    ftyp(b"jxl ", &[b"jxl "]),
    compressed,
    boxed(b"jxlc", &codestream),
  ]
  .concat();
  let found = specs(&file);
  assert_eq!(
    found.warnings,
    ["what a camera wrote down is compressed in a way that is not read here"]
  );
  let facts = image_facts(found);
  assert_eq!(
    (facts.width, facts.height, facts.camera_make),
    (640, 480, None)
  );
}
