use super::*;
use crate::testing::office_package;
use crate::{Evidence, Facts, sniff, specs, specs_source};

const WORD: &str =
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

fn document(found: crate::Specs) -> DocumentFacts {
  match found.facts {
    Facts::Document(facts) => *facts,
    other => panic!(
      "no document facts for {}: {other:?} {:?}",
      found.media_type, found.warnings
    ),
  }
}

fn detail(facts: &DocumentFacts) -> &OfficeDetail {
  match &facts.detail {
    DocumentDetail::Office(detail) => detail,
    other => panic!("not an Office file's detail: {other:?}"),
  }
}

#[test]
fn the_main_part_says_exactly_what_kind_of_file_it_is() {
  let cases = [
    ("word/document.xml", WORD, MediaType::Docx),
    (
      "word/document.xml",
      "application/vnd.ms-word.document.macroEnabled.main+xml",
      MediaType::Docm,
    ),
    (
      "word/document.xml",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
      MediaType::Dotx,
    ),
    (
      "xl/workbook.xml",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
      MediaType::Xlsx,
    ),
    (
      "xl/workbook.xml",
      "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
      MediaType::Xlsm,
    ),
    (
      "xl/workbook.xml",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
      MediaType::Xltx,
    ),
    (
      "ppt/presentation.xml",
      "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
      MediaType::Pptx,
    ),
    (
      "ppt/presentation.xml",
      "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
      MediaType::Pptm,
    ),
    (
      "ppt/presentation.xml",
      "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
      MediaType::Potx,
    ),
    (
      "ppt/presentation.xml",
      "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml",
      MediaType::Ppsx,
    ),
  ];
  for (main, main_type, media_type) in cases {
    let found = sniff(&office_package(main, main_type));
    assert_eq!(found.media_type(), media_type, "{main_type}");
    assert!(matches!(
      found,
      crate::Sniffed::Named {
        evidence: Evidence::Container,
        ..
      }
    ));
  }
}

#[test]
fn what_the_file_says_of_itself_is_read_from_its_properties() {
  let found = specs(&office_package(
    "word/document.xml",
    "application/vnd.ms-word.document.macroEnabled.main+xml",
  ));
  assert_eq!(found.media_type, MediaType::Docm);
  let facts = document(found);
  assert_eq!(
    (facts.title.as_deref(), facts.author.as_deref()),
    (Some("A Harbor at Dusk"), Some("First Maker"))
  );
  assert_eq!(facts.subject.as_deref(), Some("Light on water"));
  // Written as one string, parted by commas and semicolons.
  assert_eq!(facts.keywords, ["harbor", "dusk", "water"]);
  assert_eq!(
    (
      facts.created_date.as_deref(),
      facts.modified_date.as_deref()
    ),
    (Some("2026-11-17T23:36:27Z"), Some("2026-11-18T08:00:00Z"))
  );
  let detail = detail(&facts);
  assert_eq!((detail.macro_enabled, detail.template), (true, false));
  assert_eq!(detail.application.as_deref(), Some("Made By Hand"));
}

#[cfg(not(feature = "office"))]
#[test]
fn without_the_parsers_the_counts_are_the_ones_the_application_wrote_down() {
  let found = specs(&office_package("word/document.xml", WORD));
  assert_eq!(found.warnings.len(), 0);
  let facts = document(found);
  assert_eq!(
    (
      facts.page_count,
      facts.word_count,
      facts.language.as_deref()
    ),
    (Some(3), Some(250), Some("en-US"))
  );
  assert_eq!((facts.image_count, &facts.text_preview), (None, &None));
}

#[test]
fn a_template_and_a_slide_show_say_so() {
  let template = document(specs(&office_package(
    "xl/workbook.xml",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
  )));
  assert_eq!(
    (detail(&template).template, detail(&template).macro_enabled),
    (true, false)
  );
  let show = document(specs(&office_package(
    "ppt/presentation.xml",
    "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml",
  )));
  assert!(detail(&show).template);
}

#[test]
fn part_of_a_package_names_it_and_asks_for_the_rest() {
  let file = office_package("word/document.xml", WORD);
  let size = u64::try_from(file.len()).unwrap();
  // The whole file but its last byte: every name is reached, and nothing can be opened.
  let pieces = [crate::Piece {
    offset: 0,
    bytes: &file[..file.len() - 1],
  }];
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(
    (found.media_type, &found.facts),
    (MediaType::Docx, &Facts::None)
  );
  assert_eq!(
    found.needs,
    Some(crate::ByteRange {
      offset: size - 1,
      length: 1,
      from_end: false
    })
  );
}

#[cfg(feature = "office")]
mod with_the_parsers {
  use super::*;

  /// A fixture another crate of the workspace tracks.
  fn fixture(path: &str) -> Vec<u8> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    std::fs::read(format!("{root}/{path}")).unwrap_or_else(|error| panic!("{path}: {error}"))
  }

  #[test]
  fn a_word_document_has_the_counts_its_parser_gives() {
    let bytes = fixture("docxdown-node/__test__/fixtures/Poem-29.docx");
    let found = specs(&bytes);
    assert_eq!(
      (found.media_type, found.warnings.len()),
      (MediaType::Docx, 0)
    );
    let facts = document(found);
    let theirs = docxdown::Document::open(bytes).unwrap();
    let meta = theirs.metadata();
    assert_eq!(facts.page_count, Some(meta.page_count));
    assert_eq!(facts.word_count, Some(u64::from(meta.word_count)));
    assert_eq!(
      (facts.image_count, facts.annot_count),
      (Some(meta.image_count), Some(meta.annotation_count))
    );
    assert!(meta.page_count >= 1 && meta.word_count > 0);
    assert!(facts.page_width.is_some() && facts.paper_size.is_some());
    let preview = facts.text_preview.unwrap();
    assert!(theirs.text().trim_start().starts_with(&preview));
  }

  #[test]
  fn a_presentation_s_pages_are_its_slides() {
    let bytes = fixture("pptxdown-node/__test__/fixtures/Activity-7.pptx");
    let found = specs(&bytes);
    assert_eq!(
      (found.media_type, found.warnings.len()),
      (MediaType::Pptx, 0)
    );
    let facts = document(found);
    let meta = pptxdown::Presentation::open(bytes).unwrap().metadata();
    assert_eq!(facts.page_count, Some(meta.slide_count));
    assert_eq!(
      (facts.page_width, facts.page_height),
      (Some(meta.width), Some(meta.height))
    );
    assert_eq!(facts.page_size_uniform, Some(true));
    assert!(meta.slide_count >= 1);
  }

  #[test]
  fn a_workbook_s_pages_are_its_sheets_and_its_preview_its_first_words() {
    let bytes = fixture("xlsxdown/tests/fixtures/form.xlsx");
    let found = specs(&bytes);
    assert_eq!(
      (found.media_type, found.warnings.len()),
      (MediaType::Xlsx, 0)
    );
    let facts = document(found);
    let meta = xlsxdown::Workbook::open(bytes).unwrap().metadata();
    assert_eq!(facts.page_count, Some(meta.sheet_count));
    assert_eq!(facts.word_count, Some(u64::from(meta.word_count)));
    assert!(facts.text_preview.is_some());
  }

  #[test]
  fn a_macro_enabled_workbook_is_named_as_one_and_says_its_parser_turned_it_away() {
    let bytes = fixture("xlsxdown/tests/fixtures/macro.xlsm");
    let found = specs(&bytes);
    assert_eq!(found.media_type, MediaType::Xlsm);
    assert_eq!(found.warnings.len(), 1);
    assert!(
      found.warnings[0].starts_with("the workbook could not be read"),
      "{:?}",
      found.warnings
    );
    assert!(detail(&document(found)).macro_enabled);
  }
}
