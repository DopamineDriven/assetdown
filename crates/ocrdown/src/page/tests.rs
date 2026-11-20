use contentdown::TextSource;

use super::*;
use crate::read::OcrWord;
use crate::testing::drawn;
use crate::turn::OrientationSource;

fn word(text: &str, [left, top, right, bottom]: [i32; 4]) -> OcrWord {
  OcrWord {
    text: text.into(),
    left,
    top,
    right,
    bottom,
    confidence: 91.0,
  }
}

fn close(a: f64, b: f64) -> bool {
  (a - b).abs() < 1e-9
}

/// A letter page scanned at 200 pixels to the inch: 1700 by 2200 pixels, 612 by 792 points.
const LETTER: (f64, f64) = (612.0, 792.0);

#[test]
fn a_words_box_is_measured_in_points_up_from_the_foot_of_the_page() {
  let read = Upright {
    words: vec![word("Invoice", [170, 220, 340, 264])],
    degrees: 0,
    size: (1700, 2200),
    decided: OrientationSource::Upright,
  };
  let page = page_of(read, 2, LETTER);
  assert_eq!(page.page, 2);
  assert_eq!((page.width, page.height), LETTER);
  assert_eq!((page.rotate, page.content_rotation), (0, 0));
  assert_eq!(page.source, TextSource::Ocr);
  assert_eq!(page.warnings, Vec::<String>::new());
  let [found] = page.words.as_slice() else {
    panic!("one word was read, and the page has {}", page.words.len());
  };
  assert_eq!(found.text, "Invoice");
  // 200 pixels to the inch is 0.36 of a point to the pixel.
  assert!(close(found.x0, 61.2) && close(found.x1, 122.4), "{found:?}");
  // 220 pixels down from the top is 79.2 points down, which is 712.8 up from the foot.
  assert!(
    close(found.y1, 712.8) && close(found.y0, 696.96),
    "{found:?}"
  );
  assert_eq!(found.confidence, Some(0.91));
}

#[test]
fn a_page_read_on_its_side_is_as_wide_as_its_picture_is_tall_and_says_what_turned_it() {
  // The same scan, lying on its side: read after a quarter turn, where it is 2200 by 1700.
  let read = Upright {
    words: vec![word("Invoice", [220, 170, 440, 214])],
    degrees: 90,
    size: (2200, 1700),
    decided: OrientationSource::Osd,
  };
  let page = page_of(read, 3, LETTER);
  assert_eq!((page.width, page.height), (792.0, 612.0));
  assert_eq!((page.rotate, page.content_rotation), (0, 90));
  assert_eq!(
    page.warnings,
    ["page 3: text reads upright after a 90° turn (by OSD)"]
  );
  let found = &page.words[0];
  assert!(close(found.x0, 79.2) && close(found.x1, 158.4), "{found:?}");
  assert!(
    close(found.y1, 550.8) && close(found.y0, 534.96),
    "{found:?}"
  );
}

#[test]
fn words_that_share_a_line_are_one_line_read_from_the_left() {
  let read = Upright {
    words: vec![
      word("42", [400, 220, 460, 264]),
      word("Total", [170, 300, 300, 344]),
      word("Invoice", [170, 220, 340, 264]),
    ],
    degrees: 0,
    size: (1700, 2200),
    decided: OrientationSource::Upright,
  };
  let page = page_of(read, 1, LETTER);
  let lines: Vec<&str> = page.lines.iter().map(|line| line.text.as_str()).collect();
  assert_eq!(lines, ["Invoice 42", "Total"]);
}

#[test]
fn a_picture_with_nothing_read_is_a_page_of_its_size_with_no_words() {
  let read = Upright {
    words: Vec::new(),
    degrees: 0,
    size: (1700, 2200),
    decided: OrientationSource::Upright,
  };
  let page = page_of(read, 1, LETTER);
  assert_eq!((page.width, page.height), LETTER);
  assert!(page.words.is_empty() && page.lines.is_empty() && page.warnings.is_empty());
}

/// "TOTAL 42" drawn with dots of 8 pixels, its corner 170 across and 220 down, on a picture
/// 1700 by 600 that is 612 by 216 points: 200 pixels to the inch. "TOTAL" is 232 pixels wide
/// and 56 tall.
fn total_42() -> DynamicImage {
  drawn("TOTAL 42", 8, (1700, 600), (170, 220))
}

/// Within a point and a half: Tesseract's box hugs the ink to a pixel or two.
fn near(a: f64, b: f64) -> bool {
  (a - b).abs() < 1.5
}

#[test]
fn a_drawn_word_is_read_and_found_where_it_was_drawn() {
  let picture = total_42();
  let page = recognize_page(
    PagePicture {
      image: &picture,
      number: 1,
      size: (612.0, 216.0),
    },
    "eng",
  );
  let lines: Vec<&str> = page.lines.iter().map(|line| line.text.as_str()).collect();
  assert_eq!(lines, ["TOTAL 42"]);
  assert_eq!(
    (page.width, page.height, page.content_rotation),
    (612.0, 216.0, 0)
  );
  let total = &page.words[0];
  assert!(near(total.x0, 61.2) && near(total.x1, 144.72), "{total:?}");
  assert!(near(total.y1, 136.8) && near(total.y0, 116.64), "{total:?}");
}

#[test]
fn a_picture_lying_on_its_side_is_read_turned_upright() {
  let picture = total_42().rotate90();
  let page = recognize_page(
    PagePicture {
      image: &picture,
      number: 1,
      size: (216.0, 612.0),
    },
    "eng",
  );
  let lines: Vec<&str> = page.lines.iter().map(|line| line.text.as_str()).collect();
  assert_eq!(lines, ["TOTAL 42"]);
  // Turned a quarter clockwise to make it; three more quarters put it back.
  assert_eq!(
    (page.width, page.height, page.content_rotation),
    (612.0, 216.0, 270)
  );
  let total = &page.words[0];
  assert!(near(total.x0, 61.2) && near(total.x1, 144.72), "{total:?}");
  assert!(near(total.y1, 136.8) && near(total.y0, 116.64), "{total:?}");
}

#[test]
fn a_language_tesseract_has_no_data_for_is_said_and_not_passed_off_as_a_blank_page() {
  let picture = total_42();
  let page = recognize_page(
    PagePicture {
      image: &picture,
      number: 4,
      size: (612.0, 216.0),
    },
    "no-such-language",
  );
  assert_eq!((page.page, page.width, page.height), (4, 612.0, 216.0));
  assert!(page.words.is_empty());
  assert_eq!(
    page.warnings,
    ["page 4: not read: Tesseract has no data here for the language `no-such-language`"]
  );
}
