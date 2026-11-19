//! ICO and CUR: a count, and then one entry for each picture the file holds, which is the same
//! icon at several sizes. Each picture is a bitmap without its file header, or a whole PNG.

use super::{ColorModel, ImageDetail, ImageFacts, Reading};
use crate::source::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcoKind {
  Icon,
  Cursor,
}

/// One picture of an icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcoEntry {
  pub width: u32,
  pub height: u32,
  /// Bits in a pixel, when the entry says. A cursor's entry gives its hotspot in that place.
  pub bits_per_pixel: Option<u16>,
  /// The picture is a whole PNG, not a bitmap.
  pub png: bool,
  /// Where a cursor points, from its top left corner.
  pub hotspot: Option<(u16, u16)>,
}

/// What only an icon says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcoDetail {
  pub kind: IcoKind,
  pub entries: Vec<IcoEntry>,
}

/// No more entries than this are listed.
const MOST_ENTRIES: u16 = 1024;

const PNG_SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1A\n";

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let [_, _, kind, _, c0, c1] = reading.take(source.array::<6>(0), "inside its header")?;
  let kind = if kind == 2 {
    IcoKind::Cursor
  } else {
    IcoKind::Icon
  };
  let count = u16::from_le_bytes([c0, c1]);
  if count > MOST_ENTRIES {
    reading.warn(format!(
      "{count} pictures are listed; the first {MOST_ENTRIES} were read"
    ));
  }

  let mut entries = Vec::new();
  for index in 0..count.min(MOST_ENTRIES) {
    let at = 6 + 16 * u64::from(index);
    let Some(entry) = reading.take(source.array::<16>(at), "inside its list of pictures") else {
      break;
    };
    let [
      width,
      height,
      _,
      _,
      a0,
      a1,
      b0,
      b1,
      _,
      _,
      _,
      _,
      o0,
      o1,
      o2,
      o3,
    ] = entry;
    // A size of zero stands for 256, the largest a byte cannot hold.
    let side = |byte: u8| if byte == 0 { 256 } else { u32::from(byte) };
    let (first, second) = (u16::from_le_bytes([a0, a1]), u16::from_le_bytes([b0, b1]));
    let picture_at = u64::from(u32::from_le_bytes([o0, o1, o2, o3]));
    // A PNG states its own size, which may be larger than an entry can say. A picture that is
    // not among the bytes given is taken for a bitmap, as most are.
    let png = source
      .array::<24>(picture_at)
      .ok()
      .filter(|start| start[..8] == PNG_SIGNATURE);
    let (width, height) = match png {
      Some(start) => (
        u32::from_be_bytes([start[16], start[17], start[18], start[19]]),
        u32::from_be_bytes([start[20], start[21], start[22], start[23]]),
      ),
      None => (side(width), side(height)),
    };
    entries.push(match kind {
      IcoKind::Icon => IcoEntry {
        width,
        height,
        bits_per_pixel: (second != 0).then_some(second),
        png: png.is_some(),
        hotspot: None,
      },
      IcoKind::Cursor => IcoEntry {
        width,
        height,
        bits_per_pixel: None,
        png: png.is_some(),
        hotspot: Some((first, second)),
      },
    });
  }

  // The icon's size is its largest picture's.
  let largest = entries
    .iter()
    .max_by_key(|entry| u64::from(entry.width) * u64::from(entry.height))?;
  let color_model = match largest.bits_per_pixel {
    Some(1..=8) => ColorModel::Indexed,
    Some(24) => ColorModel::Rgb,
    Some(32) => ColorModel::Rgba,
    _ if largest.png => ColorModel::Rgba,
    _ => ColorModel::Unknown,
  };
  let (width, height) = (largest.width, largest.height);
  let frames = u32::try_from(entries.len()).unwrap_or(u32::MAX);
  Some(ImageFacts {
    frames,
    // Every picture of an icon carries a mask of what is see-through.
    has_alpha: Some(true),
    color_model,
    ..ImageFacts::sized(width, height, ImageDetail::Ico(IcoDetail { kind, entries }))
  })
}

#[cfg(test)]
mod tests;
