//! JPEG XR: a directory of tagged values, as a TIFF has, and the picture it points at.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::ifd::{Directory, Tiff};
use crate::container::{exif, icc, xmp};
use crate::source::{Missing, Source};

/// What only a JPEG XR says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JxrDetail {
  /// The version its header states: 1, or 0 for a file from when the format was HD Photo.
  pub version: u8,
  /// The last byte of the identifier of its pixel format, which is what tells the formats
  /// apart: 0x0C is 24 bits of blue, green and red.
  pub pixel_format: Option<u8>,
  /// Bits to a pixel, for a pixel format that is known.
  pub bits_per_pixel: Option<u16>,
}

const PIXEL_FORMAT: u16 = 0xBC01;
const TRANSFORMATION: u16 = 0xBC02;
const WIDTH: u16 = 0xBC80;
const HEIGHT: u16 = 0xBC81;
const ALPHA_OFFSET: u16 = 0xBCC2;
const PROFILE: u16 = 0x8773;
const XMP: u16 = 0x02BC;

/// The fifteen bytes every pixel format's identifier opens with. The sixteenth says which
/// format it is.
const SHARED: [u8; 15] = [
  0x24, 0xC3, 0xDD, 0x6F, 0x03, 0x4E, 0xFE, 0x4B, 0xB1, 0x85, 0x3D, 0x77, 0x76, 0x8D, 0xC9,
];

/// What a pixel format keeps: how its color is laid out, whether it has alpha, how many bits
/// a pixel takes, and whether its samples are plain whole numbers and not the fixed-point,
/// half or floating ones kept for a wider range.
struct Kept {
  color_model: ColorModel,
  alpha: bool,
  bits: u16,
  plain: bool,
}

fn pixel_format(which: u8) -> Option<Kept> {
  use ColorModel::{Cmyk, Grayscale, Rgb, Rgba, Unknown};
  let kept = |color_model, alpha, bits, plain| Kept {
    color_model,
    alpha,
    bits,
    plain,
  };
  // How far a format is into a run of them that differ by one channel each.
  let into = |start: u8| u16::from(which - start);
  Some(match which {
    0x05 => kept(Grayscale, false, 1, true),
    0x08 => kept(Grayscale, false, 8, true),
    0x0B => kept(Grayscale, false, 16, true),
    0x13 | 0x3E => kept(Grayscale, false, 16, false),
    0x11 | 0x3F => kept(Grayscale, false, 32, false),
    0x09 | 0x0A => kept(Rgb, false, 16, true),
    0x0C | 0x0D => kept(Rgb, false, 24, true),
    0x0E | 0x14 => kept(Rgb, false, 32, true),
    0x3D => kept(Rgb, false, 32, false),
    0x15 => kept(Rgb, false, 48, true),
    0x12 | 0x3B => kept(Rgb, false, 48, false),
    0x18 => kept(Rgb, false, 96, false),
    0x1B => kept(Rgb, false, 128, false),
    0x0F | 0x10 => kept(Rgba, true, 32, true),
    0x16 | 0x17 => kept(Rgba, true, 64, true),
    0x1D | 0x3A => kept(Rgba, true, 64, false),
    0x19 | 0x1A | 0x1E => kept(Rgba, true, 128, false),
    0x1C => kept(Cmyk, false, 32, true),
    0x1F => kept(Cmyk, false, 64, true),
    0x2C => kept(Cmyk, true, 40, true),
    0x2D => kept(Cmyk, true, 80, true),
    // Channels that are not named colors, three to eight of them: of 8 bits and then of 16,
    // without alpha and then with it.
    0x20..=0x25 => kept(Unknown, false, 8 * (3 + into(0x20)), true),
    0x26..=0x2B => kept(Unknown, false, 16 * (3 + into(0x26)), true),
    0x2E..=0x33 => kept(Unknown, true, 8 * (4 + into(0x2E)), true),
    0x34..=0x39 => kept(Unknown, true, 16 * (4 + into(0x34)), true),
    _ => return None,
  })
}

/// How a picture is to be turned, as EXIF numbers it, from the format's own number for it.
fn turn(written: u64) -> Option<u8> {
  Some(match written {
    0 => 1,
    1 => 4,
    2 => 2,
    3 => 3,
    4 => 6,
    5 => 7,
    6 => 5,
    7 => 8,
    _ => return None,
  })
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  // Two letters, a byte that marks the format, its version, and where the directory is.
  let [_, _, _, version, first @ ..] = reading.take(source.array::<8>(0), "inside its header")?;
  let tiff = Tiff::at(source, true, u64::from(u32::from_le_bytes(first)));
  let directory = reading.take(tiff.directory(tiff.first), "before its directory")?;
  reading.take(
    described(&tiff, &directory, version),
    "inside a value of its directory",
  )
}

fn described(
  tiff: &Tiff<'_, '_>,
  directory: &Directory,
  version: u8,
) -> Result<ImageFacts, Missing> {
  let number = |tag: u16| match directory.entry(tag) {
    Some(entry) => tiff.number(entry),
    None => Ok(None),
  };
  let bytes = |tag: u16| match directory.entry(tag) {
    Some(entry) => tiff.bytes(entry).map(Some),
    None => Ok(None),
  };
  let side = |tag: u16| {
    number(tag).map(|value| {
      value
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
    })
  };
  let format = bytes(PIXEL_FORMAT)?.and_then(|identifier| match identifier.strip_prefix(&SHARED) {
    Some([which]) => Some(*which),
    _ => None,
  });
  let kept = format.and_then(pixel_format);
  // Alpha is either one of the format's own channels or a plane kept apart from the picture.
  let plane = number(ALPHA_OFFSET)?.is_some_and(|offset| offset != 0);
  let has_alpha = match &kept {
    Some(kept) => Some(kept.alpha || plane),
    None => plane.then_some(true),
  };
  let color_model = match kept.as_ref().map(|kept| kept.color_model) {
    Some(ColorModel::Rgb) if plane => ColorModel::Rgba,
    Some(ColorModel::Grayscale) if plane => ColorModel::GrayscaleAlpha,
    Some(color_model) => color_model,
    None => ColorModel::Unknown,
  };
  let plain = kept.as_ref().is_some_and(|kept| kept.plain);
  let color_space = match color_model {
    ColorModel::Grayscale | ColorModel::GrayscaleAlpha => ColorSpace::Gray,
    ColorModel::Cmyk => ColorSpace::Cmyk,
    // Samples kept for a wider range are not in the space a screen shows.
    ColorModel::Rgb | ColorModel::Rgba if plain => ColorSpace::Srgb,
    _ => ColorSpace::Unknown,
  };
  let detail = ImageDetail::Jxr(JxrDetail {
    version,
    pixel_format: format,
    bits_per_pixel: kept.as_ref().map(|kept| kept.bits),
  });
  let mut facts = ImageFacts {
    has_alpha,
    color_model,
    color_space,
    xmp: bytes(XMP)?.and_then(|packet| xmp::parse(&packet)),
    ..ImageFacts::sized(side(WIDTH)?, side(HEIGHT)?, detail)
  };
  if let Some(profile) = bytes(PROFILE)?.and_then(|profile| icc::parse(&profile)) {
    facts.take_profile(profile);
  }
  facts.take_exif(exif::read(tiff, directory)?);
  facts.orientation = number(TRANSFORMATION)?.and_then(turn);
  Ok(facts)
}

#[cfg(test)]
mod tests;
