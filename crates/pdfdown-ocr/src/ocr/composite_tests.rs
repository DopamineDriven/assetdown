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
  let (words, turn) = read_page_rasters(images, "eng", &[], &mut warnings);
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
  let (words, _) = read_page_rasters(images, "eng", &[placement], &mut warnings);
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
