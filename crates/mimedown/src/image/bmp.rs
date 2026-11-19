//! BMP: a file header of 14 bytes and then a second header, which grew from 12 bytes to 124
//! over the years. Each version begins with its own size, which says what it holds.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::source::Source;

/// What only a bitmap says.
#[derive(Debug, Clone, PartialEq)]
pub struct BmpDetail {
  /// The size of the second header, which is its version: 12, 40, 52, 56, 108 or 124.
  pub header_bytes: u32,
  pub bits_per_pixel: u16,
  /// How the pixels are kept: `rgb`, `rle8`, `rle4`, `bitfields`, `jpeg`, `png`.
  pub compression: String,
  /// The rows run from the top down. A bitmap's rows usually run from the bottom up; this is
  /// the order they are stored in and not a turn of the picture.
  pub top_down: bool,
  /// Pixels to the inch across and down, when the file states a density.
  pub dpi: Option<(f64, f64)>,
}

fn le32(bytes: &[u8], at: usize) -> u32 {
  bytes
    .get(at..at + 4)
    .and_then(|field| field.first_chunk::<4>())
    .map_or(0, |field| u32::from_le_bytes(*field))
}

fn le16(bytes: &[u8], at: usize) -> u16 {
  bytes
    .get(at..at + 2)
    .and_then(|field| field.first_chunk::<2>())
    .map_or(0, |field| u16::from_le_bytes(*field))
}

fn compression_name(compression: u32) -> String {
  match compression {
    0 => "rgb".to_string(),
    1 => "rle8".to_string(),
    2 => "rle4".to_string(),
    3 | 6 => "bitfields".to_string(),
    4 => "jpeg".to_string(),
    5 => "png".to_string(),
    other => format!("{other}"),
  }
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let [h0, h1, h2, h3] = reading.take(source.array::<4>(14), "inside its header")?;
  let header_bytes = u32::from_le_bytes([h0, h1, h2, h3]);
  let wanted = usize::try_from(header_bytes.min(124)).unwrap_or(0);
  // The second header, from its own size onward. Fields a version does not have read as zero.
  let header = reading.take(source.read(14, wanted), "inside its header")?;

  // The first version, from OS/2, keeps the size in two bytes each and stops after the depth.
  if header_bytes == 12 {
    let bits_per_pixel = le16(header, 10);
    let detail = ImageDetail::Bmp(BmpDetail {
      header_bytes,
      bits_per_pixel,
      compression: compression_name(0),
      top_down: false,
      dpi: None,
    });
    return Some(ImageFacts {
      has_alpha: Some(false),
      color_model: model(bits_per_pixel, false),
      ..ImageFacts::sized(
        u32::from(le16(header, 4)),
        u32::from(le16(header, 6)),
        detail,
      )
    });
  }

  let width = le32(header, 4).cast_signed();
  let height = le32(header, 8).cast_signed();
  let bits_per_pixel = le16(header, 14);
  let compression = le32(header, 16);
  // A mask says which bits of a pixel are its alpha. It is in the header from 56 bytes on, and
  // follows a header of 40 when the pixels are kept as bit fields with alpha.
  let alpha_mask = match header_bytes {
    56.. => le32(header, 52),
    40 if compression == 6 => reading
      .take(source.array::<4>(14 + 40 + 12), "inside its masks")
      .map_or(0, u32::from_le_bytes),
    _ => 0,
  };
  let has_alpha = alpha_mask != 0;
  // From 108 bytes on the header names its color space: `sRGB`, or `Win ` for the system's.
  let color_space = match (header_bytes >= 108).then(|| le32(header, 56)) {
    Some(0x7352_4742 | 0x5769_6E20) => ColorSpace::Srgb,
    _ => ColorSpace::Unknown,
  };
  // Pixels to the meter.
  let (across, down) = (le32(header, 24), le32(header, 28));
  let dpi =
    (across != 0 && down != 0).then(|| (f64::from(across) * 0.0254, f64::from(down) * 0.0254));

  let detail = ImageDetail::Bmp(BmpDetail {
    header_bytes,
    bits_per_pixel,
    compression: compression_name(compression),
    top_down: height < 0,
    dpi,
  });
  Some(ImageFacts {
    has_alpha: Some(has_alpha),
    color_model: model(bits_per_pixel, has_alpha),
    color_space,
    ..ImageFacts::sized(width.unsigned_abs(), height.unsigned_abs(), detail)
  })
}

fn model(bits_per_pixel: u16, has_alpha: bool) -> ColorModel {
  match bits_per_pixel {
    1..=8 => ColorModel::Indexed,
    _ if has_alpha => ColorModel::Rgba,
    _ => ColorModel::Rgb,
  }
}

#[cfg(test)]
mod tests;
