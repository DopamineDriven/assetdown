use super::*;
use pdfdown::images::collect_page_decoded_images_named;

/// The raster on the first page of this crate's fixture: 800 by 200 pixels of dark type on
/// white, reading "Page One Typed Text". It stands in for one-bit images already merged, which
/// `pdfdown` tests on its own.
fn typed_raster() -> image::GrayImage {
  let bytes = std::fs::read(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/multipage-ocr.pdf"
  ))
  .unwrap();
  let doc = Document::load_mem(&bytes).unwrap();
  let page_id = *doc.get_pages().get(&1).expect("a first page");
  let (_, raster) = collect_page_decoded_images_named(&doc, page_id).remove(0);
  raster.into_luma8()
}

fn merged(bbox: [f64; 4]) -> OneBitComposite {
  OneBitComposite {
    image: typed_raster(),
    bbox,
    tone: InkTone::Dark,
    pieces: 3,
  }
}

const TYPED: [&str; 4] = ["Page", "One", "Typed", "Text"];

#[test]
fn a_merged_raster_is_read_and_its_words_land_on_the_area_it_covers() {
  // No image placement is involved: the raster states the area it covers.
  let images = OcrImages {
    named: Vec::new(),
    composites: vec![merged([100.0, 500.0, 300.0, 550.0])],
  };
  let mut warnings = Vec::new();
  let (words, turn) = read_page_rasters(images, "eng", &[], &[], 0, &mut warnings);
  let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
  for word in TYPED {
    assert!(texts.contains(&word), "{word} is missing from {texts:?}");
  }
  assert!(
    words
      .iter()
      .all(|w| w.x0 >= 100.0 && w.x1 <= 300.0 && w.y0 >= 500.0 && w.y1 <= 550.0),
    "{words:?}"
  );
  assert_eq!(turn, 0);
  assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_merged_raster_is_read_beside_the_images_left_by_name() {
  let placement = PlacedImage {
    xobject_name: "Im0".to_string(),
    x0: 0.0,
    y0: 0.0,
    x1: 300.0,
    y1: 75.0,
  };
  let images = OcrImages {
    named: vec![("Im0".to_string(), DynamicImage::ImageLuma8(typed_raster()))],
    composites: vec![merged([0.0, 100.0, 300.0, 175.0])],
  };
  let mut warnings = Vec::new();
  let (words, _) = read_page_rasters(images, "eng", &[placement], &[], 0, &mut warnings);
  let read_between = |y0: f64, y1: f64| {
    words
      .iter()
      .filter(|w| w.y0 >= y0 && w.y1 <= y1)
      .map(|w| w.text.as_str())
      .collect::<Vec<_>>()
  };
  for word in TYPED {
    assert!(read_between(0.0, 75.0).contains(&word), "{words:?}");
    assert!(read_between(100.0, 175.0).contains(&word), "{words:?}");
  }
  assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_warning_names_a_merged_raster_by_what_it_holds() {
  let composite = |tone, pieces| OneBitComposite {
    tone,
    pieces,
    ..merged([0.0, 0.0, 1.0, 1.0])
  };
  assert_eq!(
    composite_subject(&composite(InkTone::Dark, 14)),
    "14 one-bit images"
  );
  assert_eq!(
    composite_subject(&composite(InkTone::Light, 1)),
    "1 one-bit image painted light"
  );
}

/// A page of the typed raster three times over, laid on its side as a scanner keeps a sheet
/// fed in sideways: it reads again after a clockwise turn of 270.
fn page_on_its_side(bbox: [f64; 4]) -> OneBitComposite {
  let line = typed_raster();
  let mut page = image::GrayImage::from_pixel(line.width(), line.height() * 3, image::Luma([255]));
  for at in 0..3 {
    image::imageops::replace(&mut page, &line, 0, i64::from(at * line.height()));
  }
  OneBitComposite {
    image: DynamicImage::ImageLuma8(page).rotate90().into_luma8(),
    bbox,
    tone: InkTone::Dark,
    pieces: 1,
  }
}

#[test]
fn a_page_kept_on_its_side_reads_at_the_turn_its_page_is_shown_at() {
  let read = |page_turn: u32| {
    let images = OcrImages {
      named: Vec::new(),
      composites: vec![page_on_its_side([100.0, 300.0, 250.0, 500.0])],
    };
    let mut warnings = Vec::new();
    let (words, turn) = read_page_rasters(images, "eng", &[], &[], page_turn, &mut warnings);
    let texts: Vec<String> = words.into_iter().map(|w| w.text).collect();
    (texts, turn, warnings)
  };
  // The page says it is shown turned 270: the picture is read that way up first.
  let (texts, turn, warnings) = read(270);
  assert_eq!(turn, 270);
  for word in TYPED {
    let times = texts.iter().filter(|text| *text == word).count();
    assert_eq!(times, 3, "{word} in {texts:?}");
  }
  assert_eq!(warnings.len(), 1, "{warnings:?}");
  assert!(
    warnings[0].ends_with("after a 270° turn (as expected)"),
    "{warnings:?}"
  );
  // A page that says nothing comes to the same words by the longer way round.
  let (unhinted, turn, warnings) = read(0);
  assert_eq!(turn, 270);
  assert_eq!(unhinted, texts);
  assert!(!warnings[0].contains("as expected"), "{warnings:?}");
}

#[test]
fn an_image_its_placement_turns_is_not_read_at_the_turn_its_page_is_shown_at() {
  // The same picture by name, as an image the page paints through a matrix. The turn a page is
  // shown at is counted from the page as painted, and an image is read as it is kept: the two
  // are the same thing only when the painting does not turn the image.
  let read = |turned: &[String]| {
    let images = OcrImages {
      named: vec![(
        "Im0".to_string(),
        DynamicImage::ImageLuma8(page_on_its_side([0.0; 4]).image),
      )],
      composites: Vec::new(),
    };
    let placement = PlacedImage {
      xobject_name: "Im0".to_string(),
      x0: 100.0,
      y0: 300.0,
      x1: 250.0,
      y1: 500.0,
    };
    let mut warnings = Vec::new();
    let (words, turn) = read_page_rasters(images, "eng", &[placement], turned, 270, &mut warnings);
    (words.len(), turn, warnings)
  };
  // Painted as it is kept: the page's turn is what it is expected to need.
  let (kept_words, turn, warnings) = read(&[]);
  assert_eq!(turn, 270);
  assert!(warnings[0].ends_with("(as expected)"), "{warnings:?}");
  // Painted turned: nothing is expected of it, and it is searched as it always was.
  let (turned_words, turn, warnings) = read(&["Im0".to_string()]);
  assert_eq!(turn, 270);
  assert_eq!(turned_words, kept_words);
  assert!(!warnings[0].contains("as expected"), "{warnings:?}");
}
