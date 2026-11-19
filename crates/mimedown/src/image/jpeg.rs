//! JPEG: a run of segments, each a marker and a length, up to the scan where the picture
//! itself begins. The size is in the frame header; EXIF, XMP, the color profile and what Adobe
//! and JFIF have to say are each in a segment of their own.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::{exif, icc, xmp};
use crate::source::{Missing, Source};

/// What only a JPEG says.
#[derive(Debug, Clone, PartialEq)]
pub struct JpegDetail {
  /// Bits in each sample: 8, or 12 in some medical and scientific files.
  pub precision: u8,
  /// How many channels the picture has: 1, 3 or 4.
  pub components: u8,
  /// The picture is stored in passes of growing detail.
  pub progressive: bool,
  /// Every sample is exactly what was recorded: a JPEG of the lossless kind, or a JPEG-LS
  /// that allows no difference.
  pub lossless: bool,
  /// Pixels to the inch across and down, when JFIF states a density in a real unit.
  pub dpi: Option<(f64, f64)>,
  /// The file opens as JFIF does.
  pub jfif: bool,
  /// What Adobe's segment says was done to the color: 0 nothing, 1 YCbCr, 2 YCCK.
  pub adobe_transform: Option<u8>,
}

/// No more segments than this are walked.
const MOST_SEGMENTS: usize = 10_000;

/// What the segments said, gathered as they are walked.
#[derive(Default)]
struct Segments {
  /// The frame header's marker and its first six bytes.
  frame: Option<(u8, [u8; 6])>,
  /// JFIF's unit and its two densities.
  density: Option<(u8, u16, u16)>,
  jfif: bool,
  adobe_transform: Option<u8>,
  exif: Option<exif::Exif>,
  xmp: Option<xmp::Xmp>,
  /// A profile too long for one segment is cut into numbered pieces.
  profile: Vec<(u8, Vec<u8>)>,
}

impl Segments {
  fn take(&mut self, marker: u8, payload: &[u8]) {
    match marker {
      0xE0 => {
        if let Some([_, _, unit, x0, x1, y0, y1, ..]) = payload.strip_prefix(b"JFIF\0") {
          self.jfif = true;
          self.density = Some((
            *unit,
            u16::from_be_bytes([*x0, *x1]),
            u16::from_be_bytes([*y0, *y1]),
          ));
        }
      }
      0xE1 => {
        if let Some(block) = payload.strip_prefix(b"Exif\0\0") {
          self.exif = exif::from_bytes(block);
        } else if let Some(packet) = payload.strip_prefix(b"http://ns.adobe.com/xap/1.0/\0") {
          self.xmp = xmp::parse(packet);
        }
      }
      0xE2 => {
        if let Some([number, _, piece @ ..]) = payload.strip_prefix(b"ICC_PROFILE\0") {
          self.profile.push((*number, piece.to_vec()));
        }
      }
      0xEE => {
        if payload.starts_with(b"Adobe") {
          self.adobe_transform = payload.get(11).copied();
        }
      }
      // A frame header: the depth of a sample, the height, the width, the channels.
      _ => self.frame = payload.first_chunk::<6>().map(|start| (marker, *start)),
    }
  }
}

/// The frame header of JPEG-LS, which is what tells one from a JPEG.
pub(crate) const LS_FRAME: u8 = 0xF7;

/// A frame header is any of C0 to CF but the three that are tables, or the one JPEG-LS has.
fn is_frame(marker: u8) -> bool {
  matches!(marker, 0xC0..=0xCF | LS_FRAME) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

/// The marker of the file's frame header, which says what kind of JPEG it is. None for a file
/// that reaches its picture without one.
pub(crate) fn frame_marker(source: &Source<'_>) -> Result<Option<u8>, Missing> {
  let mut at = 2u64;
  for _ in 0..MOST_SEGMENTS {
    let [lead, marker] = source.array::<2>(at)?;
    if lead != 0xFF {
      return Ok(None);
    }
    match marker {
      0xFF => {
        at += 1;
        continue;
      }
      0x01 | 0xD0..=0xD7 => {
        at += 2;
        continue;
      }
      0xDA | 0xD9 => return Ok(None),
      marker if is_frame(marker) => return Ok(Some(marker)),
      _ => {}
    }
    let length = u16::from_be_bytes(source.array::<2>(at + 2)?);
    at += 2 + u64::from(length.max(2));
  }
  Ok(None)
}

/// How far a sample of a JPEG-LS may be from what was recorded, which the header of its scan
/// says after it has named the scan's channels. Nothing at all for one that lost nothing.
fn ls_difference(source: &Source<'_>, scan_at: u64, reading: &mut Reading) -> Option<u8> {
  let cut = "inside the header of its scan";
  let length = reading.take(source.array::<2>(scan_at + 2), cut)?;
  let wanted = usize::from(u16::from_be_bytes(length).saturating_sub(2));
  let header = reading.take(source.read(scan_at + 4, wanted), cut)?;
  let channels = usize::from(*header.first()?);
  header.get(1 + 2 * channels).copied()
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let mut segments = Segments::default();
  let mut scan_at = None;
  let mut at = 2u64;
  for _ in 0..MOST_SEGMENTS {
    let Some([lead, marker]) = reading.take(source.array::<2>(at), "before its picture begins")
    else {
      break;
    };
    if lead != 0xFF {
      reading.warn("a segment does not begin with a marker");
      break;
    }
    match marker {
      // A marker may be padded with any number of these.
      0xFF => {
        at += 1;
        continue;
      }
      // These stand alone, with no length after them.
      0x01 | 0xD0..=0xD7 => {
        at += 2;
        continue;
      }
      // The picture begins, or the file ends.
      0xDA => {
        scan_at = Some(at);
        break;
      }
      0xD9 => break,
      _ => {}
    }
    let Some(length) = reading.take(source.array::<2>(at + 2), "inside a segment") else {
      break;
    };
    let length = u16::from_be_bytes(length);
    let payload_at = at + 4;
    at += 2 + u64::from(length.max(2));
    if !is_frame(marker) && !matches!(marker, 0xE0 | 0xE1 | 0xE2 | 0xEE) {
      continue;
    }
    let wanted = usize::from(length.saturating_sub(2));
    let Some(payload) = reading.take(source.read(payload_at, wanted), "inside a segment") else {
      break;
    };
    segments.take(marker, payload);
  }

  let Some((marker, [precision, h0, h1, w0, w1, components])) = segments.frame else {
    if reading.needs.is_none() && reading.warnings.is_empty() {
      reading.warn("the file has no frame header");
    }
    return None;
  };
  let color_model = match (components, segments.adobe_transform) {
    (1, _) => ColorModel::Grayscale,
    // JPEG-LS has no transform of color of its own: its channels are what they are.
    (3, _) if marker == LS_FRAME => ColorModel::Rgb,
    (3, Some(0)) => ColorModel::Rgb,
    (3, _) => ColorModel::Ycbcr,
    (4, Some(2)) => ColorModel::Ycck,
    (4, _) => ColorModel::Cmyk,
    _ => ColorModel::Unknown,
  };
  // With no profile, a picture is what its channels make it; three channels are taken for
  // sRGB, which is what a JPEG without a profile is shown as.
  let color_space = match color_model {
    ColorModel::Grayscale => ColorSpace::Gray,
    ColorModel::Cmyk | ColorModel::Ycck => ColorSpace::Cmyk,
    ColorModel::Rgb | ColorModel::Ycbcr => ColorSpace::Srgb,
    _ => ColorSpace::Unknown,
  };
  let dpi = segments.density.and_then(|(unit, across, down)| {
    let to_the_inch = match unit {
      1 => 1.0,
      // Dots to the centimeter.
      2 => 2.54,
      _ => return None,
    };
    (across != 0 && down != 0).then(|| {
      (
        f64::from(across) * to_the_inch,
        f64::from(down) * to_the_inch,
      )
    })
  });
  let lossless = match (marker, scan_at) {
    (0xC3 | 0xC7 | 0xCB | 0xCF, _) => true,
    (LS_FRAME, Some(scan_at)) => ls_difference(source, scan_at, reading) == Some(0),
    _ => false,
  };
  let detail = ImageDetail::Jpeg(JpegDetail {
    precision,
    components,
    // C2, and its kin among the frame headers, are the progressive ones.
    progressive: matches!(marker, 0xC2 | 0xC6 | 0xCA | 0xCE),
    lossless,
    dpi,
    jfif: segments.jfif,
    adobe_transform: segments.adobe_transform,
  });
  let mut facts = ImageFacts {
    has_alpha: Some(false),
    color_model,
    color_space,
    xmp: segments.xmp,
    ..ImageFacts::sized(
      u32::from(u16::from_be_bytes([w0, w1])),
      u32::from(u16::from_be_bytes([h0, h1])),
      detail,
    )
  };
  if !segments.profile.is_empty() {
    segments.profile.sort_by_key(|(number, _)| *number);
    let whole: Vec<u8> = segments
      .profile
      .into_iter()
      .flat_map(|(_, piece)| piece)
      .collect();
    match icc::parse(&whole) {
      Some(profile) => facts.take_profile(profile),
      None => {
        facts.icc_profile = Some("embedded".to_string());
        reading.warn("the color profile could not be read");
      }
    }
  }
  if let Some(exif) = segments.exif {
    facts.take_exif(exif);
  }
  Some(facts)
}

#[cfg(test)]
mod tests;
