//! Radiance HDR: lines of text that describe the picture, an empty line, and then one line
//! that gives its size and the order its pixels come in.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::source::{ByteRange, Source};

/// What the four bytes of a Radiance pixel are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrEncoding {
  /// Red, green and blue, and an exponent they share.
  Rgbe,
  /// The same for X, Y and Z, which name a color apart from any screen.
  Xyze,
}

impl HdrEncoding {
  pub fn as_str(self) -> &'static str {
    match self {
      HdrEncoding::Rgbe => "rgbe",
      HdrEncoding::Xyze => "xyze",
    }
  }
}

/// What only a Radiance picture says.
#[derive(Debug, Clone, PartialEq)]
pub struct HdrDetail {
  pub encoding: HdrEncoding,
  /// What every value was multiplied by when the file was written: the product of the
  /// exposures its header lists.
  pub exposure: Option<f64>,
  /// The program that wrote the file, as it names itself.
  pub software: Option<String>,
}

/// No more of a file than this is read for its header.
const LONGEST_HEADER: usize = 64 << 10;

/// The line that gives the size: an axis and a count, twice. The first count is the lines of
/// pixels and the second the pixels in a line. Which axis comes first and which way each
/// runs say how the picture is to be turned, given here as EXIF numbers it.
fn size_line(line: &str) -> Option<(u32, u32, u8)> {
  let mut parts = line.split_whitespace();
  let first = parts.next()?;
  let lines = parts.next()?.parse::<u32>().ok()?;
  let second = parts.next()?;
  let across = parts.next()?.parse::<u32>().ok()?;
  let turn = match (first, second) {
    ("-Y", "+X") => 1,
    ("-Y", "-X") => 2,
    ("+Y", "-X") => 3,
    ("+Y", "+X") => 4,
    ("+X", "-Y") => 5,
    ("-X", "-Y") => 6,
    ("-X", "+Y") => 7,
    ("+X", "+Y") => 8,
    _ => return None,
  };
  Some((across, lines, turn))
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let head = source.head(LONGEST_HEADER);
  let text = String::from_utf8_lossy(head).replace("\r\n", "\n");
  // The header ends at an empty line, and the line after it gives the size.
  let lines = text
    .split_once("\n\n")
    .and_then(|(header, rest)| Some((header, rest.split_once('\n')?.0)));
  let Some((header, resolution)) = lines else {
    let given = u64::try_from(head.len()).unwrap_or(u64::MAX);
    let most = u64::try_from(LONGEST_HEADER).unwrap_or(u64::MAX);
    let to = source.size().map_or(most, |size| size.min(most));
    if to > given {
      reading
        .needs
        .get_or_insert(ByteRange::forward(given, to - given));
    } else if given < most {
      reading.warn("the file ends inside its header");
    } else {
      reading.warn("the header is longer than is read");
    }
    return None;
  };
  let Some((width, height, turn)) = size_line(resolution) else {
    reading.warn("the line that gives the picture's size is not one");
    return None;
  };

  let mut detail = HdrDetail {
    encoding: HdrEncoding::Rgbe,
    exposure: None,
    software: None,
  };
  for line in header.lines() {
    let Some((key, value)) = line.split_once('=') else {
      continue;
    };
    let value = value.trim();
    match key.trim() {
      "FORMAT" if value.contains("xyze") => detail.encoding = HdrEncoding::Xyze,
      "EXPOSURE" => {
        if let Ok(exposure) = value.parse::<f64>() {
          detail.exposure = Some(detail.exposure.unwrap_or(1.0) * exposure);
        }
      }
      "SOFTWARE" if !value.is_empty() => detail.software = Some(value.to_string()),
      _ => {}
    }
  }
  let (color_model, color_space) = match detail.encoding {
    // The colors red, green and blue stand for are the writer's own, and are not said here.
    HdrEncoding::Rgbe => (ColorModel::Rgb, ColorSpace::Unknown),
    HdrEncoding::Xyze => (ColorModel::Unknown, ColorSpace::Xyz),
  };
  Some(ImageFacts {
    has_alpha: Some(false),
    orientation: Some(turn),
    color_model,
    color_space,
    ..ImageFacts::sized(width, height, ImageDetail::Hdr(detail))
  })
}

#[cfg(test)]
mod tests;
