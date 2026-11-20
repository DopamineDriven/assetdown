use contentdown::{PageLayout, TextSource};
use image::GrayImage;
use image::imageops::{self, FilterType};

use crate::Tiff;
use crate::testing::{Value, drawn, sized, tiff};

/// A page of gray kept as it is, in one strip, and what else is said of it.
fn gray_page(picture: &GrayImage, more: Vec<(u16, Value)>) -> Vec<(u16, Value)> {
  let mut entries = vec![
    (256, Value::Long(vec![picture.width()])),
    (257, Value::Long(vec![picture.height()])),
    (258, Value::Short(vec![8])),
    (259, Value::Short(vec![1])),
    (262, Value::Short(vec![1])),
    (273, Value::Pieces(vec![picture.as_raw().clone()])),
    (277, Value::Short(vec![1])),
    (278, Value::Long(vec![picture.height()])),
  ];
  entries.extend(more);
  entries
}

/// So many pixels to the inch across, and so many down.
fn at(across: u32, down: u32) -> Vec<(u16, Value)> {
  vec![
    (282, Value::Fractions(vec![(across, 1)])),
    (283, Value::Fractions(vec![(down, 1)])),
    (296, Value::Short(vec![2])),
  ]
}

/// "TOTAL 42" drawn with dots of 8 pixels, its corner 170 across and 220 down, on a picture
/// 1700 by 600: at 200 pixels to the inch, 612 by 216 points. "TOTAL" is 232 pixels wide and
/// 56 tall.
fn total_42() -> GrayImage {
  drawn("TOTAL 42", 8, (1700, 600), (170, 220))
}

fn lines(page: &PageLayout) -> Vec<&str> {
  page.lines.iter().map(|line| line.text.as_str()).collect()
}

/// Within a point and a half: Tesseract's box hugs the ink to a pixel or two.
fn near(a: f64, b: f64) -> bool {
  (a - b).abs() < 1.5
}

/// "TOTAL" where `total_42` puts it on a page 612 by 216 points.
fn total_is_in_its_place(page: &PageLayout) {
  let total = &page.words[0];
  assert_eq!(total.text, "TOTAL");
  assert!(near(total.x0, 61.2) && near(total.x1, 144.72), "{total:?}");
  assert!(near(total.y1, 136.8) && near(total.y0, 116.64), "{total:?}");
}

#[test]
fn a_pages_words_are_found_where_they_lie_on_it() {
  let file = Tiff::open(tiff(true, &[gray_page(&total_42(), at(200, 200))])).unwrap();
  let page = file.page_layout(1, "eng").unwrap();
  assert_eq!(lines(&page), ["TOTAL 42"]);
  assert_eq!((page.page, page.width, page.height), (1, 612.0, 216.0));
  assert_eq!((page.rotate, page.content_rotation), (0, 0));
  assert_eq!(page.source, TextSource::Ocr);
  assert_eq!(page.warnings, Vec::<String>::new());
  total_is_in_its_place(&page);
}

#[test]
fn a_page_kept_on_its_side_is_read_as_it_is_to_be_looked_at() {
  // Orientation 6: the rows of the file run down the right side of the page.
  let kept = imageops::rotate270(&total_42());
  let mut said = at(200, 200);
  said.push((274, Value::Short(vec![6])));
  let file = Tiff::open(tiff(true, &[gray_page(&kept, said)])).unwrap();
  let page = file.page_layout(1, "eng").unwrap();
  assert_eq!(lines(&page), ["TOTAL 42"]);
  assert_eq!(
    (page.width, page.height, page.content_rotation),
    (612.0, 216.0, 0)
  );
  total_is_in_its_place(&page);
}

#[test]
fn a_fax_with_half_as_many_rows_to_the_inch_is_read_at_its_true_shape() {
  let squashed = imageops::resize(&total_42(), 1700, 300, FilterType::Nearest);
  let file = Tiff::open(tiff(true, &[gray_page(&squashed, at(200, 100))])).unwrap();
  let page = file.page_layout(1, "eng").unwrap();
  assert_eq!(lines(&page), ["TOTAL 42"]);
  assert_eq!((page.width, page.height), (612.0, 216.0));
  total_is_in_its_place(&page);
}

#[test]
fn a_page_that_states_no_resolution_is_sized_at_300_pixels_to_the_inch_and_says_so() {
  let file = Tiff::open(tiff(true, &[gray_page(&total_42(), Vec::new())])).unwrap();
  let page = file.page_layout(1, "eng").unwrap();
  assert_eq!(lines(&page), ["TOTAL 42"]);
  assert_eq!((page.width, page.height), (408.0, 144.0));
  assert_eq!(
    page.warnings,
    ["page 1: the file states no resolution, so its size is taken at 300 pixels to the inch"]
  );
}

/// A page with words, a page with none, and a page in a coding that is not read.
fn three_pages() -> Tiff {
  let blank = GrayImage::from_pixel(400, 200, image::Luma([255]));
  let mut coded = sized(400, 200);
  coded.extend(at(200, 200));
  coded.push((259, Value::Short(vec![7])));
  Tiff::open(tiff(
    true,
    &[
      gray_page(&total_42(), at(200, 200)),
      gray_page(&blank, at(200, 200)),
      coded,
    ],
  ))
  .unwrap()
}

const NOT_READ: &str =
  "page 3: its picture could not be read, so it has no words: unsupported format: ";

#[test]
fn every_page_is_read_in_order_and_one_that_cannot_be_is_listed_with_the_reason() {
  let read = three_pages().recognize("eng", Some(2));
  let [first, second, third] = read.pages() else {
    panic!(
      "three pages were read, and {} came back",
      read.pages().len()
    );
  };
  assert_eq!((first.page, lines(first)), (1, vec!["TOTAL 42"]));
  assert_eq!((second.page, second.words.len()), (2, 0));
  assert_eq!(second.warnings, Vec::<String>::new());
  assert_eq!((third.page, third.words.len()), (3, 0));
  assert_eq!((third.width, third.height), (144.0, 72.0));
  let [warning] = third.warnings.as_slice() else {
    panic!("one warning, and there are {:?}", third.warnings);
  };
  assert!(warning.starts_with(NOT_READ), "{warning}");
}

#[test]
fn the_text_of_each_page_is_its_lines_from_the_top_down() {
  let two_lines = {
    let mut picture = total_42();
    imageops::overlay(
      &mut picture,
      &drawn("42 TOTAL", 8, (600, 60), (0, 0)),
      170,
      380,
    );
    picture
  };
  let file = Tiff::open(tiff(true, &[gray_page(&two_lines, at(200, 200))])).unwrap();
  assert_eq!(
    file.recognize("eng", None).text_per_page(),
    ["TOTAL 42\n42 TOTAL"]
  );
  assert_eq!(
    three_pages().recognize("eng", None).text_per_page(),
    ["TOTAL 42", "", ""]
  );
}

#[test]
fn an_invoice_read_from_some_of_the_pages_says_which_were_not_read() {
  let invoice = three_pages().recognize("eng", None).invoice(None);
  assert_eq!(invoice.source, contentdown::invoice::InvoiceSource::Ocr);
  assert!(
    invoice
      .warnings
      .iter()
      .any(|warning| warning.starts_with(NOT_READ)),
    "{:?}",
    invoice.warnings
  );
}
