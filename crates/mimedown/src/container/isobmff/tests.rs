use super::*;
use crate::source::{ByteRange, Piece};
use crate::testing::{boxed, ftyp};

fn whole(bytes: &[u8]) -> Result<Option<Opening>, Missing> {
  opening(&Source::whole(bytes))
}

fn opens_with(first: &[u8; 4]) -> Result<Option<Opening>, Missing> {
  Ok(Some(Opening {
    first: *first,
    brands: None,
  }))
}

#[test]
fn a_file_type_box_gives_its_brands() {
  let file = [ftyp(b"heic", &[b"mif1", b"heic"]), boxed(b"meta", &[0; 12])].concat();
  assert_eq!(
    whole(&file),
    Ok(Some(Opening {
      first: *b"ftyp",
      brands: Some(Brands {
        major: *b"heic",
        compatible: vec![*b"mif1", *b"heic"],
      }),
    }))
  );
  // A major brand and nothing it is compatible with.
  let brands = whole(&ftyp(b"qt  ", &[])).unwrap().unwrap().brands.unwrap();
  assert_eq!((brands.major, brands.compatible), (*b"qt  ", vec![]));
}

#[test]
fn a_file_type_box_that_does_not_hold_together_is_not_one() {
  // Too short to hold a brand and a version.
  assert_eq!(whole(b"\0\0\0\x0Cftypheic"), Ok(None));
  // Brands are four bytes each.
  assert_eq!(whole(b"\0\0\0\x12ftypheic\0\0\0\0mi\0\0\0\0\0\0"), Ok(None));
  // A brand is four printable characters.
  assert_eq!(whole(b"\0\0\0\x10ftyp\x01\x02\x03\x04\0\0\0\0"), Ok(None));
  // No file declares a thousand brands.
  let mut huge = b"\0\0\x20\0ftypisom".to_vec();
  huge.resize(0x2000, b'a');
  assert_eq!(whole(&huge), Ok(None));
}

#[test]
fn a_file_without_a_file_type_box_is_believed_when_a_second_box_follows_the_first() {
  // How QuickTime wrote a movie before there were brands.
  let classic = [boxed(b"wide", &[]), boxed(b"mdat", &[0; 16])].concat();
  assert_eq!(whole(&classic), opens_with(b"wide"));
  let movie_first = [boxed(b"moov", &[0; 24]), boxed(b"mdat", &[0; 16])].concat();
  assert_eq!(whole(&movie_first), opens_with(b"moov"));
  // A piece of a fragmented file.
  let fragment = [boxed(b"styp", b"msdh\0\0\0\0msdh"), boxed(b"moof", &[0; 8])].concat();
  assert_eq!(whole(&fragment), opens_with(b"styp"));
}

#[test]
fn a_single_box_is_believed_when_the_file_ends_where_it_does() {
  assert_eq!(whole(&boxed(b"mdat", &[0; 32])), opens_with(b"mdat"));
  // Three bytes too many.
  let longer = [boxed(b"mdat", &[0; 32]), vec![0; 3]].concat();
  assert_eq!(whole(&longer), Ok(None));
}

#[test]
fn media_data_may_run_to_the_end_of_the_file_or_state_a_length_of_eight_bytes() {
  // A length of zero: the box is the rest of the file.
  assert_eq!(
    whole(b"\0\0\0\0mdat and then the media"),
    opens_with(b"mdat")
  );
  // No other box is the rest of the file.
  assert_eq!(whole(b"\0\0\0\0free and then anything"), Ok(None));
  // A length of one: the real length is the eight bytes that follow the type.
  let long = [
    &b"\0\0\0\x01mdat\0\0\0\0\0\0\0\x18"[..],
    &[0; 8],
    &boxed(b"moov", &[0; 8]),
  ]
  .concat();
  assert_eq!(whole(&long), opens_with(b"mdat"));
}

#[test]
fn words_that_spell_a_box_type_are_not_a_box() {
  let cases: [&[u8]; 5] = [
    b"The free software movement began in 1983.",
    b"    free(pointer);\n    return 0;\n",
    b"\t\t\t\tskip this line and the next\n",
    b"How wide is the page, and how tall?",
    b"Got moov on my mind",
  ];
  for bytes in cases {
    assert_eq!(
      whole(bytes),
      Ok(None),
      "{:?}",
      String::from_utf8_lossy(bytes)
    );
  }
}

#[test]
fn a_type_no_file_opens_with_is_not_iso_base_media() {
  assert_eq!(whole(&boxed(b"abcd", &[0; 8])), Ok(None));
  // The signature box of a JPEG 2000 file: boxes, and not this family.
  assert_eq!(
    whole(b"\0\0\0\x0CjP  \r\n\x87\n\0\0\0\x14ftypjp2 "),
    Ok(None)
  );
  assert_eq!(whole(b"\x89PNG\r\n\x1A\n\0\0\0\x0DIHDR"), Ok(None));
}

#[test]
fn fewer_than_eight_bytes_are_no_box() {
  assert_eq!(whole(b"\0\0\0"), Err(Missing::PastEnd));
}

#[test]
fn a_first_box_longer_than_the_head_asks_for_the_box_after_it() {
  // A megabyte of media first, of which the head holds 24 bytes.
  let head = b"\0\x0F\x42\x40mdat\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
  let pieces = [Piece {
    offset: 0,
    bytes: head,
  }];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(
    opening(&source),
    Err(Missing::NotGiven(ByteRange::forward(1_000_000, 8)))
  );
  // Given those eight bytes, the second box settles it.
  let next = boxed(b"moov", &[]);
  let pieces = [
    Piece {
      offset: 0,
      bytes: head,
    },
    Piece {
      offset: 1_000_000,
      bytes: &next,
    },
  ];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(opening(&source), opens_with(b"mdat"));
}

#[test]
fn a_file_type_box_cut_short_by_the_head_asks_for_the_rest_of_it() {
  let file = ftyp(b"avif", &[b"avif", b"mif1", b"miaf"]);
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..12],
  }];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(
    opening(&source),
    Err(Missing::NotGiven(ByteRange::forward(12, 16)))
  );
}
