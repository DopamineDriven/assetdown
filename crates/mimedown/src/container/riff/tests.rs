use super::*;
use crate::source::{ByteRange, Piece};
use crate::testing::riff;

#[test]
fn the_form_is_the_four_letters_after_the_length() {
  let cases: [(&[u8; 4], &[u8; 4]); 5] = [
    (b"RIFF", b"WEBP"),
    (b"RIFF", b"WAVE"),
    (b"RIFF", b"AVI "),
    // A wave file too long for a length of four bytes.
    (b"RF64", b"WAVE"),
    // The same chunks with their lengths written the other way round.
    (b"RIFX", b"WAVE"),
  ];
  for (container, declared) in cases {
    let file = riff(container, declared);
    assert_eq!(form(&Source::whole(&file)), Ok(Some(*declared)));
  }
}

#[test]
fn other_letters_are_not_riff() {
  assert_eq!(form(&Source::whole(b"RIFT\x24\0\0\0WAVEfmt ")), Ok(None));
  assert_eq!(form(&Source::whole(b"FORM\0\0\0\x24AIFFCOMM")), Ok(None));
}

#[test]
fn the_form_lies_past_a_file_of_eight_bytes() {
  assert_eq!(
    form(&Source::whole(b"RIFF\x24\0\0\0")),
    Err(Missing::PastEnd)
  );
  let pieces = [Piece {
    offset: 0,
    bytes: b"RIFF\x24\0\0\0",
  }];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(
    form(&source),
    Err(Missing::NotGiven(ByteRange::forward(8, 4)))
  );
}
