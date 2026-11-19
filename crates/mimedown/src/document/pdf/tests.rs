use super::*;
use crate::source::Piece;
use crate::testing::pdf;
use crate::{Facts, MediaType, specs, specs_source};

fn document(found: crate::Specs) -> DocumentFacts {
  match found.facts {
    Facts::Document(facts) => *facts,
    other => panic!("no document facts for {}: {other:?}", found.media_type),
  }
}

fn detail(facts: &DocumentFacts) -> &PdfDetail {
  match &facts.detail {
    DocumentDetail::Pdf(detail) => detail,
    other => panic!("not a PDF's detail: {other:?}"),
  }
}

#[test]
fn the_first_line_gives_the_version() {
  let found = specs(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n1 0 obj\n<< >>\nendobj\n");
  assert_eq!(found.media_type, MediaType::Pdf);
  let facts = document(found);
  assert_eq!(detail(&facts).version.as_deref(), Some("1.4"));
  assert_eq!(
    document(specs(b"%PDF-2.0\n")).detail,
    DocumentDetail::Pdf(PdfDetail {
      version: Some("2.0".to_string()),
      is_linearized: None,
      is_encrypted: None,
      is_searchable: None,
      creator: None,
      producer: None,
      has_form: None,
    })
  );
}

#[test]
fn a_file_laid_out_for_the_network_states_its_page_count_in_its_first_object() {
  let head = b"%PDF-1.6\n%\xE2\xE3\xCF\xD3\n1 0 obj\n<</Linearized 1/L 91234/O 3/E 5000/N 42/T 90000/H [ 500 200 ]>>\nendobj\n";
  let pieces = [Piece {
    offset: 0,
    bytes: head,
  }];
  let facts = document(specs_source(
    &Source::from_pieces(&pieces, Some(91_234)).unwrap(),
  ));
  assert_eq!(facts.page_count, Some(42));
  assert_eq!(detail(&facts).is_linearized, Some(true));
  assert_eq!((facts.word_count, &facts.text_preview), (None, &None));
}

#[cfg(not(feature = "pdf"))]
#[test]
fn without_a_reader_the_whole_file_says_no_more_than_its_head_and_nothing_is_asked_for() {
  let file = pdf(
    &["Hello from the first page"],
    &[("Creator", "made by hand")],
  );
  let found = specs(&file);
  assert_eq!((found.needs, found.warnings.len()), (None, 0));
  let facts = document(found);
  assert_eq!(detail(&facts).version.as_deref(), Some("1.7"));
  assert_eq!((facts.page_count, facts.word_count), (None, None));
}

#[cfg(feature = "pdf")]
mod with_a_reader {
  use super::*;
  use crate::source::ByteRange;

  #[test]
  fn the_whole_file_gives_its_pages_its_words_and_who_made_it() {
    let info = [
      ("Creator", "made by hand"),
      ("Producer", "mimedown's tests"),
      ("CreationDate", "D:20261117233627-05'00'"),
    ];
    let file = pdf(
      &["Hello from the first page", "and five words more here"],
      &info,
    );
    let found = specs(&file);
    assert_eq!((found.needs, &found.warnings), (None, &Vec::new()));
    let facts = document(found);
    assert_eq!((facts.page_count, facts.word_count), (Some(2), Some(10)));
    assert_eq!(
      facts.text_preview.as_deref(),
      Some("Hello from the first page and five words more here")
    );
    assert_eq!(
      (facts.page_width, facts.page_height),
      (Some(612.0), Some(792.0))
    );
    assert_eq!(
      (facts.paper_size.as_deref(), facts.page_size_uniform),
      (Some("LETTER"), Some(true))
    );
    assert_eq!(
      facts.created_date.as_deref(),
      Some("2026-11-17T23:36:27-05:00")
    );
    assert_eq!(facts.annot_count, Some(0));
    let detail = detail(&facts);
    assert_eq!(
      (detail.version.as_deref(), detail.is_linearized),
      (Some("1.7"), Some(false))
    );
    assert_eq!(
      (detail.is_searchable, detail.has_form),
      (Some(true), Some(false))
    );
    assert_eq!(
      (detail.creator.as_deref(), detail.producer.as_deref()),
      (Some("made by hand"), Some("mimedown's tests"))
    );
    // What the file does not say of itself is left unsaid.
    assert_eq!(
      (&facts.title, &facts.author, &facts.subject, &facts.language),
      (&None, &None, &None, &None)
    );
    assert_eq!(facts.keywords, Vec::<String>::new());
    assert_eq!(
      (facts.image_count, detail.is_encrypted),
      (Some(0), Some(false))
    );
  }

  #[test]
  fn what_the_document_says_of_itself_is_read() {
    let info = [
      ("Title", "A Harbor at Dusk"),
      ("Author", "Ada Marsh"),
      ("Subject", "Notes on light"),
      ("Keywords", "harbor, dusk; light"),
    ];
    let file = crate::testing::pdf_with(&["a page"], &info, "/Lang (en-GB)");
    let found = specs(&file);
    assert_eq!(found.warnings, Vec::<String>::new());
    let facts = document(found);
    assert_eq!(facts.title.as_deref(), Some("A Harbor at Dusk"));
    assert_eq!(facts.author.as_deref(), Some("Ada Marsh"));
    assert_eq!(facts.subject.as_deref(), Some("Notes on light"));
    // One string in the file, given as the list it is.
    assert_eq!(facts.keywords, ["harbor", "dusk", "light"]);
    assert_eq!(facts.language.as_deref(), Some("en-GB"));
  }

  #[test]
  fn the_pictures_a_document_paints_are_counted() {
    // A scan of one page, which `pdfdown-node` keeps among its fixtures.
    let path = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../pdfdown-node/__test__/fixtures/ascii85-dct-scan.pdf"
    );
    let facts = document(specs(&std::fs::read(path).unwrap()));
    assert_eq!((facts.page_count, facts.image_count), (Some(1), Some(1)));
  }

  /// The file locked: `password` is what a reader has to give to open it, and nothing when
  /// it is locked only against being changed.
  fn locked(file: &[u8], password: &str) -> Vec<u8> {
    use pdfdown::lopdf::{
      Document, EncryptionState, EncryptionVersion, Object, Permissions, StringFormat,
    };
    let mut document = Document::load_mem(file).unwrap();
    let id = Object::String(b"0123456789abcdef".to_vec(), StringFormat::Literal);
    document.trailer.set("ID", vec![id.clone(), id]);
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
      document: &document,
      owner_password: "owner",
      user_password: password,
      key_length: 128,
      permissions: Permissions::all(),
    })
    .unwrap();
    document.encrypt(&state).unwrap();
    let mut locked = Vec::new();
    document.save_to(&mut locked).unwrap();
    locked
  }

  #[test]
  fn a_file_encrypted_and_open_to_anyone_is_read_and_says_it_was_encrypted() {
    let file = locked(
      &pdf(&["five words on the page"], &[("Title", "Under lock")]),
      "",
    );
    let found = specs(&file);
    assert_eq!(found.warnings, Vec::<String>::new());
    let facts = document(found);
    assert_eq!(detail(&facts).is_encrypted, Some(true));
    assert_eq!(facts.title.as_deref(), Some("Under lock"));
    assert_eq!((facts.page_count, facts.word_count), (Some(1), Some(5)));
  }

  #[test]
  fn a_file_that_asks_for_a_password_says_so_and_gives_no_counts_of_what_it_could_not_read() {
    let file = locked(
      &pdf(&["five words on the page"], &[("Title", "Under lock")]),
      "a password",
    );
    let found = specs(&file);
    assert_eq!(
      found.warnings,
      ["the file is encrypted and asks for a password, so what it holds is not read"]
    );
    let facts = document(found);
    assert_eq!(detail(&facts).is_encrypted, Some(true));
    // Nothing is said of pages, words or pictures, which would each read as none.
    assert_eq!(
      (
        facts.page_count,
        facts.word_count,
        facts.image_count,
        &facts.title
      ),
      (None, None, None, &None)
    );
    assert_eq!(detail(&facts).is_searchable, None);
  }

  #[test]
  fn a_document_with_no_text_of_its_own_cannot_be_searched() {
    let facts = document(specs(&pdf(&["", ""], &[])));
    assert_eq!(
      (facts.page_count, facts.word_count, &facts.text_preview),
      (Some(2), Some(0), &None)
    );
    assert_eq!(detail(&facts).is_searchable, Some(false));
  }

  #[test]
  fn a_long_text_is_previewed_by_its_first_five_hundred_characters() {
    let line = "word ".repeat(300);
    let facts = document(specs(&pdf(&[line.trim_end()], &[])));
    assert_eq!(facts.word_count, Some(300));
    assert_eq!(
      facts
        .text_preview
        .as_ref()
        .map(|preview| preview.chars().count()),
      Some(499)
    );
  }

  #[test]
  fn part_of_a_file_gives_what_its_head_says_and_asks_for_the_rest() {
    let file = pdf(&["Hello from the first page"], &[]);
    let size = u64::try_from(file.len()).unwrap();
    let pieces = [Piece {
      offset: 0,
      bytes: &file[..200],
    }];
    let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
    assert_eq!(found.needs, Some(ByteRange::forward(200, size - 200)));
    let facts = document(found);
    assert_eq!(
      (detail(&facts).version.as_deref(), facts.page_count),
      (Some("1.7"), None)
    );
  }

  #[test]
  fn a_file_the_reader_cannot_open_is_still_a_pdf_and_says_so() {
    let found = specs(b"%PDF-1.5\nand then nothing a reader could follow\n");
    assert_eq!(found.media_type, MediaType::Pdf);
    assert_eq!(found.warnings.len(), 1);
    assert!(
      found.warnings[0].starts_with("the file could not be read as a PDF"),
      "{:?}",
      found.warnings
    );
    assert_eq!(detail(&document(found)).version.as_deref(), Some("1.5"));
  }
}
