//! A picture that reads both ways up, in a page that turns it: the picture tier must read it
//! the way the page shows it, whatever turn the page says it is shown at.

use lopdf::content::{Content, Operation};
use lopdf::{Object, Stream, dictionary};
use ocrdown::OrientationSource;

use super::*;

/// Five dots across and seven down, a `#` where there is ink. `6` and `9` are each other
/// turned a half turn, and `0` is itself, so a row of them reads as digits either way up.
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
    '6' => [
      ".###.", "#....", "#....", "####.", "#...#", "#...#", ".###.",
    ],
    '9' => [
      ".###.", "#...#", "#...#", ".####", "....#", "....#", ".###.",
    ],
    '0' => [
      ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
    ],
    ' ' => ["....."; 7],
    other => panic!("no glyph is drawn for {other:?}"),
  }
}

/// How many rows of the grid there are, and how many numbers in a row.
const ROWS: u32 = 6;
const ACROSS: usize = 4;

/// A white picture with `TOTAL 42` at its head and a grid of `606` under it, in black.
fn bill() -> image::GrayImage {
  const DOT: u32 = 5;
  let mut picture = image::GrayImage::from_pixel(700, 640, image::Luma([255]));
  let mut inked = |text: &str, at: (u32, u32)| {
    for (place, letter) in (0u32..).zip(text.chars()) {
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
  };
  inked("TOTAL 42", (60, 40));
  let row = ["606"; ACROSS].join(" ");
  for line in 0..ROWS {
    inked(&row, (60, 130 + line * 80));
  }
  picture
}

/// A page of one picture, painted through `matrix`, that says it is shown turned by `rotate`.
fn page_of(picture: &image::GrayImage, matrix: [i64; 6], rotate: i64) -> (Document, Vec<u8>) {
  let mut doc = Document::with_version("1.7");
  let pages_id = doc.new_object_id();
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
    "MediaBox" => vec![0.into(), 0.into(), 350.into(), 320.into()],
    "Rotate" => rotate,
    "Contents" => Object::Reference(content_id),
    "Resources" => Object::Dictionary(dictionary! {
      "XObject" => Object::Dictionary(dictionary! { "Im0" => Object::Reference(image) }),
    }),
  });
  doc.objects.insert(
    pages_id,
    Object::Dictionary(
      dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 },
    ),
  );
  let catalog =
    doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
  doc.trailer.set("Root", Object::Reference(catalog));
  let mut bytes = Vec::new();
  doc.save_to(&mut bytes).unwrap();
  (doc, bytes)
}

fn times(texts: &[String], word: &str) -> usize {
  texts.iter().filter(|text| *text == word).count()
}

#[test]
fn the_picture_reads_convincingly_either_way_up() {
  // What the test below rests on: turned upside down, this picture is not noise. Its grid
  // reads as `909`, surely enough that a search which read it that way up first would stop.
  let upside_down = DynamicImage::ImageLuma8(bill()).rotate180();
  let found = ocrdown::recognize_words_upright(&upside_down, "eng", Some(144));
  assert_eq!(
    (found.degrees, found.decided),
    (0, OrientationSource::Upright)
  );
  let texts: Vec<String> = found.words.into_iter().map(|word| word.text).collect();
  assert!(times(&texts, "909") >= 10, "{texts:?}");
}

#[test]
fn a_picture_its_page_turns_and_turns_back_is_read_as_the_page_shows_it() {
  let picture = bill();
  let grid = usize::try_from(ROWS).unwrap() * ACROSS;
  // Painted the usual way on a page shown as it is; and painted a half turn round on a page
  // that says it is shown a half turn round. The two turns undo each other, and both pages
  // show the same picture, the right way up.
  let pages = [
    ("as kept", page_of(&picture, [350, 0, 0, 320, 0, 0], 0)),
    (
      "turned and turned back",
      page_of(&picture, [-350, 0, 0, -320, 350, 320], 180),
    ),
  ];
  for (which, (doc, bytes)) in pages {
    let layout = extract_layout_with_ocr(&doc, &bytes, "eng", 1, 1, 150, RENDER_MODE_NEVER, None);
    let [page] = layout.as_slice() else {
      panic!("{which}: one page was made, and {} came back", layout.len());
    };
    assert_eq!(page.source, TextSource::Ocr, "{which}");
    let texts: Vec<String> = page.words.iter().map(|word| word.text.clone()).collect();
    assert_eq!(times(&texts, "606"), grid, "{which}: {texts:?}");
    assert_eq!(times(&texts, "909"), 0, "{which}: {texts:?}");
    assert_eq!(
      (times(&texts, "TOTAL"), times(&texts, "42")),
      (1, 1),
      "{which}: {texts:?}"
    );
    assert!(
      page
        .warnings
        .iter()
        .all(|warning| !warning.contains("as expected")),
      "{which}: {:?}",
      page.warnings
    );
  }
}
