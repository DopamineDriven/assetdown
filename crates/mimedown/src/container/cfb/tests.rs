use super::*;
use crate::source::Piece;
use crate::testing::{COMPOUND_MAGIC, compound};

const WORD: [u8; 16] = [
  0x06, 0x09, 0x02, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46,
];

fn children(root: &Root) -> Vec<&str> {
  let mut names: Vec<&str> = root.children.iter().map(String::as_str).collect();
  names.sort_unstable();
  names
}

fn whole(bytes: &[u8]) -> Root {
  root(&Source::whole(bytes)).unwrap().unwrap()
}

#[test]
fn the_names_under_the_root_are_listed_with_the_roots_class() {
  let file = compound(
    &[
      ("WordDocument", &[]),
      ("1Table", &[]),
      ("\u{5}SummaryInformation", &[]),
    ],
    WORD,
  );
  let found = whole(&file);
  assert_eq!(
    children(&found),
    ["\u{5}SummaryInformation", "1Table", "WordDocument"]
  );
  assert_eq!(found.class_id, WORD);
  assert_eq!(found.needs, None);
}

#[test]
fn what_a_storage_holds_is_not_under_the_root() {
  // A Word document with a workbook embedded in it.
  let file = compound(
    &[
      ("WordDocument", &[]),
      ("ObjectPool", &["Workbook", "\u{1}Ole"]),
    ],
    WORD,
  );
  assert_eq!(children(&whole(&file)), ["ObjectPool", "WordDocument"]);
}

#[test]
fn a_directory_longer_than_one_sector_is_followed_through_the_table() {
  let file = compound(
    &[
      ("Workbook", &[]),
      ("\u{5}SummaryInformation", &[]),
      ("\u{5}DocumentSummaryInformation", &[]),
      ("\u{1}CompObj", &[]),
      ("Ctls", &[]),
      ("_VBA_PROJECT_CUR", &["VBA"]),
    ],
    [0; 16],
  );
  // Eight entries: two sectors of four.
  assert_eq!(file.len(), 512 * 4);
  assert_eq!(children(&whole(&file)).len(), 6);
  assert!(whole(&file).children.iter().any(|name| name == "Ctls"));
}

#[test]
fn a_root_that_holds_nothing_has_no_names() {
  let found = whole(&compound(&[], [0; 16]));
  assert_eq!(children(&found), Vec::<&str>::new());
}

#[test]
fn other_bytes_are_not_a_compound_file() {
  assert_eq!(root(&Source::whole(b"PK\x03\x04 and so on")), Ok(None));
  let mut near = COMPOUND_MAGIC.to_vec();
  near[7] = 0xE2;
  near.resize(512, 0);
  assert_eq!(root(&Source::whole(&near)), Ok(None));
}

#[test]
fn a_file_cut_off_after_its_signature_is_a_compound_file_with_no_names() {
  assert_eq!(whole(&COMPOUND_MAGIC), Root::default());
  // A header whose sector size is not one there is.
  let mut header = compound(&[("WordDocument", &[])], WORD);
  header[30] = 3;
  assert_eq!(whole(&header), Root::default());
}

#[test]
fn a_directory_the_head_does_not_reach_is_asked_for() {
  let file = compound(&[("PowerPoint Document", &[])], [0; 16]);
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..512],
  }];
  let found = root(&Source::from_pieces(&pieces, None).unwrap())
    .unwrap()
    .unwrap();
  assert_eq!(children(&found), Vec::<&str>::new());
  // The directory starts in sector 1, which is the third run of 512 bytes.
  assert_eq!(found.needs, Some(ByteRange::forward(1024, 512)));

  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..512],
    },
    Piece {
      offset: 1024,
      bytes: &file[1024..1536],
    },
  ];
  let found = root(&Source::from_pieces(&pieces, None).unwrap())
    .unwrap()
    .unwrap();
  assert_eq!(children(&found), ["PowerPoint Document"]);
  // The table that says whether the directory goes on was not given either.
  assert_eq!(found.needs, Some(ByteRange::forward(516, 4)));
}

#[test]
fn a_directory_that_points_back_at_itself_is_read_once() {
  let mut file = compound(&[("WordDocument", &[])], WORD);
  // The table says sector 1 is followed by sector 1.
  file[516..520].copy_from_slice(&1u32.to_le_bytes());
  assert_eq!(children(&whole(&file)), ["WordDocument"]);
}
