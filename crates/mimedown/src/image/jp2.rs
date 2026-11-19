//! JPEG 2000: a file of boxes, one of which describes the picture and one of which is the
//! picture, or the picture's codestream with no boxes around it.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::isobmff::{self, boxes};
use crate::container::{icc, xmp};
use crate::source::{Missing, Source};

/// What only a JPEG 2000 says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jp2Detail {
  /// How many channels the picture has.
  pub components: u16,
  /// Bits in each sample. None when the channels differ.
  pub bits_per_sample: Option<u8>,
  /// The number the format gives the picture's color space: 16 sRGB, 17 gray, 18 sYCC.
  pub enumerated_color: Option<u32>,
  /// The file is the picture's codestream alone, with no boxes around it.
  pub codestream: bool,
}

/// No more boxes than this are walked, and no header box or packet longer than this is read.
const MOST_BOXES: usize = 1000;
const LONGEST_BOX: u64 = 4 << 20;

/// What marks a box of no fixed meaning as holding an XMP packet.
const XMP_BOX: [u8; 16] = [
  0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC,
];

/// What the picture's own size marker says.
struct Size {
  width: u32,
  height: u32,
  components: u16,
  /// The bits of a sample, when every channel has the same.
  bits: Option<u8>,
}

/// The size marker of the codestream that begins at `at`. It follows the two bytes that open
/// the stream: the area of the grid the picture ends at and the point it starts from, the
/// tiles, and then each channel's depth.
fn size_marker(source: &Source<'_>, at: u64) -> Result<Option<Size>, Missing> {
  let start = source.array::<42>(at)?;
  let [0xFF, 0x4F, 0xFF, 0x51, _, _, _, _, grid @ ..] = start else {
    return Ok(None);
  };
  let (numbers, _) = grid.as_chunks::<4>();
  let [across, down, from_left, from_top, ..] = numbers else {
    return Ok(None);
  };
  let side = |end: &[u8; 4], start: &[u8; 4]| {
    u32::from_be_bytes(*end).saturating_sub(u32::from_be_bytes(*start))
  };
  let components = u16::from_be_bytes([start[40], start[41]]);
  // Three bytes for each channel, the first of which is its depth less one.
  let channels = source.read(at + 42, 3 * usize::from(components))?;
  let (channels, _) = channels.as_chunks::<3>();
  let mut depths = channels.iter().map(|[depth, _, _]| (depth & 0x7F) + 1);
  let first = depths.next();
  let bits = first.filter(|first| depths.all(|depth| depth == *first));
  Ok(Some(Size {
    width: side(across, from_left),
    height: side(down, from_top),
    components,
    bits,
  }))
}

/// The facts of a picture known only by its size marker, which says nothing of its color.
fn from_marker(size: Size, codestream: bool) -> ImageFacts {
  let color_model = match size.components {
    1 => ColorModel::Grayscale,
    3 => ColorModel::Rgb,
    _ => ColorModel::Unknown,
  };
  let detail = ImageDetail::Jp2(Jp2Detail {
    components: size.components,
    bits_per_sample: size.bits,
    enumerated_color: None,
    codestream,
  });
  ImageFacts {
    color_model,
    ..ImageFacts::sized(size.width, size.height, detail)
  }
}

/// What the header box says: its own boxes give the size, how color is kept, what each
/// channel is, and whether the samples point into a palette.
fn from_header(header: &[u8]) -> Option<ImageFacts> {
  let [h0, h1, h2, h3, w0, w1, w2, w3, c0, c1, depth, ..] =
    *boxes(header).find(|(kind, _)| kind == b"ihdr")?.1
  else {
    return None;
  };
  let components = u16::from_be_bytes([c0, c1]);
  // The first box that says how color is kept is the one to go by: a number for a space the
  // format names, or a profile.
  let color = boxes(header)
    .find(|(kind, _)| kind == b"colr")
    .map(|(_, color)| color);
  let (enumerated, profile) = match color {
    Some([1, _, _, n0, n1, n2, n3, ..]) => (Some(u32::from_be_bytes([*n0, *n1, *n2, *n3])), None),
    Some([2, _, _, profile @ ..]) => (None, icc::parse(profile)),
    _ => (None, None),
  };
  // A channel is color, or it is how see-through the color is, said in either of two ways.
  let alphas = boxes(header)
    .find(|(kind, _)| kind == b"cdef")
    .map_or(0, |(_, defined)| {
      // Each is a channel's number, what it is, and the color it belongs to.
      let (entries, _) = defined.get(2..).unwrap_or_default().as_chunks::<6>();
      entries
        .iter()
        .filter(|[_, _, kind @ .., _, _]| matches!(kind, [0, 1 | 2]))
        .count()
    });
  let has_alpha = alphas > 0;
  let palette = boxes(header).any(|(kind, _)| &kind == b"pclr");
  let colors = usize::from(components).saturating_sub(alphas);
  let (color_model, color_space) = match enumerated {
    Some(16 | 20) => (ColorModel::Rgb, ColorSpace::Srgb),
    Some(17) => (ColorModel::Grayscale, ColorSpace::Gray),
    Some(18 | 24) => (ColorModel::Ycbcr, ColorSpace::Srgb),
    Some(12) => (ColorModel::Cmyk, ColorSpace::Cmyk),
    Some(14) => (ColorModel::Lab, ColorSpace::Lab),
    Some(21) => (ColorModel::Rgb, ColorSpace::ProphotoRgb),
    Some(_) => (ColorModel::Unknown, ColorSpace::Unknown),
    // A profile says which colors, and the count of channels how they are laid out.
    None if profile.is_some() && colors == 1 => (ColorModel::Grayscale, ColorSpace::Gray),
    None if profile.is_some() && colors == 3 => (ColorModel::Rgb, ColorSpace::Unknown),
    None => (ColorModel::Unknown, ColorSpace::Unknown),
  };
  let color_model = match color_model {
    _ if palette => ColorModel::Indexed,
    ColorModel::Rgb if has_alpha => ColorModel::Rgba,
    ColorModel::Grayscale if has_alpha => ColorModel::GrayscaleAlpha,
    color_model => color_model,
  };
  let detail = ImageDetail::Jp2(Jp2Detail {
    components,
    // All ones in place of a depth says the channels differ.
    bits_per_sample: (depth != 255).then(|| (depth & 0x7F) + 1),
    enumerated_color: enumerated,
    codestream: false,
  });
  let mut facts = ImageFacts {
    has_alpha: Some(has_alpha),
    color_model,
    color_space,
    ..ImageFacts::sized(
      u32::from_be_bytes([w0, w1, w2, w3]),
      u32::from_be_bytes([h0, h1, h2, h3]),
      detail,
    )
  };
  if let Some(profile) = profile {
    facts.take_profile(profile);
  }
  Some(facts)
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  // The picture's codestream alone.
  if matches!(source.array::<2>(0), Ok([0xFF, 0x4F])) {
    let size = reading.take(size_marker(source, 0), "inside its size marker")??;
    return Some(from_marker(size, true));
  }
  let (mut header, mut packet, mut picture_at) = (None, None, None);
  let mut at = 0u64;
  for _ in 0..MOST_BOXES {
    let found = match isobmff::box_at(source, at) {
      Ok(Some(found)) => found,
      Ok(None) => {
        reading.warn("a box states a length shorter than its own header");
        break;
      }
      Err(Missing::PastEnd) => break,
      // A box's header is followed by what it holds and then by the next, so a block is
      // asked for.
      Err(Missing::NotGiven(range)) => {
        reading.needs.get_or_insert(source.block(range));
        break;
      }
    };
    let wanted = usize::try_from(found.payload).unwrap_or(usize::MAX);
    match &found.kind {
      b"jp2h" if found.payload <= LONGEST_BOX => {
        let payload = reading.take(
          source.read(found.payload_at, wanted),
          "inside its header box",
        )?;
        header = Some(payload.to_vec());
      }
      // A box of no fixed meaning opens with sixteen bytes that say what it holds.
      b"uuid" if found.payload <= LONGEST_BOX => {
        let holds = reading.take(source.array::<16>(found.payload_at), "inside a box");
        if holds == Some(XMP_BOX)
          && let Some(payload) = reading.take(source.read(found.payload_at, wanted), "inside a box")
        {
          packet = payload.get(16..).and_then(xmp::parse);
        }
      }
      b"jp2c" => {
        picture_at = Some(found.payload_at);
        break;
      }
      _ => {}
    }
    at = found.next;
  }
  let facts = match (header.as_deref().and_then(from_header), picture_at) {
    (Some(facts), _) => Some(facts),
    // With no header box, the picture's own marker has the size.
    (None, Some(picture_at)) => reading
      .take(size_marker(source, picture_at), "inside its size marker")?
      .map(|size| from_marker(size, false)),
    (None, None) => None,
  };
  let Some(mut facts) = facts else {
    if reading.needs.is_none() && reading.warnings.is_empty() {
      reading.warn("the file has no header box and no picture");
    }
    return None;
  };
  facts.xmp = packet;
  Some(facts)
}

#[cfg(test)]
mod tests;
