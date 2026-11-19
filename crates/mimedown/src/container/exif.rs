//! EXIF: what a camera wrote down about a picture. It is a TIFF structure with three
//! directories: the first for the picture, one for how it was taken, one for where.

use std::cell::Cell;

use super::ifd::{Directory, Tiff, if_there};
use crate::source::{Missing, Source};

/// What EXIF says of a picture.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Exif {
  /// How the picture is to be turned for display, from 1 to 8.
  pub orientation: Option<u8>,
  pub make: Option<String>,
  pub model: Option<String>,
  pub lens_model: Option<String>,
  /// When the picture was taken, with its offset from UTC when the camera recorded one.
  pub date_time_original: Option<String>,
  /// Latitude and longitude in degrees, south and west below zero.
  pub position: Option<(f64, f64)>,
  /// 1 for sRGB; 65535 for a picture that names no space.
  pub color_space: Option<u16>,
}

const ORIENTATION: u16 = 0x0112;
const MAKE: u16 = 0x010F;
const MODEL: u16 = 0x0110;
const EXIF_DIRECTORY: u16 = 0x8769;
const GPS_DIRECTORY: u16 = 0x8825;
const DATE_TIME_ORIGINAL: u16 = 0x9003;
const OFFSET_TIME_ORIGINAL: u16 = 0x9011;
const COLOR_SPACE: u16 = 0xA001;
const LENS_MODEL: u16 = 0xA434;

/// What the EXIF block in `bytes` says. It stands alone, so nothing can be missing from it.
pub(crate) fn from_bytes(bytes: &[u8]) -> Option<Exif> {
  let source = Source::whole(bytes);
  let tiff = Tiff::open(&source).ok()??;
  let first = tiff.directory(tiff.first).ok()?;
  read(&tiff, &first).ok()
}

/// What a TIFF structure says, starting from its first directory.
pub(crate) fn read(tiff: &Tiff<'_, '_>, first: &Directory) -> Result<Exif, Missing> {
  // What a pointer left dangling pointed at is left out, and the rest is read.
  let lost = Cell::new(false);
  let text = |directory: &Directory, tag: u16| match directory.entry(tag) {
    Some(entry) => if_there(tiff.text(entry), &lost),
    None => Ok(None),
  };
  let number = |directory: &Directory, tag: u16| match directory.entry(tag) {
    Some(entry) => if_there(tiff.number(entry), &lost),
    None => Ok(None),
  };
  // Zero is no directory, as it is no next one.
  let pointed = |tag: u16| match number(first, tag)? {
    Some(offset) if offset != 0 => if_there(tiff.directory(offset).map(Some), &lost),
    _ => Ok(None),
  };

  let mut exif = Exif {
    orientation: number(first, ORIENTATION)?
      .and_then(|turn| u8::try_from(turn).ok())
      .filter(|turn| (1..=8).contains(turn)),
    make: text(first, MAKE)?,
    model: text(first, MODEL)?,
    ..Exif::default()
  };
  if let Some(taken) = pointed(EXIF_DIRECTORY)? {
    exif.lens_model = text(&taken, LENS_MODEL)?;
    exif.color_space = number(&taken, COLOR_SPACE)?.and_then(|space| u16::try_from(space).ok());
    let offset = text(&taken, OFFSET_TIME_ORIGINAL)?.unwrap_or_default();
    exif.date_time_original =
      text(&taken, DATE_TIME_ORIGINAL)?.and_then(|written| date(&written, &offset));
  }
  if let Some(place) = pointed(GPS_DIRECTORY)? {
    let degrees = |value: u16, hemisphere: u16, below: &str| -> Result<Option<f64>, Missing> {
      let Some(entry) = place.entry(value) else {
        return Ok(None);
      };
      let parts = if_there(tiff.fractions(entry), &lost)?;
      let [whole, minutes, seconds] = parts[..] else {
        return Ok(None);
      };
      let size = whole + minutes / 60.0 + seconds / 3600.0;
      let negative = text(&place, hemisphere)?.is_some_and(|letter| letter == below);
      Ok(Some(if negative { -size } else { size }))
    };
    if let (Some(latitude), Some(longitude)) = (degrees(2, 1, "S")?, degrees(4, 3, "W")?) {
      exif.position = Some((latitude, longitude));
    }
  }
  Ok(exif)
}

/// `2026:11:17 23:36:27` as `2026-11-17T23:36:27`, with the offset after it when there is one.
/// A date that is all zeros or spaces is a camera that did not know the time.
pub(crate) fn date(written: &str, offset: &str) -> Option<String> {
  let (day, time) = written.split_once(' ')?;
  let day = day.replace(':', "-");
  let known = day.chars().any(|c| c.is_ascii_digit() && c != '0');
  known.then(|| format!("{day}T{time}{offset}"))
}

#[cfg(test)]
mod tests;
