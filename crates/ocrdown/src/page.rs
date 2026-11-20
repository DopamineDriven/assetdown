//! A page that is one picture, as a scan or a fax is: the picture read, and its words set on
//! the page.

use contentdown::{PageLayout, TextSource, words_to_lines};
use image::DynamicImage;

use crate::engine::ready;
use crate::read::{Upright, place_words, recognize_words_upright};
use crate::resolution::source_ppi;
use crate::turn::OrientationSource;

/// A picture that is a whole page.
#[derive(Debug, Clone, Copy)]
pub struct PagePicture<'a> {
  pub image: &'a DynamicImage,
  /// The page's number, counted from 1.
  pub number: u32,
  /// The picture's width and height in points, of which there are 72 to the inch.
  pub size: (f64, f64),
}

/// The page a picture reads as: its words where they lie on it, in points counted up from its
/// foot, and the lines they make. The page is the picture turned the way its text reads.
///
/// When Tesseract cannot start with the language, the page has no words and a warning that
/// says so: it is not passed off as a blank page.
pub fn recognize_page(picture: PagePicture<'_>, lang: &str) -> PageLayout {
  if let Err(why) = ready(lang) {
    let nothing = Upright {
      words: Vec::new(),
      degrees: 0,
      size: (picture.image.width(), picture.image.height()),
      decided: OrientationSource::Upright,
    };
    let mut page = page_of(nothing, picture.number, picture.size);
    page
      .warnings
      .push(format!("page {}: not read: {why}", picture.number));
    return page;
  }
  let ppi = source_ppi(picture.image.width(), picture.size.0);
  let read = recognize_words_upright(picture.image, lang, Some(ppi));
  page_of(read, picture.number, picture.size)
}

/// The page a picture's reading makes. `size` is the picture's width and height in points as
/// it was given; the page is the picture turned the way its text reads, so a quarter turn
/// makes the page as wide as the picture was tall.
pub(crate) fn page_of(read: Upright, number: u32, size: (f64, f64)) -> PageLayout {
  let (width, height) = match read.degrees {
    90 | 270 => (size.1, size.0),
    _ => size,
  };
  let warnings = match read.degrees {
    0 => Vec::new(),
    degrees => vec![format!(
      "page {number}: text reads upright after a {degrees}° turn ({})",
      read.decided.label()
    )],
  };
  let placed = place_words(
    read.words,
    read.size.0,
    read.size.1,
    [0.0, 0.0, width, height],
  );
  let (words, lines) = words_to_lines(placed, &[]);
  PageLayout {
    page: number,
    width,
    height,
    rotate: 0,
    content_rotation: read.degrees,
    source: TextSource::Ocr,
    words,
    lines,
    images: Vec::new(),
    warnings,
  }
}

#[cfg(test)]
mod tests;
