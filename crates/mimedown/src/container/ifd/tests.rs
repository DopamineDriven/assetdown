use super::*;
use crate::testing::{TiffValue, big_tiff, tiff};

#[test]
fn a_value_is_read_from_its_entry_or_from_where_the_entry_points() {
  for little in [true, false] {
    let file = tiff(
      little,
      &[vec![
        (256, TiffValue::Short(vec![640])),
        (258, TiffValue::Short(vec![8, 8, 8])),
        (282, TiffValue::Fractions(vec![(300, 1)])),
        (305, TiffValue::Text("made by hand")),
      ]],
    );
    let source = Source::whole(&file);
    let tiff = Tiff::open(&source).unwrap().unwrap();
    assert_eq!(
      (tiff.little_endian, tiff.big, tiff.first),
      (little, false, 8)
    );
    let directory = tiff.directory(tiff.first).unwrap();
    assert_eq!((directory.entries.len(), directory.next), (4, 0));
    assert_eq!(tiff.number(directory.entry(256).unwrap()), Ok(Some(640)));
    // Three values of two bytes do not fit an entry's four.
    assert_eq!(
      tiff.numbers(directory.entry(258).unwrap()),
      Ok(vec![8, 8, 8])
    );
    assert_eq!(
      tiff.fractions(directory.entry(282).unwrap()),
      Ok(vec![300.0])
    );
    assert_eq!(
      tiff.text(directory.entry(305).unwrap()),
      Ok(Some("made by hand".to_string()))
    );
    assert!(directory.entry(999).is_none());
  }
}

#[test]
fn a_bigtiff_is_the_same_with_eight_bytes_for_an_offset_and_a_count() {
  for little in [true, false] {
    let file = big_tiff(little, &[(256, 4, 70_000), (257, 3, 480)]);
    let source = Source::whole(&file);
    let tiff = Tiff::open(&source).unwrap().unwrap();
    assert_eq!((tiff.big, tiff.first), (true, 16));
    let directory = tiff.directory(tiff.first).unwrap();
    assert_eq!(tiff.number(directory.entry(256).unwrap()), Ok(Some(70_000)));
    assert_eq!(tiff.number(directory.entry(257).unwrap()), Ok(Some(480)));
  }
}

#[test]
fn other_bytes_are_not_a_tiff() {
  assert!(
    Tiff::open(&Source::whole(b"II\x2C\0\x08\0\0\0"))
      .unwrap()
      .is_none()
  );
  assert!(
    Tiff::open(&Source::whole(b"PK\x03\x04\0\0\0\0"))
      .unwrap()
      .is_none()
  );
  assert!(matches!(
    Tiff::open(&Source::whole(b"II*")),
    Err(Missing::PastEnd)
  ));
}

#[test]
fn a_count_no_file_could_hold_is_a_file_that_ends_early() {
  // A BigTIFF states how many entries a directory has in eight bytes. Where the directory
  // would end is past the largest offset there is, which is past the end of this file too.
  let mut file = big_tiff(true, &[(256, 4, 70_000)]);
  file[16..24].copy_from_slice(&0x1000_0000_0000_0000u64.to_le_bytes());
  file.resize(90_000, 0);
  let source = Source::whole(&file);
  let tiff = Tiff::open(&source).unwrap().unwrap();
  assert!(matches!(tiff.directory(tiff.first), Err(Missing::PastEnd)));
}
