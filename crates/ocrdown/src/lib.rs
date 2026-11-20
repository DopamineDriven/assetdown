//! Text from a picture with Tesseract: the words, their boxes, and the turn that reads them
//! upright.
//!
//! A picture is an `image::DynamicImage`, and nothing here knows what file it came from: the
//! format crates hand over a page's raster and place the words that come back. Tesseract and
//! Leptonica are linked from the system.

mod engine;
mod page;
mod read;
mod resolution;
#[cfg(test)]
mod testing;
mod turn;

pub use engine::{in_order, normalize_max_threads, pool, ready};
pub use page::{PagePicture, recognize_page};
pub use read::{
  OcrWord, Upright, place_words, recognize_text, recognize_words, recognize_words_upright,
};
pub use resolution::{DEFAULT_SOURCE_PPI, source_ppi};
pub use turn::{OrientationSource, unrotate_pixel_box};
