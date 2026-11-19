//! JPEG XL: a codestream whose header is packed bit by bit, bare or in a file of boxes that
//! can also hold what a camera wrote down.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::{exif, isobmff, xmp};
use crate::source::{ByteRange, Missing, Source};

/// What only a JPEG XL says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JxlDetail {
  /// The codestream is in a file of boxes and does not stand alone.
  pub container: bool,
  pub bits_per_sample: u8,
  /// Samples are floating-point numbers.
  pub float_samples: bool,
  /// The picture is kept in the format's own color space, as nearly every file that lost
  /// something is and a file that lost nothing is not.
  pub xyb: bool,
  /// How many times an animation plays, where 0 is without end.
  pub loops: Option<u32>,
  /// The file holds what it takes to give back, byte for byte, the JPEG it was made from.
  pub jpeg_reconstruction: bool,
}

/// No more of a codestream than this is read for its header, no more boxes than this are
/// walked, and no box of what a camera wrote down longer than this is read.
const HEADER_BYTES: usize = 4096;
const MOST_BOXES: usize = 1000;
const LONGEST_BOX: u64 = 4 << 20;

/// The header's bits, read the way they are packed: the first in the lowest place of a byte.
/// Every read is `None` once the bytes run out.
struct Bits<'a> {
  bytes: &'a [u8],
  at: usize,
}

/// The four ways one number can be written: what each way starts from, and how many bits
/// are added to that.
type Ways = [(u32, usize); 4];

/// A side of the picture.
const SIDE: Ways = [(1, 9), (1, 13), (1, 18), (1, 30)];
/// A number that names one of a list.
const NAMED: Ways = [(0, 0), (1, 0), (2, 4), (18, 6)];
/// One of the two numbers that place a color.
const PLACE: Ways = [(0, 19), (524_288, 19), (1_048_576, 20), (2_097_152, 21)];

impl Bits<'_> {
  fn take(&mut self, count: usize) -> Option<u32> {
    let mut value = 0u32;
    for place in 0..count {
      let byte = *self.bytes.get(self.at / 8)?;
      value |= u32::from((byte >> (self.at % 8)) & 1) << place;
      self.at += 1;
    }
    Some(value)
  }

  fn flag(&mut self) -> Option<bool> {
    Some(self.take(1)? == 1)
  }

  /// A number written in one of four ways, which two bits choose.
  fn number(&mut self, ways: Ways) -> Option<u32> {
    let (from, count) = ways[usize::try_from(self.take(2)?).ok()?];
    from.checked_add(self.take(count)?)
  }

  /// A size: sides that are multiples of eight in five bits each, or written in full; and
  /// the width either on its own or as the height in one of seven fixed shapes.
  fn size(&mut self) -> Option<(u32, u32)> {
    let small = self.flag()?;
    let side = |bits: &mut Self| {
      if small {
        Some((bits.take(5)? + 1) * 8)
      } else {
        bits.number(SIDE)
      }
    };
    let height = side(self)?;
    let (over, under) = match self.take(3)? {
      0 => return Some((side(self)?, height)),
      1 => (1, 1),
      2 => (12, 10),
      3 => (4, 3),
      4 => (3, 2),
      5 => (16, 9),
      6 => (5, 4),
      _ => (2, 1),
    };
    let width = u64::from(height) * over / under;
    Some((u32::try_from(width).ok()?, height))
  }

  /// The depth of a sample, and whether samples are floating-point numbers.
  fn depth(&mut self) -> Option<(u32, bool)> {
    let float = self.flag()?;
    if !float {
      return Some((self.number([(8, 0), (10, 0), (12, 0), (1, 6)])?, false));
    }
    let bits = self.number([(32, 0), (16, 0), (24, 0), (1, 6)])?;
    // How many of them are the exponent.
    self.take(4)?;
    Some((bits, true))
  }
}

/// How the header says color is kept.
enum Color {
  /// As usual: sRGB.
  Usual,
  /// By a profile, which is packed in a way that is not read here.
  Profile,
  Gray,
  /// Red, green and blue, with the format's numbers for which white and which three colors.
  Rgb {
    white: u32,
    primaries: u32,
  },
  /// The format's own space, or one it does not name.
  Other,
}

/// What the packed header says.
struct Header {
  width: u32,
  height: u32,
  orientation: u32,
  bits: u32,
  float: bool,
  alpha: bool,
  /// A channel of black beyond the color's, which is how the format keeps CMYK.
  black: bool,
  xyb: bool,
  loops: Option<u32>,
  color: Color,
}

/// The kinds of channel beyond the color's that are told apart here.
const ALPHA: u32 = 0;
const SPOT_COLOR: u32 = 2;
const BLACK: u32 = 4;
const FILTER_MOSAIC: u32 = 5;

/// The header: the size, and then the description of the picture as far as how its color is
/// kept. Every field has a usual value, and one bit says they all have it.
fn header(bits: &mut Bits<'_>) -> Option<Header> {
  let (width, height) = bits.size()?;
  let mut header = Header {
    width,
    height,
    orientation: 1,
    bits: 8,
    float: false,
    alpha: false,
    black: false,
    xyb: true,
    loops: None,
    color: Color::Usual,
  };
  if bits.flag()? {
    return Some(header);
  }
  // The rarer fields: how the picture is turned, a size to show it at, a smaller copy of
  // it, and an animation.
  if bits.flag()? {
    header.orientation = bits.take(3)? + 1;
    if bits.flag()? {
      bits.size()?;
    }
    if bits.flag()? {
      // A smaller copy, whose sides are written in ways of their own.
      let eighths = bits.flag()?;
      let ways = if eighths {
        [(16, 0), (32, 0), (1, 5), (33, 9)]
      } else {
        [(1, 6), (65, 8), (321, 10), (1345, 12)]
      };
      bits.number(ways)?;
      if bits.take(3)? == 0 {
        bits.number(ways)?;
      }
    }
    if bits.flag()? {
      // Ticks to the second, as one number over another; then how often it plays.
      bits.number([(100, 0), (1000, 0), (1, 10), (1, 30)])?;
      bits.number([(1, 0), (1001, 0), (1, 8), (1, 10)])?;
      header.loops = Some(bits.number([(0, 0), (0, 3), (0, 16), (0, 32)])?);
      bits.flag()?;
    }
  }
  (header.bits, header.float) = bits.depth()?;
  bits.flag()?;
  // The channels beyond the color's.
  for _ in 0..bits.number([(0, 0), (1, 0), (2, 4), (1, 12)])? {
    // As such a channel usually is: alpha, in eight bits.
    if bits.flag()? {
      header.alpha = true;
      continue;
    }
    let kind = bits.number(NAMED)?;
    bits.depth()?;
    // How far it is shrunk, and then its name.
    bits.number([(0, 0), (3, 0), (4, 0), (1, 3)])?;
    for _ in 0..bits.number([(0, 0), (0, 4), (16, 5), (48, 10)])? {
      bits.take(8)?;
    }
    match kind {
      ALPHA => {
        header.alpha = true;
        bits.flag()?;
      }
      SPOT_COLOR => {
        bits.take(32)?;
        bits.take(32)?;
      }
      BLACK => header.black = true,
      FILTER_MOSAIC => {
        bits.number([(1, 0), (0, 2), (3, 4), (19, 8)])?;
      }
      _ => {}
    }
  }
  header.xyb = bits.flag()?;
  if bits.flag()? {
    return Some(header);
  }
  // How color is kept: by a profile, or by naming a space, its white and its three colors.
  let profile = bits.flag()?;
  let space = bits.number(NAMED)?;
  header.color = match (profile, space) {
    (true, _) => Color::Profile,
    (false, 1) => Color::Gray,
    (false, 0) => {
      let white = bits.number(NAMED)?;
      // A white of its own is placed by two numbers.
      if white == CUSTOM {
        bits.number(PLACE)?;
        bits.number(PLACE)?;
      }
      Color::Rgb {
        white,
        primaries: bits.number(NAMED)?,
      }
    }
    _ => Color::Other,
  };
  Some(header)
}

/// The format's numbers for the white of daylight, for a white or colors of the file's own,
/// and for the three colors of sRGB, of Rec. 2020 and of P3.
const DAYLIGHT: u32 = 1;
const CUSTOM: u32 = 2;
const SRGB_COLORS: u32 = 1;
const REC2020_COLORS: u32 = 9;
const P3_COLORS: u32 = 11;

/// As much of the `length` bytes at `at` as there is, and the range that was not given when
/// some of it was not.
fn given<'s>(source: &'s Source<'_>, at: u64, length: usize) -> (&'s [u8], Option<ByteRange>) {
  // The file may end before that many.
  let left = source
    .size()
    .and_then(|size| usize::try_from(size.saturating_sub(at)).ok());
  let length = left.map_or(length, |left| left.min(length));
  match source.read(at, length) {
    Ok(bytes) => (bytes, None),
    Err(Missing::NotGiven(range)) => {
      let have = usize::try_from(range.offset.saturating_sub(at)).unwrap_or(0);
      (source.read(at, have).unwrap_or_default(), Some(range))
    }
    Err(Missing::PastEnd) => (&[], None),
  }
}

/// What a file of boxes holds beside the codestream.
#[derive(Default)]
struct Carried {
  exif: Option<exif::Exif>,
  xmp: Option<xmp::Xmp>,
  jpeg_reconstruction: bool,
  /// EXIF or XMP is there, in a box that is compressed with Brotli.
  compressed: bool,
}

/// A codestream's start, the range of it that was not given when some was not, and what the
/// file carries beside it.
struct Opening {
  codestream: Vec<u8>,
  lacking: Option<ByteRange>,
  carried: Carried,
}

/// The start of the codestream in a file of boxes, which keeps it whole in one box or in
/// pieces in several, and what the file carries beside it. What a camera wrote down may stand
/// before the codestream, between its pieces or after it, so every box is looked at, and one
/// that was not given is asked for a block at a time.
fn boxed(source: &Source<'_>, reading: &mut Reading) -> Opening {
  let (mut codestream, mut carried, mut lacking) = (Vec::new(), Carried::default(), None);
  let mut at = 0u64;
  for _ in 0..MOST_BOXES {
    let found = match isobmff::box_at(source, at) {
      Ok(Some(found)) => found,
      Ok(None) => {
        reading.warn("a box states a length shorter than its own header");
        break;
      }
      Err(Missing::PastEnd) => break,
      Err(Missing::NotGiven(range)) => {
        reading.needs.get_or_insert(source.block(range));
        break;
      }
    };
    let wanted = usize::try_from(found.payload).unwrap_or(usize::MAX);
    match &found.kind {
      b"jxlc" | b"jxlp" if codestream.len() < HEADER_BYTES && lacking.is_none() => {
        // A piece opens with four bytes that give its place in the order.
        let piece = &found.kind == b"jxlp";
        let wanted = wanted
          .saturating_sub(if piece { 4 } else { 0 })
          .min(HEADER_BYTES - codestream.len());
        // Once some of the start is missing, a later piece cannot be joined to it.
        let (start, missing) = given(source, found.payload_at + 4 * u64::from(piece), wanted);
        codestream.extend_from_slice(start);
        lacking = missing;
      }
      b"jbrd" => carried.jpeg_reconstruction = true,
      // A box that holds another one compressed opens with the name of the one it holds.
      b"brob" => {
        if let Ok(b"Exif" | b"xml ") = source.read(found.payload_at, 4) {
          carried.compressed = true;
        }
      }
      b"Exif" | b"xml " if found.payload <= LONGEST_BOX => {
        match (source.read(found.payload_at, wanted), &found.kind) {
          // Four bytes that say how far in the block starts, and then the block.
          (Ok([o0, o1, o2, o3, block @ ..]), b"Exif") => {
            let skipped = usize::try_from(u32::from_be_bytes([*o0, *o1, *o2, *o3])).unwrap_or(0);
            carried.exif = block.get(skipped..).and_then(exif::from_bytes);
          }
          (Ok(packet), b"xml ") => carried.xmp = xmp::parse(packet),
          (Err(Missing::NotGiven(range)), _) => {
            reading.needs.get_or_insert(source.block(range));
          }
          _ => {}
        }
      }
      _ => {}
    }
    // A box that runs to the end of a file whose size is not known has nothing after it.
    if found.next == u64::MAX {
      break;
    }
    at = found.next;
  }
  Opening {
    codestream,
    lacking,
    carried,
  }
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let container = !matches!(source.array::<2>(0), Ok([0xFF, 0x0A]));
  let Opening {
    codestream,
    lacking,
    carried,
  } = if container {
    boxed(source, reading)
  } else {
    let (start, lacking) = given(source, 0, HEADER_BYTES);
    Opening {
      codestream: start.to_vec(),
      lacking,
      carried: Carried::default(),
    }
  };
  let packed = codestream.strip_prefix(&[0xFF, 0x0A]).and_then(|written| {
    header(&mut Bits {
      bytes: written,
      at: 0,
    })
  });
  let Some(header) = packed else {
    // More of the codestream's start is wanted only when what was given of it did not hold
    // the header.
    if let Some(range) = lacking {
      reading.needs.get_or_insert(range);
    }
    if reading.needs.is_none() && reading.warnings.is_empty() {
      reading.warn(if codestream.len() >= HEADER_BYTES {
        "the header is longer than is read"
      } else {
        "the file ends inside its header"
      });
    }
    return None;
  };

  let (space, embedded) = match header.color {
    Color::Usual => (ColorSpace::Srgb, false),
    Color::Profile => (ColorSpace::Unknown, true),
    Color::Gray => (ColorSpace::Gray, false),
    Color::Rgb {
      white: DAYLIGHT,
      primaries,
    } => (
      match primaries {
        SRGB_COLORS => ColorSpace::Srgb,
        REC2020_COLORS => ColorSpace::Rec2020,
        P3_COLORS => ColorSpace::DisplayP3,
        _ => ColorSpace::Unknown,
      },
      false,
    ),
    Color::Rgb { .. } | Color::Other => (ColorSpace::Unknown, false),
  };
  let (color_model, color_space) = match (&header.color, header.alpha) {
    _ if header.black => (ColorModel::Cmyk, ColorSpace::Cmyk),
    (Color::Gray, true) => (ColorModel::GrayscaleAlpha, space),
    (Color::Gray, false) => (ColorModel::Grayscale, space),
    (_, true) => (ColorModel::Rgba, space),
    (_, false) => (ColorModel::Rgb, space),
  };
  if header.loops.is_some() {
    reading.warn("the frames of an animated JPEG XL are not counted");
  }
  if carried.compressed {
    reading.warn("what a camera wrote down is compressed in a way that is not read here");
  }
  let detail = ImageDetail::Jxl(JxlDetail {
    container,
    bits_per_sample: u8::try_from(header.bits).unwrap_or(u8::MAX),
    float_samples: header.float,
    xyb: header.xyb,
    loops: header.loops,
    jpeg_reconstruction: carried.jpeg_reconstruction,
  });
  let mut facts = ImageFacts {
    animated: header.loops.is_some(),
    has_alpha: Some(header.alpha),
    color_model,
    color_space,
    icc_profile: embedded.then(|| "embedded".to_string()),
    xmp: carried.xmp,
    ..ImageFacts::sized(header.width, header.height, detail)
  };
  if let Some(exif) = carried.exif {
    facts.take_exif(exif);
  }
  // How the picture is turned is the codestream's to say, whatever a camera wrote down.
  facts.orientation = u8::try_from(header.orientation).ok();
  Some(facts)
}

#[cfg(test)]
mod tests;
