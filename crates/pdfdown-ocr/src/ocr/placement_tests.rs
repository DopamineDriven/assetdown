//! Pictures painted through a matrix that turns them, and pages read in their order: the
//! picture tier must read each the way the page shows it, and set its words where it shows them.
//!
//! The pictures here read one way up only. What the search does with a picture that reads
//! either way up is tested where the search is, in `ocrdown`, on readings made up for the
//! purpose: how surely Tesseract reads a drawn `606` differs from one build of it to the next.

use lopdf::content::{Content, Operation};
use lopdf::{Object, Stream, dictionary};

use super::*;

/// Five dots across and seven down, a `#` where there is ink.
fn glyph(letter: char) -> [&'static str; 7] {
  match letter {
    'T' => [
      "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
    ],
    'O' => [
      ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
    ],
    'A' => [
      ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
    ],
    'L' => [
      "#....", "#....", "#....", "#....", "#....", "#....", "#####",
    ],
    '4' => [
      "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#.",
    ],
    '2' => [
      ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
    ],
    ' ' => ["....."; 7],
    other => panic!("no glyph is drawn for {other:?}"),
  }
}

/// Six lines of `TOTAL 42 TOTAL 42` in black on a white picture of 700 by 640: twenty-four
/// words that read one way up and no other.
fn plain() -> image::GrayImage {
  const DOT: u32 = 5;
  let mut picture = image::GrayImage::from_pixel(700, 640, image::Luma([255]));
  for line in 0..6u32 {
    let at = (60, 60 + line * 90);
    for (place, letter) in (0u32..).zip("TOTAL 42 TOTAL 42".chars()) {
      for (row, dots) in (0u32..).zip(glyph(letter)) {
        for (column, _) in (0u32..).zip(dots.chars()).filter(|(_, dot)| *dot == '#') {
          for (down, across) in (0..DOT).flat_map(|down| (0..DOT).map(move |across| (down, across)))
          {
            picture.put_pixel(
              at.0 + (place * 6 + column) * DOT + across,
              at.1 + row * DOT + down,
              image::Luma([0]),
            );
          }
        }
      }
    }
  }
  picture
}

/// A document of pages of one picture each, painted through a matrix, each saying it is shown
/// turned by so many degrees. Every page is `media` wide and high.
fn document_of(
  pages: &[(&image::GrayImage, [i64; 6], i64)],
  media: [i64; 2],
) -> (Document, Vec<u8>) {
  let mut doc = Document::with_version("1.7");
  let pages_id = doc.new_object_id();
  let mut kids = Vec::new();
  for (picture, matrix, rotate) in pages {
    let image = doc.add_object(Stream::new(
      dictionary! {
        "Type" => "XObject", "Subtype" => "Image",
        "Width" => i64::from(picture.width()), "Height" => i64::from(picture.height()),
        "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
      },
      picture.as_raw().clone(),
    ));
    let content = Content {
      operations: vec![
        Operation::new("q", vec![]),
        Operation::new("cm", matrix.iter().map(|value| (*value).into()).collect()),
        Operation::new("Do", vec!["Im0".into()]),
        Operation::new("Q", vec![]),
      ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
      "Type" => "Page", "Parent" => Object::Reference(pages_id),
      "MediaBox" => vec![0.into(), 0.into(), media[0].into(), media[1].into()],
      "Rotate" => *rotate,
      "Contents" => Object::Reference(content_id),
      "Resources" => Object::Dictionary(dictionary! {
        "XObject" => Object::Dictionary(dictionary! { "Im0" => Object::Reference(image) }),
      }),
    });
    kids.push(Object::Reference(page_id));
  }
  let count = i64::try_from(kids.len()).unwrap();
  doc.objects.insert(
    pages_id,
    Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
  );
  let catalog =
    doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
  doc.trailer.set("Root", Object::Reference(catalog));
  let mut bytes = Vec::new();
  doc.save_to(&mut bytes).unwrap();
  (doc, bytes)
}

/// The one page of a document of one picture, painted through `matrix` on a page of 350 by 320
/// that says it is shown turned by `rotate`, read through the picture tier.
fn read_page_of(picture: &image::GrayImage, matrix: [i64; 6], rotate: i64) -> PageLayout {
  let (doc, bytes) = document_of(&[(picture, matrix, rotate)], [350, 320]);
  let mut layout = extract_layout_with_ocr(&doc, &bytes, "eng", 1, 1, 150, RENDER_MODE_NEVER, None);
  assert_eq!(layout.len(), 1);
  let page = layout.remove(0);
  assert_eq!(page.source, TextSource::Ocr);
  page
}

/// How many times a page holds a word.
fn times(page: &PageLayout, word: &str) -> usize {
  page.words.iter().filter(|found| found.text == word).count()
}

/// Whether two pages hold the same words in the same places, to within half a point.
fn same_places(a: &PageLayout, b: &PageLayout) -> bool {
  a.words.len() == b.words.len()
    && a.words.iter().zip(&b.words).all(|(a, b)| {
      a.text == b.text
        && [a.x0 - b.x0, a.y0 - b.y0, a.x1 - b.x1, a.y1 - b.y1]
          .iter()
          .all(|off| off.abs() < 0.5)
    })
}

#[test]
fn a_picture_its_page_turns_and_turns_back_is_read_as_the_page_shows_it() {
  let picture = plain();
  // Painted the usual way on a page shown as it is; and painted a half turn round on a page
  // that says it is shown a half turn round. The two turns undo each other, and both pages
  // show the same picture, the right way up. The turn a page says it is shown at is counted
  // from the page as painted, and a picture is read as it is kept: taken for the same thing,
  // they would have the second page's picture read upside down first.
  let as_kept = read_page_of(&picture, [350, 0, 0, 320, 0, 0], 0);
  let turned_back = read_page_of(&picture, [-350, 0, 0, -320, 350, 320], 180);
  for (which, page) in [
    ("as kept", &as_kept),
    ("turned and turned back", &turned_back),
  ] {
    assert_eq!(
      (times(page, "TOTAL"), times(page, "42")),
      (12, 12),
      "{which}: {:?}",
      page.words
    );
    // Neither picture was turned to be read.
    assert!(page.warnings.is_empty(), "{which}: {:?}", page.warnings);
  }
  // The words are given in the frame the page reads in: as painted for the first, and turned
  // a half turn for the second. In that frame the two pages are the same, their first word at
  // the top left where both show it.
  assert_eq!(
    (as_kept.content_rotation, turned_back.content_rotation),
    (0, 180)
  );
  let head = &as_kept.words[0];
  assert!(
    head.text == "TOTAL" && head.x1 < 175.0 && head.y0 > 160.0,
    "{head:?}"
  );
  assert!(
    same_places(&turned_back, &as_kept),
    "{:?}\n{:?}",
    turned_back.words,
    as_kept.words
  );
}

#[test]
fn a_picture_kept_on_its_side_and_painted_upright_is_read_once_and_set_where_it_shows() {
  let picture = plain();
  let on_its_side = DynamicImage::ImageLuma8(picture.clone())
    .rotate90()
    .into_luma8();
  let as_kept = read_page_of(&picture, [350, 0, 0, 320, 0, 0], 0);
  assert_eq!((times(&as_kept, "TOTAL"), times(&as_kept, "42")), (12, 12));
  // The painting gives the picture the quarter turn it needs: the page as painted reads as it
  // is, and the picture as kept is expected to need that turn, so it is read once.
  let painted_upright = read_page_of(&on_its_side, [0, 320, -350, 0, 350, 0], 0);
  assert_eq!(painted_upright.content_rotation, 0);
  assert_eq!(
    painted_upright.warnings.len(),
    1,
    "{:?}",
    painted_upright.warnings
  );
  assert!(
    painted_upright.warnings[0].ends_with("after a 270° turn (as expected)"),
    "{:?}",
    painted_upright.warnings
  );
  assert!(
    same_places(&painted_upright, &as_kept),
    "{:?}\n{:?}",
    painted_upright.words,
    as_kept.words
  );
}

#[test]
fn a_page_read_in_its_turn_hands_the_turn_it_read_at_to_the_next() {
  // A scan fed in sideways, with nothing in the file to say so: two pages kept on their side,
  // then one kept the right way up. Each is painted the usual way on a page shown as it is.
  let upright = plain();
  let on_its_side = DynamicImage::ImageLuma8(upright.clone())
    .rotate90()
    .into_luma8();
  let (doc, bytes) = document_of(
    &[
      (&on_its_side, [320, 0, 0, 350, 0, 0], 0),
      (&on_its_side, [320, 0, 0, 350, 0, 0], 0),
      (&upright, [350, 0, 0, 320, 0, 0], 0),
    ],
    [350, 350],
  );
  let expected_of = |pages: &[PageLayout]| -> Vec<bool> {
    pages
      .iter()
      .map(|page| {
        page
          .warnings
          .iter()
          .any(|warning| warning.contains("as expected"))
      })
      .collect()
  };
  let texts_of = |pages: &[PageLayout]| -> Vec<Vec<String>> {
    pages
      .iter()
      .map(|page| page.words.iter().map(|word| word.text.clone()).collect())
      .collect()
  };
  let turns_of =
    |pages: &[PageLayout]| -> Vec<u32> { pages.iter().map(|page| page.content_rotation).collect() };

  // One page at a time, in order. The first is searched for its way up, and found to read
  // after a turn of 270. The second is read that way up first, and stands. The third is tried
  // that way too, reads as nothing, and is searched as it would have been.
  let in_turn = extract_layout_with_ocr(&doc, &bytes, "eng", 1, 1, 150, RENDER_MODE_NEVER, None);
  assert_eq!(turns_of(&in_turn), [270, 270, 0]);
  assert_eq!(expected_of(&in_turn), [false, true, false]);
  for page in &in_turn {
    assert_eq!(
      (times(page, "TOTAL"), times(page, "42")),
      (12, 12),
      "page {}: {:?}",
      page.page,
      page.words
    );
  }

  // Several at once: no page waits on another, nothing is handed on, and the words are the same.
  let at_once = extract_layout_with_ocr(&doc, &bytes, "eng", 1, 3, 150, RENDER_MODE_NEVER, None);
  assert_eq!(expected_of(&at_once), [false, false, false]);
  assert_eq!(turns_of(&at_once), [270, 270, 0]);
  assert_eq!(texts_of(&at_once), texts_of(&in_turn));
}

#[test]
fn only_a_page_read_well_from_its_pictures_hands_its_turn_on() {
  let word = |confidence: f64| contentdown::TextWord {
    text: "word".into(),
    x0: 0.0,
    y0: 0.0,
    x1: 10.0,
    y1: 10.0,
    line: 0,
    font_size: None,
    font_name: None,
    bold: None,
    italic: None,
    symbolic: None,
    confidence: Some(confidence),
  };
  let page = |source: TextSource, turn: u32, words: usize, confidence: f64| PageLayout {
    page: 1,
    width: 612.0,
    height: 792.0,
    rotate: 0,
    content_rotation: turn,
    source,
    words: vec![word(confidence); words],
    lines: Vec::new(),
    images: Vec::new(),
    warnings: Vec::new(),
  };
  assert_eq!(turn_to_hand_on(&page(TextSource::Ocr, 270, 40, 0.93)), 270);
  // A page that read well the way it is hands on that nothing is to be expected.
  assert_eq!(turn_to_hand_on(&page(TextSource::Ocr, 0, 40, 0.93)), 0);
  // Too few words, or words too unsure, say too little of how the sheets were fed.
  assert_eq!(turn_to_hand_on(&page(TextSource::Ocr, 270, 9, 0.93)), 0);
  assert_eq!(turn_to_hand_on(&page(TextSource::Ocr, 270, 40, 0.71)), 0);
  // A page with text of its own, or read from a rendering, was not read from a kept picture.
  assert_eq!(turn_to_hand_on(&page(TextSource::Native, 270, 40, 0.93)), 0);
  assert_eq!(
    turn_to_hand_on(&page(TextSource::Rendered, 270, 40, 0.93)),
    0
  );
}
