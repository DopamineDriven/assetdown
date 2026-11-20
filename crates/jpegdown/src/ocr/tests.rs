use contentdown::PageLayout;
use image::imageops;
use image::{DynamicImage, GrayImage};

use crate::testing::{Exif, drawn, encoded, exif, with_segment};
use crate::{Jpeg, JpegError};

/// "TOTAL 42" drawn with dots of 8 pixels, its corner 170 across and 220 down, on a picture
/// 1700 by 600: at 200 pixels to the inch, 612 by 216 points. "TOTAL" is 232 pixels wide and
/// 56 tall.
fn total_42() -> GrayImage {
  drawn("TOTAL 42", 8, (1700, 600), (170, 220))
}

fn jpeg(picture: GrayImage, dpi: Option<u16>) -> Vec<u8> {
  encoded(&DynamicImage::ImageLuma8(picture), dpi)
}

fn lines(page: &PageLayout) -> Vec<&str> {
  page.lines.iter().map(|line| line.text.as_str()).collect()
}

/// Within two points: Tesseract's box hugs the ink to a pixel or two, and a picture that was
/// enlarged has softer edges.
fn near(a: f64, b: f64) -> bool {
  (a - b).abs() < 2.0
}

/// "TOTAL" where `total_42` puts it on a page 612 by 216 points.
fn total_is_in_its_place(page: &PageLayout) {
  let total = &page.words[0];
  assert_eq!(total.text, "TOTAL");
  assert!(near(total.x0, 61.2) && near(total.x1, 144.72), "{total:?}");
  assert!(near(total.y1, 136.8) && near(total.y0, 116.64), "{total:?}");
}

#[test]
fn a_scans_words_are_found_where_they_lie_on_the_page() {
  let read = Jpeg::open(jpeg(total_42(), Some(200)))
    .unwrap()
    .recognize("eng")
    .unwrap();
  let page = read.page();
  assert_eq!(lines(page), ["TOTAL 42"]);
  assert_eq!((page.page, page.width, page.height), (1, 612.0, 216.0));
  assert_eq!((page.rotate, page.content_rotation), (0, 0));
  assert_eq!(page.warnings, Vec::<String>::new());
  total_is_in_its_place(page);
  assert_eq!(read.text(), "TOTAL 42");
}

#[test]
fn a_picture_with_few_pixels_to_the_inch_is_enlarged_before_it_is_read_and_says_so() {
  // The same page at half the size in pixels: 100 to the inch.
  let small = drawn("TOTAL 42", 4, (850, 300), (85, 110));
  let read = Jpeg::open(jpeg(small, Some(100)))
    .unwrap()
    .recognize("eng")
    .unwrap();
  let page = read.page();
  assert_eq!(lines(page), ["TOTAL 42"]);
  assert_eq!((page.width, page.height), (612.0, 216.0));
  assert_eq!(
    page.warnings,
    ["page 1: the picture has 100 pixels to the inch, and was enlarged to 200 before it was read"]
  );
  total_is_in_its_place(page);
}

#[test]
fn a_picture_kept_on_its_side_is_read_as_it_is_to_be_looked_at() {
  // Orientation 6: the rows of the file run down the right side of the page.
  let kept = jpeg(imageops::rotate270(&total_42()), Some(200));
  let kept = with_segment(&kept, &exif(true, &[(0x0112, Exif::Short(6))]));
  let read = Jpeg::open(kept).unwrap().recognize("eng").unwrap();
  let page = read.page();
  assert_eq!(lines(page), ["TOTAL 42"]);
  assert_eq!(
    (page.width, page.height, page.content_rotation),
    (612.0, 216.0, 0)
  );
  total_is_in_its_place(page);
}

#[test]
fn a_picture_that_states_no_resolution_is_read_as_the_sheet_it_is_taken_to_be_and_says_so() {
  // A letter page at 200 pixels to the inch that does not say so.
  let sheet = drawn("TOTAL 42", 8, (1700, 2200), (170, 220));
  let read = Jpeg::open(jpeg(sheet, None))
    .unwrap()
    .recognize("eng")
    .unwrap();
  let page = read.page();
  assert_eq!(lines(page), ["TOTAL 42"]);
  assert_eq!((page.width, page.height), (612.0, 792.0));
  assert_eq!(
    page.warnings,
    [
      "page 1: the file states no resolution that can be believed, so the picture is taken to be a Letter sheet"
    ]
  );
  let total = &page.words[0];
  assert!(near(total.x0, 61.2) && near(total.x1, 144.72), "{total:?}");
  // 220 pixels down a page 2,200 pixels and 792 points tall.
  assert!(
    near(total.y1, 792.0 - 79.2) && near(total.y0, 792.0 - 99.36),
    "{total:?}"
  );
}

#[test]
fn the_invoice_is_read_from_the_page_and_carries_what_the_page_warns_of() {
  let small = drawn("TOTAL 42", 4, (850, 300), (85, 110));
  let read = Jpeg::open(jpeg(small, Some(100)))
    .unwrap()
    .recognize("eng")
    .unwrap();
  let invoice = read.invoice(None);
  assert_eq!(invoice.source, contentdown::invoice::InvoiceSource::Ocr);
  assert!(
    invoice
      .warnings
      .iter()
      .any(|warning| warning.contains("was enlarged to 200")),
    "{:?}",
    invoice.warnings
  );
}

#[test]
fn a_picture_that_cannot_be_decoded_gives_the_reason_and_no_page() {
  use crate::testing::{file, frame};
  let error = Jpeg::open(file(&[frame(0xC3, 8, 4, 4, 1)]))
    .unwrap()
    .recognize("eng")
    .unwrap_err();
  assert!(
    matches!(error, JpegError::UnsupportedFormat(_)),
    "{error:?}"
  );
}
