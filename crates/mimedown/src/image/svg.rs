//! SVG: a picture written as XML. Its size is what the attributes of its root element say and
//! nothing else in the file: a `width` three elements down belongs to something else.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::source::Source;
use crate::text::{self, markup};

/// What only an SVG says.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgDetail {
  /// The root's `width` and `height`, as written, units and all.
  pub width: Option<String>,
  pub height: Option<String>,
  /// The root's `viewBox`: where the drawing's own coordinates start, and how far they run.
  pub view_box: Option<[f64; 4]>,
}

/// How much of the start of a file is read for its root element.
const HEAD_BYTES: usize = 64 << 10;

/// A length in pixels, when it is written in a unit that has a size of its own.
fn pixels(written: &str) -> Option<f64> {
  let written = written.trim();
  let digits = written
    .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
    .unwrap_or(written.len());
  let size: f64 = written[..digits].parse().ok()?;
  let to_pixels = match written[digits..].trim() {
    "" | "px" => 1.0,
    "pt" => 96.0 / 72.0,
    "pc" => 16.0,
    "mm" => 96.0 / 25.4,
    "cm" => 96.0 / 2.54,
    "in" => 96.0,
    // A share of something else, or a size that depends on a font.
    _ => return None,
  };
  Some(size * to_pixels)
}

/// A length as a whole number of pixels. A size that is not a size is none.
fn whole(length: f64) -> u32 {
  let rounded = length.round();
  if rounded.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&rounded) {
    // Checked to be a whole number within range, so the conversion keeps its value.
    rounded as u32
  } else {
    0
  }
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let head = source.head(HEAD_BYTES);
  let cut = source
    .whole_file()
    .is_none_or(|file| file.len() > head.len());
  let (_, text) = text::decode(head, cut)?;
  let Some(root) = markup::root(&text) else {
    // The root's tag runs past what was given.
    if source.whole_file().is_none() {
      reading
        .needs
        .get_or_insert(source.rest_from(source.head_len()));
    } else {
      reading.warn("the root element's tag is not closed");
    }
    return None;
  };
  let view_box = root.attribute("viewBox").and_then(|written| {
    let numbers: Vec<f64> = written
      .split(|c: char| c.is_whitespace() || c == ',')
      .filter(|part| !part.is_empty())
      .filter_map(|part| part.parse().ok())
      .collect();
    <[f64; 4]>::try_from(numbers).ok()
  });
  let side = |name: &str, of_box: usize| {
    root
      .attribute(name)
      .and_then(pixels)
      .or(view_box.map(|view_box| view_box[of_box]))
      .map_or(0, whole)
  };
  let detail = ImageDetail::Svg(SvgDetail {
    width: root.attribute("width").map(str::to_string),
    height: root.attribute("height").map(str::to_string),
    view_box,
  });
  Some(ImageFacts {
    color_model: ColorModel::Vector,
    color_space: ColorSpace::Srgb,
    ..ImageFacts::sized(side("width", 2), side("height", 3), detail)
  })
}

#[cfg(test)]
mod tests;
