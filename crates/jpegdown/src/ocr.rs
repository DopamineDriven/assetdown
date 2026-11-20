//! Reading the words of the page, with the `ocr` feature: the picture is handed to Tesseract,
//! and the words that come back are set on the page.

use contentdown::PageLayout;
use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};
use ocrdown::{PagePicture, WORKING_PPI, enlarged, recognize_page};

use crate::{Jpeg, JpegError};

/// What a file's page reads as.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognized {
  page: PageLayout,
}

impl Recognized {
  /// The page with its words and the lines they make.
  pub fn page(&self) -> &PageLayout {
    &self.page
  }

  /// The page itself.
  pub fn into_page(self) -> PageLayout {
    self.page
  }

  /// The page's text: its lines from the top down, a line of text to each.
  pub fn text(&self) -> String {
    let lines: Vec<&str> = self
      .page
      .lines
      .iter()
      .map(|line| line.text.as_str())
      .collect();
    lines.join("\n")
  }

  /// The invoice fields read from the page. What the page warns of comes first among its
  /// warnings.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    let mut invoice = extract_from_pages(std::slice::from_ref(&self.page), options);
    let of_the_page = self.page.warnings.iter().cloned();
    invoice.warnings = of_the_page.chain(invoice.warnings).collect();
    invoice
  }
}

impl Jpeg {
  /// The page's words where they lie on it, in points counted up from its foot, with the
  /// lines they make. The page is the picture as it is to be looked at, at the size
  /// [`crate::Page::size_as_read`] gives, and is turned further when its text reads another
  /// way up. A picture with fewer than 200 pixels to the inch is enlarged to that before it
  /// is read. A size that was assumed, an enlarging, and a turn are each said in the page's
  /// warnings. Fails as [`Jpeg::raster`] does when the picture cannot be decoded.
  pub fn recognize(&self, lang: &str) -> Result<Recognized, JpegError> {
    let image = self.raster()?;
    let size = self.page().size_as_read();
    let mut warnings = Vec::new();
    if let Some(sheet) = size.assumed {
      warnings.push(format!(
        "page 1: the file states no resolution that can be believed, so the picture is taken to be a {sheet:?} sheet"
      ));
    }
    let larger = enlarged(&image, size.width);
    if larger.is_some() {
      let ppi = f64::from(image.width()) * 72.0 / size.width;
      warnings.push(format!(
        "page 1: the picture has {ppi:.0} pixels to the inch, and was enlarged to {WORKING_PPI} before it was read"
      ));
    }
    let picture = PagePicture {
      image: larger.as_ref().unwrap_or(&image),
      number: 1,
      size: (size.width, size.height),
    };
    let mut page = recognize_page(picture, lang);
    // What was done to the picture comes before what reading it found.
    warnings.append(&mut page.warnings);
    page.warnings = warnings;
    Ok(Recognized { page })
  }
}

#[cfg(test)]
mod tests;
