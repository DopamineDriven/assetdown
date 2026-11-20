//! A JPEG's header: the marked segments before its picture data, and what they say of it.

use crate::error::JpegError;

/// A resolution, in pixels to the inch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolution {
  pub x: f64,
  pub y: f64,
}

/// How a picture's data is coded, from the marker of its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coding {
  /// What nearly every JPEG is: eight bits, one pass.
  Baseline,
  /// One pass, with deeper samples or more tables than the baseline allows.
  Extended,
  /// Several passes, each sharpening the last.
  Progressive,
  /// Kept exactly, with no loss. A coding of its own, which is not read.
  Lossless,
  /// Another coding, by the marker of its frame: the arithmetic and the layered ones.
  Other(u8),
}

/// No more segments than this are walked before the picture data. A camera writes a few
/// dozen; this is room for a file stuffed with notes, and no room for one made to waste time.
pub(crate) const MOST_SEGMENTS: usize = 10_000;

/// What the segments before the picture data say.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Header {
  pub width: u32,
  pub height: u32,
  /// How many bits a sample has.
  pub precision: u8,
  /// How many components a pixel has: 1 is gray, 3 is color, 4 is the four inks of a press.
  pub components: u8,
  pub coding: Coding,
  pub resolution: Option<Resolution>,
  /// How the picture is to be turned for display, numbered from 1 to 8 as Exif does: 1 is as
  /// it lies.
  pub orientation: u8,
  pub software: Option<String>,
  /// `2015-03-04T10:22:33`, as the file records it, with no time zone.
  pub date_time: Option<String>,
  /// What the Adobe segment says of the components: 0 as they are, 1 YCbCr, 2 YCCK.
  pub adobe_transform: Option<u8>,
}

/// What Exif's first directory adds.
#[derive(Default)]
struct Exif {
  orientation: Option<u8>,
  resolution: Option<Resolution>,
  software: Option<String>,
  date_time: Option<String>,
}

const CENTIMETERS_TO_THE_INCH: f64 = 2.54;

/// A resolution in pixels to the inch, from two counts and the unit they are in: 2 is the inch
/// and 3 the centimeter, as Exif numbers them. None when either count is not a positive number.
fn resolution(across: f64, down: f64, unit: u16) -> Option<Resolution> {
  let real = |count: f64| count.is_finite() && count > 0.0;
  if !real(across) || !real(down) {
    return None;
  }
  match unit {
    2 => Some(Resolution { x: across, y: down }),
    3 => Some(Resolution {
      x: across * CENTIMETERS_TO_THE_INCH,
      y: down * CENTIMETERS_TO_THE_INCH,
    }),
    _ => None,
  }
}

/// `2015:03:04 10:22:33` as `2015-03-04T10:22:33`. Anything of another shape is left out.
fn date_time(written: &str) -> Option<String> {
  let bytes = written.as_bytes();
  let shaped = bytes.len() == 19
    && bytes.iter().enumerate().all(|(at, byte)| match at {
      4 | 7 | 13 | 16 => *byte == b':',
      10 => *byte == b' ',
      _ => byte.is_ascii_digit(),
    });
  shaped.then(|| {
    format!(
      "{}-{}-{}T{}",
      &written[..4],
      &written[5..7],
      &written[8..10],
      &written[11..]
    )
  })
}

/// Exif's first directory, from the bytes after `Exif\0\0`. Damage anywhere in it leaves out
/// what could not be read and nothing else: the picture is still a picture.
fn exif(tiff: &[u8]) -> Exif {
  let mut found = Exif::default();
  let little = match tiff.get(..2) {
    Some(b"II") => true,
    Some(b"MM") => false,
    _ => return found,
  };
  let two = |at: usize| {
    let bytes: [u8; 2] = tiff.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(if little {
      u16::from_le_bytes(bytes)
    } else {
      u16::from_be_bytes(bytes)
    })
  };
  let four = |at: usize| {
    let bytes: [u8; 4] = tiff.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(if little {
      u32::from_le_bytes(bytes)
    } else {
      u32::from_be_bytes(bytes)
    })
  };
  let place = |at: usize| four(at).and_then(|offset| usize::try_from(offset).ok());
  let fraction = |entry: usize| {
    let at = place(entry + 8)?;
    let (over, under) = (four(at)?, four(at.checked_add(4)?)?);
    (under != 0).then(|| f64::from(over) / f64::from(under))
  };
  let text = |entry: usize| {
    let length = usize::try_from(four(entry + 4)?).ok()?;
    // A value of four bytes or fewer is kept in the entry itself.
    let at = if length <= 4 {
      entry + 8
    } else {
      place(entry + 8)?
    };
    let bytes = tiff.get(at..at.checked_add(length)?)?;
    let bytes = bytes.split(|byte| *byte == 0).next()?;
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    (!text.is_empty()).then_some(text)
  };
  let Some(directory) = place(4) else {
    return found;
  };
  let Some(count) = two(directory) else {
    return found;
  };
  let (mut across, mut down, mut unit) = (None, None, 2);
  for index in 0..usize::from(count) {
    let entry = directory + 2 + 12 * index;
    let Some(tag) = two(entry) else { break };
    match tag {
      0x0112 => found.orientation = two(entry + 8).and_then(|v| u8::try_from(v).ok()),
      0x011A => across = fraction(entry),
      0x011B => down = fraction(entry),
      0x0128 => unit = two(entry + 8).unwrap_or(2),
      0x0131 => found.software = text(entry),
      0x0132 => found.date_time = text(entry).as_deref().and_then(date_time),
      _ => {}
    }
  }
  if let (Some(across), Some(down)) = (across, down) {
    found.resolution = resolution(across, down, unit);
  }
  found
}

/// The header of the JPEG in `bytes`: every segment up to the start of its picture data.
pub(crate) fn read(bytes: &[u8]) -> Result<Header, JpegError> {
  if bytes.get(..2) != Some(&[0xFF, 0xD8]) {
    return Err(JpegError::UnsupportedFormat(
      "the file does not open as a JPEG does".to_string(),
    ));
  }
  let past_the_end = || JpegError::Malformed("a segment runs past the end of the file".to_string());
  let mut frame = None;
  let mut jfif = None;
  let mut from_exif = Exif::default();
  let mut adobe_transform = None;
  let mut at = 2;
  let mut walked = 0usize;
  // A marker is 0xFF and a byte that names it; any number of 0xFF may stand in front.
  while let Some(&lead) = bytes.get(at) {
    if lead != 0xFF {
      return Err(JpegError::Malformed(
        "what follows a segment is not the mark of another".to_string(),
      ));
    }
    let Some(&marker) = bytes.get(at + 1) else {
      break;
    };
    if marker == 0xFF {
      at += 1;
      continue;
    }
    at += 2;
    match marker {
      // The picture data starts, or the file ends.
      0xDA | 0xD9 => break,
      // Marks that stand alone, with nothing after them.
      0x01 | 0xD0..=0xD8 => continue,
      _ => {}
    }
    walked += 1;
    if walked > MOST_SEGMENTS {
      return Err(JpegError::LimitExceeded(format!(
        "the file has more than {MOST_SEGMENTS} segments before its picture"
      )));
    }
    let length: [u8; 2] = bytes
      .get(at..at + 2)
      .and_then(|length| length.try_into().ok())
      .ok_or_else(past_the_end)?;
    let length = usize::from(u16::from_be_bytes(length));
    let payload = length
      .checked_sub(2)
      .and_then(|held| bytes.get(at + 2..at + 2 + held))
      .ok_or_else(past_the_end)?;
    at += length;
    match marker {
      // A frame: every marker from 0xC0 to 0xCF but the three that are tables and a reserve.
      0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
        let [precision, h1, h2, w1, w2, components, ..] = *payload else {
          return Err(JpegError::Malformed(
            "the frame is too short to state a size".to_string(),
          ));
        };
        let coding = match marker {
          0xC0 => Coding::Baseline,
          0xC1 => Coding::Extended,
          0xC2 => Coding::Progressive,
          0xC3 => Coding::Lossless,
          other => Coding::Other(other),
        };
        let size = (
          u32::from(u16::from_be_bytes([w1, w2])),
          u32::from(u16::from_be_bytes([h1, h2])),
        );
        frame = Some((size, precision, components, coding));
      }
      0xE0 => {
        // After the name: two bytes of version, the unit, and the two counts.
        if let Some([_, _, unit, x1, x2, y1, y2, ..]) = payload.strip_prefix(b"JFIF\0") {
          let across = f64::from(u16::from_be_bytes([*x1, *x2]));
          let down = f64::from(u16::from_be_bytes([*y1, *y2]));
          // JFIF numbers the inch 1 and the centimeter 2; Exif numbers them 2 and 3.
          jfif = resolution(across, down, u16::from(*unit) + 1);
        }
      }
      0xE1 => {
        if let Some(tiff) = payload.strip_prefix(b"Exif\0\0") {
          from_exif = exif(tiff);
        }
      }
      0xEE => {
        if let Some(rest) = payload.strip_prefix(b"Adobe") {
          adobe_transform = rest.get(6).copied();
        }
      }
      _ => {}
    }
  }
  let Some(((width, height), precision, components, coding)) = frame else {
    return Err(JpegError::Malformed(
      "the file's picture begins before anything states its size".to_string(),
    ));
  };
  if width == 0 || height == 0 {
    return Err(JpegError::Malformed(
      "the frame states a picture with no width or no height".to_string(),
    ));
  }
  Ok(Header {
    width,
    height,
    precision,
    components,
    coding,
    resolution: jfif.or(from_exif.resolution),
    orientation: from_exif
      .orientation
      .filter(|turn| (1..=8).contains(turn))
      .unwrap_or(1),
    software: from_exif.software,
    date_time: from_exif.date_time,
    adobe_transform,
  })
}

#[cfg(test)]
mod tests;
