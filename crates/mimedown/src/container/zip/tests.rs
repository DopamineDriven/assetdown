use super::*;
use crate::source::Piece;
use crate::testing::{Sizes, zip};

const OFFICE: [(&str, &[u8]); 3] = [
  ("[Content_Types].xml", b"<Types/>"),
  ("_rels/.rels", b"<Relationships/>"),
  ("word/document.xml", b"<w:document/>"),
];

fn names(listing: &Listing) -> Vec<&str> {
  listing.names.iter().map(String::as_str).collect()
}

fn whole(bytes: &[u8]) -> Listing {
  listing(&Source::whole(bytes)).unwrap().unwrap()
}

/// What the head of a file lists, when the file's size is not known.
fn head(bytes: &[u8]) -> Listing {
  let pieces = [Piece { offset: 0, bytes }];
  listing(&Source::from_pieces(&pieces, None).unwrap())
    .unwrap()
    .unwrap()
}

#[test]
fn the_names_are_read_from_the_directory_at_the_end() {
  let found = whole(&zip(&OFFICE, Sizes::InHeader, b""));
  assert_eq!(
    names(&found),
    ["[Content_Types].xml", "_rels/.rels", "word/document.xml"]
  );
  assert!(found.complete);
  assert_eq!((found.needs, found.declared), (None, None));
}

#[test]
fn a_zip_written_as_a_stream_lists_its_names_all_the_same() {
  let found = whole(&zip(&OFFICE, Sizes::AfterData, b""));
  assert_eq!(names(&found).len(), 3);
  assert!(found.complete);
}

#[test]
fn a_comment_after_the_directory_does_not_hide_it() {
  let found = whole(&zip(
    &OFFICE,
    Sizes::InHeader,
    b"packed by hand, with a note",
  ));
  assert_eq!(names(&found).len(), 3);
  assert!(found.complete);
}

#[test]
fn a_first_entry_called_mimetype_declares_what_the_file_is() {
  let entries: [(&str, &[u8]); 2] = [
    ("mimetype", b"application/epub+zip"),
    ("META-INF/container.xml", b"<container/>"),
  ];
  let found = whole(&zip(&entries, Sizes::InHeader, b""));
  assert_eq!(found.declared.as_deref(), Some("application/epub+zip"));
  // Only the first entry speaks for the file.
  let entries: [(&str, &[u8]); 2] = [
    ("notes.txt", b"hello"),
    ("mimetype", b"application/epub+zip"),
  ];
  assert_eq!(whole(&zip(&entries, Sizes::InHeader, b"")).declared, None);
}

#[test]
fn an_archive_of_nothing_is_still_a_zip() {
  let found = whole(&zip(&[], Sizes::InHeader, b""));
  assert_eq!(names(&found), Vec::<&str>::new());
  assert!(found.complete);
}

#[test]
fn other_bytes_are_not_a_zip() {
  assert_eq!(listing(&Source::whole(b"PK\x03\x05 and so on")), Ok(None));
  assert_eq!(listing(&Source::whole(b"%PDF-1.7\n")), Ok(None));
}

#[test]
fn a_head_lists_the_names_one_entry_leads_to_the_next_and_asks_for_the_end() {
  let file = zip(&OFFICE, Sizes::InHeader, b"");
  // The first two entries, and the header of the third without its name.
  let found = head(&file[..160]);
  assert_eq!(names(&found), ["[Content_Types].xml", "_rels/.rels"]);
  assert!(!found.complete);
  // The file's size is not known, so the range is counted back from its end.
  assert_eq!(
    found.needs,
    Some(ByteRange {
      offset: 65_557,
      length: 65_557,
      from_end: true
    })
  );
}

#[test]
fn a_head_that_reaches_the_directory_has_every_name() {
  let file = zip(&OFFICE, Sizes::InHeader, b"");
  let found = head(&file);
  assert_eq!(names(&found).len(), 3);
  assert!(found.complete);
  assert_eq!(found.needs, None);
}

#[test]
fn a_head_of_a_streamed_zip_gives_the_first_name_only() {
  let file = zip(&OFFICE, Sizes::AfterData, b"");
  let found = head(&file[..200]);
  assert_eq!(names(&found), ["[Content_Types].xml"]);
  assert!(!found.complete);
  assert!(found.needs.is_some());
}

#[test]
fn when_the_size_is_known_the_end_is_asked_for_by_its_offset_and_then_read() {
  let big = vec![7u8; 100_000];
  let entries: [(&str, &[u8]); 3] = [
    ("[Content_Types].xml", b"<Types/>"),
    ("media/film.bin", &big),
    ("xl/workbook.xml", b"<workbook/>"),
  ];
  let file = zip(&entries, Sizes::AfterData, b"");
  let size = u64::try_from(file.len()).unwrap();
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..4096],
  }];
  let first = listing(&Source::from_pieces(&pieces, Some(size)).unwrap())
    .unwrap()
    .unwrap();
  assert_eq!(names(&first), ["[Content_Types].xml"]);
  let needs = first.needs.unwrap();
  assert_eq!(
    needs,
    ByteRange {
      offset: size - 65_557,
      length: 65_557,
      from_end: false
    }
  );

  // Given the end, every name is there.
  let tail = usize::try_from(needs.offset).unwrap();
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..4096],
    },
    Piece {
      offset: needs.offset,
      bytes: &file[tail..],
    },
  ];
  let second = listing(&Source::from_pieces(&pieces, Some(size)).unwrap())
    .unwrap()
    .unwrap();
  assert_eq!(
    names(&second),
    ["[Content_Types].xml", "media/film.bin", "xl/workbook.xml"]
  );
  assert!(second.complete);
  assert_eq!(second.needs, None);
}

#[test]
fn a_zip_that_lost_its_directory_lists_what_its_entries_lead_to() {
  let file = zip(&OFFICE, Sizes::InHeader, b"");
  // Cut inside the third entry's data: no directory, and nothing to ask for.
  let found = whole(&file[..175]);
  assert_eq!(
    names(&found),
    ["[Content_Types].xml", "_rels/.rels", "word/document.xml"]
  );
  assert!(!found.complete);
  assert_eq!(found.needs, None);
}
