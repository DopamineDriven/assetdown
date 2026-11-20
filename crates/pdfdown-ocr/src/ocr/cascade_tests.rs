use super::*;
use contentdown::TextWord;

fn page(words: &[(f64, f64, f64, f64, &str)], images: &[(f64, f64, f64, f64)]) -> PageLayout {
  PageLayout {
    page: 1,
    width: 612.0,
    height: 792.0,
    rotate: 0,
    content_rotation: 0,
    source: TextSource::Native,
    words: words
      .iter()
      .map(|&(x0, y0, x1, y1, t)| TextWord {
        text: t.into(),
        x0,
        y0,
        x1,
        y1,
        line: 0,
        font_size: None,
        font_name: None,
        bold: None,
        italic: None,
        symbolic: None,
        confidence: None,
      })
      .collect(),
    lines: Vec::new(),
    images: images
      .iter()
      .enumerate()
      .map(|(i, &(x0, y0, x1, y1))| PlacedImage {
        xobject_name: format!("Im{i}"),
        x0,
        y0,
        x1,
        y1,
      })
      .collect(),
    warnings: Vec::new(),
  }
}

fn origin_box(p: &PageLayout) -> [f64; 4] {
  [0.0, 0.0, p.width, p.height]
}

#[test]
fn coverage_is_measured_against_the_page_box_not_an_origin_box() {
  // a CropBox at [400, 400, 1012, 1192] with the scan filling it and two stamped words on it:
  // clipped to an origin box the scan would cover 17% and the page would wrongly stay Native
  let bbox = [400.0, 400.0, 1012.0, 1192.0];
  let p = page(
    &[
      (440.0, 1160.0, 480.0, 1170.0, "PO"),
      (485.0, 1160.0, 525.0, 1170.0, "4711"),
    ],
    &[(406.0, 406.0, 1006.0, 1186.0)],
  );
  assert_eq!(page_needs_ocr(&p, 1, 0, bbox), OcrDecision::Ocr);
  assert_eq!(
    page_needs_ocr(&p, 1, 0, origin_box(&p)),
    OcrDecision::Native
  );
}

#[test]
fn ocr_score_weighs_words_by_confidence_and_weak_tier2_needs_words_and_confidence() {
  let row = |conf: f64| -> Vec<Word> {
    (0..4)
      .map(|i| {
        let x = f64::from(i) * 10.0;
        word(x, 0.0, x + 8.0, 10.0, Some(conf))
      })
      .collect()
  };
  let (strong, faint) = (row(0.9), row(0.2));
  assert!((ocr_score(&strong) - 3.6).abs() < 1e-9);
  assert!(ocr_score(&strong) > ocr_score(&faint));
  assert_eq!(ocr_score(&[]), 0.0);
  assert!(!tier2_is_weak(&strong));
  assert!(
    tier2_is_weak(&faint),
    "a mean confidence under WEAK_TIER2_CONFIDENCE is weak"
  );
  assert!(
    tier2_is_weak(&strong[..2]),
    "fewer than WEAK_TIER2_WORDS words is weak"
  );
  assert!(
    !tier2_is_weak(&strong[..WEAK_TIER2_WORDS]),
    "exactly WEAK_TIER2_WORDS words is not weak, and such an image votes on the page's turn"
  );
}

fn word(x0: f64, y0: f64, x1: f64, y1: f64, confidence: Option<f64>) -> Word {
  Word {
    text: "w".into(),
    x0,
    y0,
    x1,
    y1,
    baseline: y0,
    font_size: None,
    font: None,
    confidence,
  }
}

#[test]
fn coverage_rule_sends_stamped_scan_to_ocr() {
  // two stamped words over a full-page scan: 0.2% word area, 97% image area
  let p = page(
    &[
      (40.0, 760.0, 80.0, 770.0, "PO"),
      (85.0, 760.0, 125.0, 770.0, "4711"),
    ],
    &[(6.0, 6.0, 606.0, 786.0)],
  );
  assert_eq!(page_needs_ocr(&p, 1, 0, origin_box(&p)), OcrDecision::Ocr);
}

#[test]
fn outline_text_with_a_logo_and_a_stamp_is_ocr() {
  // two stamped words, a logo covering 1.6% of the page, and thousands of path operators
  let p = page(
    &[
      (40.0, 760.0, 80.0, 770.0, "PO"),
      (85.0, 760.0, 125.0, 770.0, "4711"),
    ],
    &[(44.0, 695.0, 188.0, 750.0)],
  );
  assert_eq!(
    page_needs_ocr(&p, 1, 50_000, origin_box(&p)),
    OcrDecision::Ocr
  );
}

#[test]
fn ruled_cover_sheet_stays_native() {
  // a few words and a table border (dozens of path operators) is still a native page
  let p = page(
    &[
      (50.0, 700.0, 120.0, 712.0, "Cover"),
      (125.0, 700.0, 180.0, 712.0, "sheet"),
    ],
    &[],
  );
  assert_eq!(
    page_needs_ocr(&p, 1, 80, origin_box(&p)),
    OcrDecision::Native
  );
}

#[test]
fn coverage_rule_keeps_sparse_cover_sheet_native() {
  let p = page(
    &[
      (50.0, 700.0, 120.0, 712.0, "Cover"),
      (125.0, 700.0, 180.0, 712.0, "sheet"),
      (50.0, 680.0, 90.0, 692.0, "for"),
      (95.0, 680.0, 160.0, 692.0, "invoice"),
      (165.0, 680.0, 220.0, 692.0, "bundle"),
    ],
    &[],
  );
  assert_eq!(
    page_needs_ocr(&p, 1, 0, origin_box(&p)),
    OcrDecision::Native
  );
}

#[test]
fn min_text_length_still_forces_ocr() {
  let p = page(&[(50.0, 700.0, 120.0, 712.0, "Cover")], &[]);
  assert_eq!(page_needs_ocr(&p, 999, 0, origin_box(&p)), OcrDecision::Ocr);
}

#[test]
fn outline_page_goes_to_render() {
  let p = page(&[], &[]);
  assert_eq!(
    page_needs_ocr(&p, 1, 5_000, origin_box(&p)),
    OcrDecision::RenderOnly
  );
  assert_eq!(page_needs_ocr(&p, 1, 0, origin_box(&p)), OcrDecision::Ocr);
}

#[test]
fn merge_drops_ocr_duplicates_of_native_words() {
  let native = vec![word(0.0, 0.0, 10.0, 10.0, None)];
  let ocr = vec![
    word(1.0, 1.0, 10.0, 10.0, Some(0.9)),
    word(50.0, 50.0, 60.0, 60.0, Some(0.8)),
  ];
  let (merged, dropped) = merge_native_and_ocr(native, ocr);
  assert_eq!(dropped, 1);
  assert_eq!(merged.len(), 2);
  assert_eq!(merged[0].confidence, None, "native word first, kept as is");
  assert_eq!(
    merged[1].confidence,
    Some(0.8),
    "the disjoint OCR word survives"
  );
}

/// A document of one page whose own content has no text: it draws a form, and "Invoice 42" is
/// set inside the form, in a font the form brings with it.
fn text_inside_a_form() -> Document {
  use lopdf::content::{Content, Operation};
  use lopdf::{Object, Stream, dictionary};

  let op = |name: &str, operands: Vec<Object>| Operation::new(name, operands);
  let mut doc = Document::with_version("1.7");
  let pages_id = doc.new_object_id();
  let font = doc
    .add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
  let form_ops = vec![
    op("BT", vec![]),
    op("Tf", vec!["F9".into(), 10.into()]),
    op("Td", vec![100.into(), 700.into()]),
    op("Tj", vec![Object::string_literal("Invoice 42")]),
    op("ET", vec![]),
  ];
  let form = doc.add_object(Stream::new(
    dictionary! {
      "Type" => "XObject", "Subtype" => "Form",
      "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
      "Resources" => Object::Dictionary(dictionary! {
        "Font" => Object::Dictionary(dictionary! { "F9" => Object::Reference(font) }),
      }),
    },
    Content {
      operations: form_ops,
    }
    .encode()
    .unwrap(),
  ));
  let draws = Content {
    operations: vec![
      op("q", vec![]),
      op("Do", vec!["Fx".into()]),
      op("Q", vec![]),
    ],
  };
  let content = doc.add_object(Stream::new(dictionary! {}, draws.encode().unwrap()));
  let page = doc.add_object(dictionary! {
    "Type" => "Page", "Parent" => Object::Reference(pages_id),
    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    "Contents" => Object::Reference(content),
    "Resources" => Object::Dictionary(dictionary! {
      "XObject" => Object::Dictionary(dictionary! { "Fx" => Object::Reference(form) }),
    }),
  });
  doc.objects.insert(
    pages_id,
    Object::Dictionary(
      dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1 },
    ),
  );
  let catalog =
    doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
  doc.trailer.set("Root", Object::Reference(catalog));
  doc
}

#[test]
fn a_page_whose_text_is_inside_a_form_gives_that_text_and_is_not_recognized() {
  let doc = text_inside_a_form();
  let pages = extract_text_with_ocr(&doc, &[], "eng", 5, 1, 300, RENDER_MODE_NEVER);
  let [page] = pages.as_slice() else {
    panic!("one page was made, and {} came back", pages.len());
  };
  assert_eq!((page.page, page.text.as_str()), (1, "Invoice 42"));
  assert_eq!(page.source, TextSource::Native);
}
