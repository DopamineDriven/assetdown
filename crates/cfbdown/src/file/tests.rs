use super::*;
use crate::write::compound;

fn file_of(streams: &[(&str, &[u8])]) -> Vec<u8> {
  compound(streams).unwrap()
}

fn named<'a>(found: &'a [Stream], name: &str) -> &'a [u8] {
  &found
    .iter()
    .find(|stream| stream.name == name)
    .unwrap_or_else(|| panic!("no {name}"))
    .bytes
}

fn streams_of(file: &[u8]) -> Vec<Stream> {
  CompoundFile::open(file).unwrap().streams()
}

#[test]
fn small_streams_are_read_out_of_the_mini_stream() {
  let contents: Vec<u8> = (0..200u8).collect();
  let file = file_of(&[("AttachDesc", b"a name"), ("AttachContents", &contents)]);
  let found = streams_of(&file);
  assert_eq!(found.len(), 2);
  assert_eq!(named(&found, "AttachDesc"), b"a name");
  assert_eq!(named(&found, "AttachContents"), &contents[..]);
}

#[test]
fn a_stream_of_many_sectors_is_read_whole_and_no_further() {
  // 5,000 bytes: ten sectors, and the last of them not full.
  let large: Vec<u8> = (0..5000u32).map(|n| (n % 251) as u8).collect();
  let file = file_of(&[("Package", &large), ("Note", b"small")]);
  let found = streams_of(&file);
  assert_eq!(named(&found, "Package"), &large[..]);
  assert_eq!(named(&found, "Note"), b"small");
}

#[test]
fn an_empty_stream_is_listed_with_nothing_in_it() {
  let found = streams_of(&file_of(&[("Empty", b"")]));
  assert_eq!(named(&found, "Empty"), b"");
}

#[test]
fn bytes_that_are_not_a_compound_file_say_so() {
  assert_eq!(
    CompoundFile::open(b"%PDF-1.7").err(),
    Some(CfbError::NotCompound)
  );
  assert_eq!(CompoundFile::open(&[]).err(), Some(CfbError::NotCompound));
  assert_eq!(CfbError::NotCompound.to_string(), "not a compound file");
}

#[test]
fn a_compound_file_cut_short_is_malformed_and_says_what_cannot_be_read() {
  let file = file_of(&[("a", b"b")]);
  let error = CompoundFile::open(&file[..300]).err().unwrap();
  assert!(matches!(error, CfbError::Malformed(_)), "{error:?}");
  assert!(
    error.to_string().starts_with("malformed compound file: "),
    "{error}"
  );
  // The header is whole, and the directory it points at is gone.
  assert!(matches!(
    CompoundFile::open(&file[..512]).err(),
    Some(CfbError::Malformed(_))
  ));
}

#[test]
fn a_chain_of_sectors_that_leads_back_on_itself_is_not_followed_round() {
  let large = vec![7u8; 5000];
  let mut file = file_of(&[("Package", &large)]);
  // The table is the first sector after the header. The stream's chain starts after the
  // table's own entry and the directory's: its first sector is made to point at itself.
  let first = 2u32;
  let entry = 512 + 4 * usize::try_from(first).unwrap();
  file[entry..entry + 4].copy_from_slice(&first.to_le_bytes());
  let found = streams_of(&file);
  assert!(
    found.iter().all(|stream| stream.name != "Package"),
    "a stream with a broken chain is left out"
  );
}

/// Where the directory entry numbered `index` starts in a file `compound` wrote.
fn entry_at(file: &[u8], index: usize) -> usize {
  let first = u32::from_le_bytes(file[0x30..0x34].try_into().unwrap());
  (usize::try_from(first).unwrap() + 1) * 512 + index * 128
}

/// The name of the directory entry that starts at `at`.
fn name_at(file: &[u8], at: usize) -> String {
  let units: Vec<u16> = file[at..at + 64]
    .as_chunks::<2>()
    .0
    .iter()
    .map(|pair| u16::from_le_bytes(*pair))
    .take_while(|unit| *unit != 0)
    .collect();
  String::from_utf16_lossy(&units)
}

/// The number of the directory entry of this name.
fn entry_named(file: &[u8], name: &str) -> usize {
  (0..)
    .find(|index| name_at(file, entry_at(file, *index)) == name)
    .unwrap()
}

#[test]
fn a_stream_said_to_be_longer_than_its_sectors_is_left_out() {
  let mut file = file_of(&[("Short", &[1u8; 100]), ("Whole", b"kept")]);
  let size = entry_at(&file, entry_named(&file, "Short")) + 0x78;
  file[size..size + 8].copy_from_slice(&1_000_000u64.to_le_bytes());
  let found = streams_of(&file);
  assert!(found.iter().all(|stream| stream.name != "Short"));
  assert_eq!(named(&found, "Whole"), b"kept");
}

#[test]
fn a_stream_inside_storages_is_found_by_its_path() {
  let large = vec![9u8; 6000];
  let file = file_of(&[
    ("WordDocument", b"the text"),
    ("ObjectPool/_1/Package", &large),
    ("ObjectPool/_2/Package", b"another"),
  ]);
  let opened = CompoundFile::open(&file).unwrap();
  assert_eq!(
    opened.stream("WordDocument").as_deref(),
    Some(&b"the text"[..])
  );
  assert_eq!(opened.stream("ObjectPool/_1/Package"), Some(large));
  assert_eq!(
    opened.stream("ObjectPool/_2/Package").as_deref(),
    Some(&b"another"[..])
  );
  // A storage is not a stream, and a stream of one storage is not found in another.
  assert_eq!(opened.stream("ObjectPool"), None);
  assert_eq!(opened.stream("Package"), None);
  assert_eq!(opened.stream("ObjectPool/_3/Package"), None);
  assert!(opened.is_storage("ObjectPool/_1"));
  assert!(!opened.is_storage("WordDocument"));
  assert!(!opened.is_storage("ObjectPool/_3"));
}

#[test]
fn every_stream_is_listed_by_its_own_name_whichever_storage_it_lies_in() {
  let file = file_of(&[("Top", b"1"), ("Outer/Inner/Deep", b"2")]);
  let found = streams_of(&file);
  assert_eq!(found.len(), 2);
  assert_eq!(named(&found, "Top"), b"1");
  assert_eq!(named(&found, "Deep"), b"2");
}

#[test]
fn names_are_matched_whatever_their_case() {
  let file = file_of(&[("Workbook", b"records"), ("Pool/Item", b"held")]);
  let opened = CompoundFile::open(&file).unwrap();
  assert_eq!(opened.stream("WORKBOOK").as_deref(), Some(&b"records"[..]));
  assert_eq!(opened.stream("pool/ITEM").as_deref(), Some(&b"held"[..]));
}

#[test]
fn a_storage_lists_what_it_holds() {
  let file = file_of(&[
    ("Workbook", b"records"),
    ("Pool/_2/Package", b"b"),
    ("Pool/_1/Package", b"a"),
    ("Pool/Loose", b"c"),
  ]);
  let opened = CompoundFile::open(&file).unwrap();
  let listed = |storage: &str| -> Vec<(String, bool)> {
    let mut listed: Vec<(String, bool)> = opened
      .children(storage)
      .into_iter()
      .map(|child| (child.name.to_string(), child.is_storage))
      .collect();
    listed.sort();
    listed
  };
  assert_eq!(
    listed(""),
    [("Pool".to_string(), true), ("Workbook".to_string(), false)]
  );
  assert_eq!(
    listed("Pool"),
    [
      ("Loose".to_string(), false),
      ("_1".to_string(), true),
      ("_2".to_string(), true)
    ]
  );
  assert_eq!(listed("Pool/_1"), [("Package".to_string(), false)]);
  assert!(listed("Nowhere").is_empty());
  assert!(listed("Workbook").is_empty());
}

#[test]
fn a_directory_whose_entries_point_back_at_each_other_is_walked_once() {
  let mut file = file_of(&[("A", b"1"), ("B", b"2"), ("C", b"3")]);
  // Every entry is made the sibling of itself, to the left and to the right.
  for name in ["A", "B", "C"] {
    let index = entry_named(&file, name);
    let at = entry_at(&file, index);
    let own = u32::try_from(index).unwrap().to_le_bytes();
    file[at + 0x44..at + 0x48].copy_from_slice(&own);
    file[at + 0x48..at + 0x4C].copy_from_slice(&own);
  }
  let opened = CompoundFile::open(&file).unwrap();
  // What the root points at is still found, and found once.
  assert_eq!(opened.children("").len(), 1);
  assert_eq!(opened.streams().len(), 3);
}

#[test]
fn a_storage_is_given_as_a_file_of_its_own() {
  let large = vec![4u8; 5000];
  let file = file_of(&[
    ("WordDocument", b"text"),
    ("ObjectPool/_1/Workbook", &large),
    ("ObjectPool/_1/Sub/Note", b"n"),
  ]);
  let opened = CompoundFile::open(&file).unwrap();
  let lifted = opened.storage("ObjectPool/_1").unwrap();
  let inner = CompoundFile::open(&lifted).unwrap();
  assert_eq!(inner.stream("Workbook"), Some(large));
  assert_eq!(inner.stream("Sub/Note").as_deref(), Some(&b"n"[..]));
  assert_eq!(inner.children("").len(), 2);
  assert_eq!(opened.storage("WordDocument"), None);
  assert_eq!(opened.storage("Nowhere"), None);
}
