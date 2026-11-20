//! A page that is one picture, as a scan or a fax is: the picture read, and its words set on
//! the page.

use contentdown::{PageLayout, TextSource, words_to_lines};
use image::DynamicImage;

use crate::engine::ready;
use crate::read::{Thresholding, Upright, place_words, recognize_words_upright_cut};
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
/// A picture of gray or of color is read twice, cut into black and white once for the whole
/// picture and once a part at a time ([`Thresholding`]), and the reading with more sure words
/// is kept. Measured on six invoice pages rendered in gray at 100, 150, 200, and 300 pixels
/// to the inch: 89%, 93%, 92%, and 92% of the PDFs' words were found this way, against 89%,
/// 91%, 83%, and 83% with one cut and 87%, 90%, 91%, and 92% a part at a time. A picture of
/// pure black and white is read once, since it reads the same either way.
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
  let one_cut = recognize_words_upright_cut(picture.image, lang, Some(ppi), Thresholding::Global);
  let read = match two_toned(picture.image) {
    true => one_cut,
    false => {
      let by_parts =
        recognize_words_upright_cut(picture.image, lang, Some(ppi), Thresholding::Adaptive);
      surer(one_cut, by_parts)
    }
  };
  page_of(read, picture.number, picture.size)
}

/// A word Tesseract is at least this sure of, out of 100, counts toward how good a reading is.
/// Below it are the specks and the rules that a poor cut turns into letters.
const SURE: f32 = 60.0;

/// How much a reading is worth: the confidence of its sure words, added up.
fn worth(read: &Upright) -> f32 {
  let sure = read.words.iter().map(|word| word.confidence);
  sure.filter(|confidence| *confidence >= SURE).sum()
}

/// Of two readings of one picture, the one with more sure words; the first when they are
/// level.
pub(crate) fn surer(first: Upright, second: Upright) -> Upright {
  if worth(&second) > worth(&first) {
    second
  } else {
    first
  }
}

/// Whether every pixel of a picture is pure black or pure white: what a fax or a scan in one
/// bit is. Such a picture reads the same however it is cut.
pub(crate) fn two_toned(image: &DynamicImage) -> bool {
  let pure = |sample: &u8| *sample == 0 || *sample == 255;
  match image {
    DynamicImage::ImageLuma8(gray) => gray.as_raw().iter().all(pure),
    DynamicImage::ImageRgb8(color) => color
      .pixels()
      .all(|pixel| pure(&pixel.0[0]) && pixel.0[0] == pixel.0[1] && pixel.0[1] == pixel.0[2]),
    other => other.to_luma8().as_raw().iter().all(pure) && !other.color().has_color(),
  }
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
