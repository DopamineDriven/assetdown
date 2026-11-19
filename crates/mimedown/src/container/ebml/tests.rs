use super::*;
use crate::source::Piece;
use crate::testing::{LengthIn, ebml};

fn declares(doc_type: Option<&str>) -> Result<Option<Header>, Missing> {
  Ok(Some(Header {
    doc_type: doc_type.map(str::to_string),
  }))
}

#[test]
fn the_header_names_the_document_type() {
  for doc_type in ["webm", "matroska"] {
    let file = ebml(Some(doc_type.as_bytes()), LengthIn::OneByte);
    assert_eq!(header(&Source::whole(&file)), declares(Some(doc_type)));
  }
}

#[test]
fn the_headers_length_may_be_written_in_eight_bytes() {
  let file = ebml(Some(b"webm"), LengthIn::EightBytes);
  assert_eq!(header(&Source::whole(&file)), declares(Some("webm")));
}

#[test]
fn a_document_type_padded_with_zeros_is_read_without_them() {
  let file = ebml(Some(b"webm\0\0\0\0"), LengthIn::OneByte);
  assert_eq!(header(&Source::whole(&file)), declares(Some("webm")));
}

#[test]
fn a_header_that_names_no_document_type_is_still_a_header() {
  let file = ebml(None, LengthIn::OneByte);
  assert_eq!(header(&Source::whole(&file)), declares(None));
  // The header's four bytes and its length, and then the file ends.
  assert_eq!(header(&Source::whole(&file[..5])), declares(None));
}

#[test]
fn other_bytes_are_not_ebml() {
  assert_eq!(
    header(&Source::whole(b"\x1A\x45\xDF\xA4\x84webm")),
    Ok(None)
  );
  assert_eq!(header(&Source::whole(b"RIFF\x24\0\0\0WAVE")), Ok(None));
}

#[test]
fn a_header_the_head_stops_inside_is_asked_for() {
  let file = ebml(Some(b"matroska"), LengthIn::OneByte);
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..10],
  }];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert!(matches!(header(&source), Err(Missing::NotGiven(_))));
  let pieces = [Piece {
    offset: 0,
    bytes: &file,
  }];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(header(&source), declares(Some("matroska")));
}
