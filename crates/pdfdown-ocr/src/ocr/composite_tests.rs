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

/// The same page the right way up: the typed raster three times over.
fn page_upright() -> image::GrayImage {
  let line = typed_raster();
  let mut page = image::GrayImage::from_pixel(line.width(), line.height() * 3, image::Luma([255]));
  for at in 0..3 {
    image::imageops::replace(&mut page, &line, 0, i64::from(at * line.height()));
  }
  page
}

/// The words of a page that paints one image, by name, through a matrix; with them the page's
/// turn and the warnings.
fn read_painted(
  image: image::GrayImage,
  matrix: [f64; 6],
  page_turn: u32,
) -> (Vec<Word>, u32, Vec<String>) {
  use pdfdown::layout::Placement;

  let placement = Placement::new(matrix);
  let corners =
    [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(u, v)| placement.onto_page(u, v));
  let xs = corners.map(|corner| corner.0);
  let ys = corners.map(|corner| corner.1);
  let placed = PlacedImage {
    xobject_name: "Im0".to_string(),
    x0: xs.iter().copied().fold(f64::INFINITY, f64::min),
    y0: ys.iter().copied().fold(f64::INFINITY, f64::min),
    x1: xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    y1: ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
  };
  let images = OcrImages {
    named: vec![("Im0".to_string(), DynamicImage::ImageLuma8(image))],
    composites: Vec::new(),
  };
  let mut warnings = Vec::new();
  let (words, turn) = read_page_rasters(
    images,
    "eng",
    &[placed],
    &[placement],
    page_turn,
    &mut warnings,
  );
  (words, turn, warnings)
}

/// Whether two readings put the same words in the same places, to within half a point.
fn same_places(a: &[Word], b: &[Word]) -> bool {
  a.len() == b.len()
    && a.iter().zip(b).all(|(a, b)| {
      a.text == b.text
        && [a.x0 - b.x0, a.y0 - b.y0, a.x1 - b.x1, a.y1 - b.y1]
          .iter()
          .all(|off| off.abs() < 0.5)
    })
}

#[test]
fn an_image_kept_turned_and_painted_upright_is_read_once_and_set_where_it_shows() {
  // The page shows the same thing three ways: the picture as it is kept, painted the usual
  // way; the picture kept on its side and painted a quarter turn round; and the picture kept
  // upside down and painted a half turn round. Each fills the box from (100, 300) to (300, 450).
  let upright = page_upright();
  let turned = |image: &image::GrayImage, quarter_turns: u32| {
    let image = DynamicImage::ImageLuma8(image.clone());
    match quarter_turns {
      1 => image.rotate90(),
      2 => image.rotate180(),
      _ => image.rotate270(),
    }
    .into_luma8()
  };
  let (usual, turn, warnings) =
    read_painted(upright.clone(), [200.0, 0.0, 0.0, 150.0, 100.0, 300.0], 0);
  assert_eq!(turn, 0);
  assert!(warnings.is_empty(), "{warnings:?}");
  for word in TYPED {
    let times = usual.iter().filter(|found| found.text == word).count();
    assert_eq!(times, 3, "{word} in {usual:?}");
  }
  // The first line is the highest on the page, and a line reads from left to right.
  let (first, last) = (&usual[0], &usual[usual.len() - 1]);
  assert!(first.y0 > last.y1, "{usual:?}");
  assert!(
    first.x1 < usual[1].x0 && usual[1].x1 < usual[2].x0,
    "{usual:?}"
  );

  // Kept on its side (it reads after a turn of 270), painted with that turn.
  let (on_side, turn, warnings) = read_painted(
    turned(&upright, 1),
    [0.0, 150.0, -200.0, 0.0, 300.0, 300.0],
    0,
  );
  assert_eq!(turn, 0, "the page as painted reads as it is");
  assert_eq!(warnings.len(), 1, "{warnings:?}");
  assert!(
    warnings[0].ends_with("after a 270° turn (as expected)"),
    "{warnings:?}"
  );
  assert!(same_places(&on_side, &usual), "{on_side:?}\n{usual:?}");

  // Kept on its other side (it reads after a turn of 90), painted with that turn.
  let (other_side, turn, warnings) = read_painted(
    turned(&upright, 3),
    [0.0, -150.0, 200.0, 0.0, 100.0, 450.0],
    0,
  );
  assert_eq!(turn, 0);
  assert!(
    warnings[0].ends_with("after a 90° turn (as expected)"),
    "{warnings:?}"
  );
  assert!(
    same_places(&other_side, &usual),
    "{other_side:?}\n{usual:?}"
  );

  // Kept upside down, painted a half turn round.
  let (upside_down, turn, warnings) = read_painted(
    turned(&upright, 2),
    [-200.0, 0.0, 0.0, -150.0, 300.0, 450.0],
    0,
  );
  assert_eq!(turn, 0);
  assert!(
    warnings[0].ends_with("after a 180° turn (as expected)"),
    "{warnings:?}"
  );
  assert!(
    same_places(&upside_down, &usual),
    "{upside_down:?}\n{usual:?}"
  );
}

#[test]
fn an_image_painted_mirrored_is_flipped_before_it_is_read() {
  // Kept flipped left to right, as a scanner's driver sometimes keeps a picture, and painted
  // through a matrix that flips it back. No turn reads mirrored text, so nothing was read of
  // such an image before.
  let upright = page_upright();
  let flipped = DynamicImage::ImageLuma8(upright.clone())
    .fliph()
    .into_luma8();
  let (usual, _, _) = read_painted(upright, [200.0, 0.0, 0.0, 150.0, 100.0, 300.0], 0);
  let (mirrored, turn, warnings) =
    read_painted(flipped, [-200.0, 0.0, 0.0, 150.0, 300.0, 300.0], 0);
  assert_eq!(turn, 0);
  assert!(warnings.is_empty(), "{warnings:?}");
  assert!(same_places(&mirrored, &usual), "{mirrored:?}\n{usual:?}");
}

#[test]
fn a_turned_image_votes_for_the_turn_the_page_as_painted_needs() {
  // The picture as it is kept reads upright, and the page paints it a half turn round: the
  // page as painted reads upside down, which is what its turn has to say, and the words are
  // where the painting put them.
  let upright = page_upright();
  let (usual, _, _) = read_painted(upright.clone(), [200.0, 0.0, 0.0, 150.0, 100.0, 300.0], 0);
  let (half, turn, warnings) = read_painted(upright, [-200.0, 0.0, 0.0, -150.0, 300.0, 450.0], 180);
  assert_eq!(turn, 180);
  assert!(
    warnings.is_empty(),
    "nothing was turned to read it: {warnings:?}"
  );
  // Each word is where the usual painting has it, turned a half turn about the box's middle.
  let about_middle: Vec<Word> = usual
    .iter()
    .map(|word| Word {
      x0: 400.0 - word.x1,
      x1: 400.0 - word.x0,
      y0: 750.0 - word.y1,
      y1: 750.0 - word.y0,
      ..word.clone()
    })
    .collect();
  assert!(
    same_places(&half, &about_middle),
    "{half:?}\n{about_middle:?}"
  );
}
