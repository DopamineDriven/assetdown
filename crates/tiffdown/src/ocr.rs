//! Reading the words of a file's pages, with the `ocr` feature: each page's picture is handed to
//! Tesseract, and the words that come back are set on the page.

use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};
use contentdown::{PageLayout, TextSource};
use ocrdown::{DEFAULT_SOURCE_PPI, PagePicture, in_order, recognize_page};

use crate::{Page, Tiff, TiffError};

/// A page's width and height in points as it is to be looked at, and whether the file states
/// the resolution they come from. A page that states none is sized at what a scanner most
/// often writes.
fn size_as_looked_at(page: &Page) -> ((f64, f64), bool) {
  let assumed = |pixels: u32| f64::from(pixels) * 72.0 / f64::from(DEFAULT_SOURCE_PPI);
  let (width, height, stated) = match page.size_in_points() {
    Some((width, height)) => (width, height, true),
    None => (assumed(page.width), assumed(page.height), false),
  };
  // Orientations 5 to 8 keep the page on its side.
  match page.orientation {
    5..=8 => ((height, width), stated),
    _ => ((width, height), stated),
  }
}

/// A page whose picture could not be decoded: its size, no words, and why.
fn unread(page: &Page, error: &TiffError) -> PageLayout {
  let ((width, height), stated) = size_as_looked_at(page);
  let mut warnings = Vec::new();
  if !stated {
    warnings.push(no_resolution(page));
  }
  warnings.push(format!(
    "page {}: its picture could not be read, so it has no words: {error}",
    page.number
  ));
  PageLayout {
    page: page.number,
    width,
    height,
    rotate: 0,
    content_rotation: 0,
    source: TextSource::Ocr,
    words: Vec::new(),
    lines: Vec::new(),
    images: Vec::new(),
    warnings,
  }
}

fn no_resolution(page: &Page) -> String {
  format!(
    "page {}: the file states no resolution, so its size is taken at {DEFAULT_SOURCE_PPI} pixels to the inch",
    page.number
  )
}

/// What a file's pages read as.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognized {
  pages: Vec<PageLayout>,
}

impl Recognized {
  /// Each page with its words and the lines they make, in the file's order.
  pub fn pages(&self) -> &[PageLayout] {
    &self.pages
  }

  /// The pages themselves.
  pub fn into_pages(self) -> Vec<PageLayout> {
    self.pages
  }

  /// The text of each page: its lines from the top down, a line of text to each.
  pub fn text_per_page(&self) -> Vec<String> {
    let text = |page: &PageLayout| {
      let lines: Vec<&str> = page.lines.iter().map(|line| line.text.as_str()).collect();
      lines.join("\n")
    };
    self.pages.iter().map(text).collect()
  }

  /// The invoice fields read from the pages. What the pages warn of comes first among its
  /// warnings: the fields are from the pages that could be read, and a page that could not is
  /// named there.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    let mut invoice = extract_from_pages(&self.pages, options);
    let of_the_pages = self
      .pages
      .iter()
      .flat_map(|page| page.warnings.iter().cloned());
    invoice.warnings = of_the_pages.chain(invoice.warnings).collect();
    invoice
  }
}

impl Tiff {
  /// The words of the page of this number, counted from 1, where they lie on the page as it
  /// is to be looked at, in points counted up from its foot, with the lines they make. The
  /// page is turned further when its text reads another way up, and says so in a warning.
  pub fn page_layout(&self, number: u32, lang: &str) -> Result<PageLayout, TiffError> {
    let (page, _) = self.found(number)?;
    self.read(page, lang)
  }

  fn read(&self, page: &Page, lang: &str) -> Result<PageLayout, TiffError> {
    let image = self.raster(page.number)?;
    let (size, stated) = size_as_looked_at(page);
    let picture = PagePicture {
      image: &image,
      number: page.number,
      size,
    };
    let mut layout = recognize_page(picture, lang);
    if !stated {
      layout.warnings.insert(0, no_resolution(page));
    }
    Ok(layout)
  }

  /// Every page, read as [`Tiff::page_layout`] reads one, no more than `max_threads` at once
  /// (four when none is given, and never more than the machine has cores). A page whose
  /// picture cannot be decoded is there with its size, no words, and a warning that says why.
  pub fn recognize(&self, lang: &str, max_threads: Option<u32>) -> Recognized {
    let pages = in_order(max_threads, self.pages().iter().collect(), |page: &Page| {
      self
        .read(page, lang)
        .unwrap_or_else(|error| unread(page, &error))
    });
    Recognized { pages }
  }
}

#[cfg(test)]
mod tests;
