use super::*;
use crate::testing::{Sizes, png_header, png_of, zip};

#[test]
fn an_image_is_named_and_its_facts_are_read() {
  let found = specs(&png_of(&[png_header(640, 480, 8, 6)]));
  assert_eq!(found.media_type, MediaType::Png);
  assert_eq!(found.evidence, Some(Evidence::Signature));
  let Facts::Image(facts) = &found.facts else {
    panic!("no image facts: {found:?}");
  };
  assert_eq!((facts.width, facts.height), (640, 480));
}

#[test]
fn a_type_with_no_reader_here_is_named_and_nothing_more() {
  let found = specs(&zip(&[("notes.txt", b"hello")], Sizes::InHeader, b""));
  assert_eq!(
    (found.media_type, &found.facts),
    (MediaType::Zip, &Facts::None)
  );
  assert_eq!((found.needs, found.warnings.len()), (None, 0));
}

#[test]
fn bytes_that_name_nothing_have_no_facts_and_are_not_an_error() {
  let found = specs(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);
  assert_eq!(found.media_type, MediaType::OctetStream);
  assert_eq!((found.evidence, &found.facts), (None, &Facts::None));
}

#[test]
fn text_has_a_document_s_facts_and_says_what_named_it() {
  use crate::{DocumentDetail, Options, specs_with};
  let text = "# Notes\n\nA first line of words.\nAnd a second.\n";
  let hint = Options {
    file_name: Some("notes.md"),
    ..Options::default()
  };
  let named = specs_with(&Source::whole(text.as_bytes()), hint);
  assert_eq!(
    (named.media_type, named.evidence),
    (MediaType::Markdown, Some(Evidence::FileName))
  );
  let Facts::Document(facts) = &named.facts else {
    panic!("no document facts: {named:?}");
  };
  assert_eq!(facts.word_count, Some(10));
  assert_eq!(facts.text_preview.as_deref(), Some(text.trim_end()));
  let DocumentDetail::Text(detail) = &facts.detail else {
    panic!("not a text's detail: {:?}", facts.detail);
  };
  assert_eq!((detail.encoding.as_str(), detail.line_count), ("utf-8", 4));

  // Without a name it is text, and the bytes do not say of what kind.
  let unnamed = specs(text.as_bytes());
  assert_eq!(
    (unnamed.media_type, unnamed.evidence),
    (MediaType::Txt, Some(Evidence::Content))
  );
}

#[test]
fn a_name_changes_nothing_for_a_file_that_is_not_text() {
  use crate::{Options, specs_with};
  let picture = png_of(&[png_header(8, 8, 8, 6)]);
  let hint = Options {
    file_name: Some("notes.txt"),
    ..Options::default()
  };
  let found = specs_with(&Source::whole(&picture), hint);
  assert_eq!(
    (found.media_type, found.evidence),
    (MediaType::Png, Some(Evidence::Signature))
  );
}

#[test]
fn part_of_a_text_is_counted_as_far_as_it_goes_and_asks_for_the_rest() {
  let text = "one two three\nfour five six\n".repeat(40);
  let size = u64::try_from(text.len()).unwrap();
  let pieces = [crate::Piece {
    offset: 0,
    bytes: &text.as_bytes()[..280],
  }];
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Txt);
  assert_eq!(found.needs, Some(ByteRange::forward(280, size - 280)));
  let Facts::Document(facts) = &found.facts else {
    panic!("no document facts: {found:?}");
  };
  assert_eq!(facts.word_count, Some(60));
}

#[test]
fn what_the_name_still_needs_is_asked_for_before_what_the_facts_do() {
  let file = png_of(&[png_header(8, 8, 8, 2)]);
  let pieces = [crate::Piece {
    offset: 0,
    bytes: &file[..4],
  }];
  let found = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  assert_eq!(found.media_type, MediaType::OctetStream);
  assert_eq!(found.needs, Some(ByteRange::forward(4, 4092)));
}
