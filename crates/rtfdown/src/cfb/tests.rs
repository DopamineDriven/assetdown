use super::*;
use crate::testing::compound;

fn named<'a>(found: &'a [Stream], name: &str) -> &'a [u8] {
  &found
    .iter()
    .find(|stream| stream.name == name)
    .unwrap_or_else(|| panic!("no {name}"))
    .bytes
}

#[test]
fn small_streams_are_read_out_of_the_mini_stream() {
  let contents: Vec<u8> = (0..200u8).collect();
  let file = compound(&[("AttachDesc", b"a name"), ("AttachContents", &contents)]);
  let found = streams(&file).unwrap();
  assert_eq!(found.len(), 2);
  assert_eq!(named(&found, "AttachDesc"), b"a name");
  assert_eq!(named(&found, "AttachContents"), &contents[..]);
}

#[test]
fn a_stream_of_many_sectors_is_read_whole_and_no_further() {
  // 5,000 bytes: ten sectors, and the last of them not full.
  let large: Vec<u8> = (0..5000u32).map(|n| (n % 251) as u8).collect();
  let file = compound(&[("Package", &large), ("Note", b"small")]);
  let found = streams(&file).unwrap();
  assert_eq!(named(&found, "Package"), &large[..]);
  assert_eq!(named(&found, "Note"), b"small");
}

#[test]
fn an_empty_stream_is_listed_with_nothing_in_it() {
  let found = streams(&compound(&[("Empty", b"")])).unwrap();
  assert_eq!(named(&found, "Empty"), b"");
}

#[test]
fn bytes_that_are_not_a_compound_file_are_none() {
  assert!(streams(b"%PDF-1.7").is_none());
  assert!(streams(&[]).is_none());
  assert!(streams(&compound(&[("a", b"b")])[..300]).is_none());
}

#[test]
fn a_chain_of_sectors_that_leads_back_on_itself_is_not_followed_round() {
  let large = vec![7u8; 5000];
  let mut file = compound(&[("Package", &large)]);
  // The FAT is the first sector after the header. The stream's chain starts after the FAT's
  // own entry and the directory's: its first sector is made to point at itself.
  let first = 2u32;
  let entry = 512 + 4 * usize::try_from(first).unwrap();
  file[entry..entry + 4].copy_from_slice(&first.to_le_bytes());
  let found = streams(&file).unwrap();
  assert!(
    found.iter().all(|stream| stream.name != "Package"),
    "a stream with a broken chain is left out"
  );
}

#[test]
fn a_stream_said_to_be_longer_than_its_sectors_is_left_out() {
  let mut file = compound(&[("Short", &[1u8; 100]), ("Whole", b"kept")]);
  // The directory is the second sector after the header; the first stream is its second
  // entry, and its size is at 0x78 in the entry.
  let size = 512 * 2 + 128 + 0x78;
  file[size..size + 8].copy_from_slice(&1_000_000u64.to_le_bytes());
  let found = streams(&file).unwrap();
  assert!(found.iter().all(|stream| stream.name != "Short"));
  assert_eq!(named(&found, "Whole"), b"kept");
}
