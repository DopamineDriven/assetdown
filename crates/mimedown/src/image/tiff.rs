//! TIFF: directories of tagged values, one for each page. The first directory is wherever the
//! header says it is, which is as often the end of the file as the start.

use std::cell::Cell;
use std::collections::HashSet;
use std::iter::once;

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::ifd::{Directory, Entry, Tiff, if_there};
use crate::container::{exif, icc, xmp};
use crate::media_type::MediaType;
use crate::source::{Missing, Source};

/// What only a TIFF says.
#[derive(Debug, Clone, PartialEq)]
pub struct TiffDetail {
  /// BigTIFF, which a file over four gigabytes has to be.
  pub big: bool,
  pub little_endian: bool,
  /// How many directories the file chains together: its pages.
  pub pages: u32,
  /// The format's number for how the pixels are packed: 1 none, 5 LZW, 7 JPEG, 8 deflate.
  pub compression: Option<u16>,
  pub bits_per_sample: Vec<u16>,
  pub samples_per_pixel: u16,
  /// The format's number for what the samples mean: 0 and 1 gray, 2 RGB, 3 a palette, 5 CMYK.
  pub photometric: Option<u16>,
  /// Pixels to the inch across and down, when the file states a density in a real unit.
  pub dpi: Option<(f64, f64)>,
  pub software: Option<String>,
  /// When the file was written, as it is written.
  pub date_time: Option<String>,
}

/// No more pages than this are counted.
const MOST_PAGES: u32 = 100_000;

/// No more of the directories a first directory points at than this are read.
const MOST_SUB_DIRECTORIES: usize = 16;

const SUBFILE_TYPE: u16 = 254;
const WIDTH: u16 = 256;
const HEIGHT: u16 = 257;
const PHOTOMETRIC: u16 = 262;
const MAKE: u16 = 271;
const SUB_DIRECTORIES: u16 = 330;
const DNG_VERSION: u16 = 0xC612;

/// What the samples under a sensor's color filter are: one to the pixel, in a mosaic. Only
/// what a camera records is kept this way.
const MOSAIC: u64 = 32803;

/// The directories the first directory points at. A camera's file keeps what its sensor
/// recorded in one of these.
fn sub_directories(tiff: &Tiff<'_, '_>, first: &Directory) -> Result<Vec<Directory>, Missing> {
  let offsets = match first.entry(SUB_DIRECTORIES) {
    Some(entry) => tiff.numbers(entry)?,
    None => Vec::new(),
  };
  offsets
    .into_iter()
    .take(MOST_SUB_DIRECTORIES)
    .map(|offset| tiff.directory(offset))
    .collect()
}

/// What kind of TIFF the file is: one of the forms a camera records in, or a TIFF and no more.
pub(crate) fn kind(source: &Source<'_>) -> Result<MediaType, Missing> {
  let Some(tiff) = Tiff::open(source)? else {
    return Ok(MediaType::Tiff);
  };
  // Canon writes its two letters and a version after the header.
  match source.read(8, 3) {
    Ok(b"CR\x02") => return Ok(MediaType::Cr2),
    Ok(_) | Err(Missing::PastEnd) => {}
    Err(missing) => return Err(missing),
  }
  let first = tiff.directory(tiff.first)?;
  if first.entry(DNG_VERSION).is_some() {
    return Ok(MediaType::Dng);
  }
  let subs = sub_directories(&tiff, &first)?;
  let mut mosaic = false;
  for directory in once(&first).chain(&subs) {
    if let Some(entry) = directory.entry(PHOTOMETRIC)
      && tiff.number(entry)? == Some(MOSAIC)
    {
      mosaic = true;
      break;
    }
  }
  if !mosaic {
    return Ok(MediaType::Tiff);
  }
  let make = match first.entry(MAKE) {
    Some(entry) => tiff.text(entry)?,
    None => None,
  };
  let make = make.unwrap_or_default().to_ascii_uppercase();
  Ok(if make.starts_with("NIKON") {
    MediaType::Nef
  } else if make.starts_with("SONY") {
    MediaType::Arw
  } else {
    MediaType::Raw
  })
}

/// The directory that describes the picture itself. A camera's file opens with a small copy
/// for display and keeps the sensor's recording in a directory the first one points at, marked
/// as the full picture. Of the directories so marked it is the largest; a file that marks none
/// has its largest taken.
fn full_picture<'d>(
  tiff: &Tiff<'_, '_>,
  first: &'d Directory,
  subs: &'d [Directory],
) -> Result<&'d Directory, Missing> {
  let mut best: Option<(bool, u64, &Directory)> = None;
  for directory in once(first).chain(subs) {
    let number = |tag: u16| match directory.entry(tag) {
      Some(entry) => tiff.number(entry),
      None => Ok(None),
    };
    // The lowest bit of the type is set for a smaller copy of another picture in the file.
    let full = number(SUBFILE_TYPE)?.unwrap_or(0) & 1 == 0;
    let area = number(WIDTH)?
      .unwrap_or(0)
      .saturating_mul(number(HEIGHT)?.unwrap_or(0));
    if best.is_none_or(|(best_full, best_area, _)| (full, area) > (best_full, best_area)) {
      best = Some((full, area, directory));
    }
  }
  Ok(best.map_or(first, |(_, _, directory)| directory))
}

/// What a TIFF says of its picture, and how many directories it holds. A file a camera recorded
/// is read the same way: its other directories are smaller copies of the one picture.
pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let tiff = reading.take(Tiff::open(source), "inside its header")??;
  let first = reading.take(tiff.directory(tiff.first), "before its first directory")?;
  let subs = reading
    .take(
      sub_directories(&tiff, &first),
      "inside a directory its first one points at",
    )
    .unwrap_or_default();
  let picture = reading.take(
    full_picture(&tiff, &first, &subs),
    "inside a value of a directory",
  )?;
  let lost = Cell::new(false);
  let mut facts = reading.take(
    page(&tiff, picture, &first, &lost),
    "inside a value of its first directory",
  )?;
  if lost.get() {
    reading.warn("a value of its first directory lies past the end of the file and is left out");
  }

  // The other pages are counted by following each directory to the next.
  let mut pages = 1;
  let mut next = first.next;
  let mut counted = HashSet::from([tiff.first]);
  while next != 0 && pages < MOST_PAGES {
    if !counted.insert(next) {
      reading.warn(
        "its directories lead back to one already counted, where the count of its pages stops",
      );
      break;
    }
    match tiff.directory(next) {
      Ok(directory) => {
        pages += 1;
        next = directory.next;
      }
      Err(missing) => {
        reading.take(Err::<(), _>(missing), "before the last of its pages");
        break;
      }
    }
  }
  if let ImageDetail::Tiff(detail) = &mut facts.detail {
    detail.pages = pages;
  }
  Some(facts)
}

/// The facts of the page a directory describes. What a file says once for all its pictures
/// (who made it, when, its profile) is in the file's `first` directory when it is not in the
/// page's own.
fn page(
  tiff: &Tiff<'_, '_>,
  directory: &Directory,
  first: &Directory,
  lost: &Cell<bool>,
) -> Result<ImageFacts, Missing> {
  let numbers = |tag: u16| match directory.entry(tag) {
    Some(entry) => if_there(tiff.numbers(entry), lost),
    None => Ok(Vec::new()),
  };
  let number = |tag: u16| numbers(tag).map(|values| values.first().copied());
  let short = |tag: u16| number(tag).map(|value| value.and_then(|value| u16::try_from(value).ok()));
  let of_the_file =
    |tag: u16| -> Option<&Entry> { directory.entry(tag).or_else(|| first.entry(tag)) };
  let text = |tag: u16| match of_the_file(tag) {
    Some(entry) => if_there(tiff.text(entry), lost),
    None => Ok(None),
  };
  let bytes = |tag: u16| match of_the_file(tag) {
    Some(entry) => if_there(tiff.bytes(entry).map(Some), lost),
    None => Ok(None),
  };
  let fraction = |tag: u16| match directory.entry(tag) {
    Some(entry) => if_there(tiff.fractions(entry), lost).map(|values| values.first().copied()),
    None => Ok(None),
  };
  let side = |tag: u16| {
    number(tag).map(|value| {
      value
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
    })
  };

  let photometric = short(262)?;
  // A sample beyond the color's own is alpha when the file says it is.
  let has_alpha = numbers(338)?.iter().any(|extra| matches!(extra, 1 | 2));
  let color_model = match photometric {
    Some(0 | 1) if has_alpha => ColorModel::GrayscaleAlpha,
    Some(0 | 1) => ColorModel::Grayscale,
    Some(2) if has_alpha => ColorModel::Rgba,
    Some(2) => ColorModel::Rgb,
    Some(3) => ColorModel::Indexed,
    Some(5) => ColorModel::Cmyk,
    Some(6) => ColorModel::Ycbcr,
    Some(8 | 9) => ColorModel::Lab,
    _ => ColorModel::Unknown,
  };
  let color_space = match color_model {
    ColorModel::Grayscale | ColorModel::GrayscaleAlpha => ColorSpace::Gray,
    ColorModel::Cmyk => ColorSpace::Cmyk,
    ColorModel::Lab => ColorSpace::Lab,
    ColorModel::Rgb | ColorModel::Rgba | ColorModel::Indexed | ColorModel::Ycbcr => {
      ColorSpace::Srgb
    }
    _ => ColorSpace::Unknown,
  };
  // The unit a density is in: 2 the inch, 3 the centimeter, 1 none.
  let to_the_inch = match short(296)?.unwrap_or(2) {
    2 => Some(1.0),
    3 => Some(2.54),
    _ => None,
  };
  let dpi = match (fraction(282)?, fraction(283)?, to_the_inch) {
    (Some(across), Some(down), Some(scale)) => Some((across * scale, down * scale)),
    _ => None,
  };
  let detail = ImageDetail::Tiff(TiffDetail {
    big: tiff.big,
    little_endian: tiff.little_endian,
    pages: 1,
    compression: short(259)?,
    bits_per_sample: numbers(258)?
      .into_iter()
      .filter_map(|bits| u16::try_from(bits).ok())
      .collect(),
    samples_per_pixel: short(277)?.unwrap_or(1),
    photometric,
    dpi,
    software: text(305)?,
    date_time: text(306)?.map(|written| exif::date(&written, "").unwrap_or(written)),
  });
  let mut facts = ImageFacts {
    has_alpha: Some(has_alpha),
    color_model,
    color_space,
    xmp: bytes(700)?.and_then(|packet| xmp::parse(&packet)),
    ..ImageFacts::sized(side(256)?, side(257)?, detail)
  };
  if let Some(profile) = bytes(34675)?.and_then(|profile| icc::parse(&profile)) {
    facts.take_profile(profile);
  }
  facts.take_exif(exif::read(tiff, first)?);
  Ok(facts)
}

#[cfg(test)]
mod tests;
