//! A color profile: a header of 128 bytes that says what kind of profile it is, and a table of
//! tags, one of which is the name it goes by.

use crate::image::ColorSpace;

/// What a color profile says of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IccProfile {
  /// The name the profile gives itself: `sRGB IEC61966-2.1`, `Display P3`.
  pub description: Option<String>,
  /// `2.1.0`, `4.3.0`.
  pub version: String,
  /// What the profile is for: `mntr` for a display, `scnr` for an input device, `prtr` for
  /// an output device.
  pub class: String,
  /// The color the profile describes: `RGB`, `CMYK`, `GRAY`, `Lab`.
  pub data_color_space: String,
  /// The space it is defined against: `XYZ` or `Lab`.
  pub connection_space: String,
  /// Who made the software that wrote it, and who made the device, as four letters each.
  pub cmm: Option<String>,
  pub manufacturer: Option<String>,
  /// When it was made. The format keeps this in UTC.
  pub created: Option<String>,
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_be_bytes(*bytes.get(at..)?.first_chunk::<4>()?))
}

fn be16(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_be_bytes(*bytes.get(at..)?.first_chunk::<2>()?))
}

/// Four letters, without the spaces and zeros that pad them.
fn letters(bytes: &[u8], at: usize) -> Option<String> {
  let four = bytes.get(at..at + 4)?;
  let text: String = four.iter().map(|byte| char::from(*byte)).collect();
  let text = text
    .trim_matches(|c: char| c == ' ' || c == '\0')
    .to_string();
  (!text.is_empty() && text.is_ascii()).then_some(text)
}

/// What the profile in `bytes` says, when it is one.
pub(crate) fn parse(bytes: &[u8]) -> Option<IccProfile> {
  if bytes.get(36..40)? != b"acsp" {
    return None;
  }
  let version = bytes.get(8..10)?;
  let created = (0..6)
    .map(|field| be16(bytes, 24 + 2 * field))
    .collect::<Option<Vec<u16>>>()
    .filter(|fields| fields[0] != 0)
    .map(|f| {
      format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        f[0], f[1], f[2], f[3], f[4], f[5]
      )
    });
  Some(IccProfile {
    description: description(bytes),
    version: format!("{}.{}.{}", version[0], version[1] >> 4, version[1] & 0x0F),
    class: letters(bytes, 12).unwrap_or_default(),
    data_color_space: letters(bytes, 16).unwrap_or_default(),
    connection_space: letters(bytes, 20).unwrap_or_default(),
    cmm: letters(bytes, 4),
    manufacturer: letters(bytes, 48),
    created,
  })
}

/// The profile's name, from its `desc` tag. Version 2 keeps it as plain text and version 4 as
/// one or more translations in UTF-16, of which the first is taken.
fn description(bytes: &[u8]) -> Option<String> {
  let tags = usize::try_from(be32(bytes, 128)?).ok()?.min(256);
  let (at, length) = (0..tags).find_map(|tag| {
    let entry = 132 + 12 * tag;
    (bytes.get(entry..entry + 4)? == b"desc").then_some(())?;
    Some((
      usize::try_from(be32(bytes, entry + 4)?).ok()?,
      usize::try_from(be32(bytes, entry + 8)?).ok()?,
    ))
  })?;
  let tag = bytes.get(at..at.checked_add(length)?)?;
  let text = match tag.get(..4)? {
    b"desc" => {
      let length = usize::try_from(be32(tag, 8)?).ok()?;
      let text = tag.get(12..12usize.checked_add(length)?)?;
      text
        .iter()
        .map(|byte| char::from(*byte))
        .collect::<String>()
    }
    b"mluc" => {
      let length = usize::try_from(be32(tag, 20)?).ok()?;
      let start = usize::try_from(be32(tag, 24)?).ok()?;
      let (units, _) = tag.get(start..start.checked_add(length)?)?.as_chunks::<2>();
      let units = units.iter().map(|unit| u16::from_be_bytes(*unit));
      char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
    }
    _ => return None,
  };
  let text = text
    .trim_matches(|c: char| c == '\0' || c.is_whitespace())
    .to_string();
  (!text.is_empty()).then_some(text)
}

impl IccProfile {
  /// The space the profile stands for: by its name when the name is a known one, and
  /// otherwise by the kind of color it describes.
  pub(crate) fn color_space(&self) -> ColorSpace {
    let named = self.description.as_deref().and_then(ColorSpace::named);
    named.unwrap_or(match self.data_color_space.as_str() {
      "GRAY" => ColorSpace::Gray,
      "CMYK" => ColorSpace::Cmyk,
      "Lab" => ColorSpace::Lab,
      "XYZ" => ColorSpace::Xyz,
      _ => ColorSpace::Unknown,
    })
  }

  /// What the profile is called in an image's facts.
  pub(crate) fn name(&self) -> String {
    self
      .description
      .clone()
      .unwrap_or_else(|| "embedded".to_string())
  }
}

#[cfg(test)]
mod tests;
