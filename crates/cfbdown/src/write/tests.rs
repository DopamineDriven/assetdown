use super::*;
use crate::file::CompoundFile;

#[test]
fn a_file_whose_table_needs_more_than_one_sector_is_read_back() {
  // 200,000 bytes are 391 sectors, and one sector of the table chains 128.
  let large: Vec<u8> = (0..200_000u32).map(|n| (n % 253) as u8).collect();
  let other: Vec<u8> = (0..70_000u32).map(|n| (n % 241) as u8).collect();
  let file = compound(&[("Workbook", &large), ("Data", &other), ("Small", b"s")]).unwrap();
  let opened = CompoundFile::open(&file).unwrap();
  assert_eq!(opened.stream("Workbook"), Some(large));
  assert_eq!(opened.stream("Data"), Some(other));
  assert_eq!(opened.stream("Small").as_deref(), Some(&b"s"[..]));
}

#[test]
fn a_file_with_more_table_sectors_than_the_header_lists_is_read_back() {
  // The header lists 109 sectors of the table, which chain 109 × 128 sectors of 512 bytes:
  // a little over seven million bytes. This is past that.
  let large: Vec<u8> = (0..7_400_000u32).map(|n| (n % 239) as u8).collect();
  let file = compound(&[("Workbook", &large), ("After", &[5u8; 9000])]).unwrap();
  let listed = u32::from_le_bytes(file[0x48..0x4C].try_into().unwrap());
  assert!(listed >= 1, "the table's sectors run on past the header");
  let opened = CompoundFile::open(&file).unwrap();
  assert_eq!(opened.stream("Workbook"), Some(large));
  assert_eq!(opened.stream("After"), Some(vec![5u8; 9000]));
}

#[test]
fn many_small_streams_fill_more_than_one_sector_of_the_small_table() {
  let names: Vec<String> = (0..300).map(|n| format!("S{n}")).collect();
  let bodies: Vec<Vec<u8>> = (0..300u32)
    .map(|n| vec![(n % 251) as u8; 100 + n as usize])
    .collect();
  let streams: Vec<(&str, &[u8])> = names
    .iter()
    .zip(&bodies)
    .map(|(name, body)| (name.as_str(), body.as_slice()))
    .collect();
  let file = compound(&streams).unwrap();
  let opened = CompoundFile::open(&file).unwrap();
  for (name, body) in &streams {
    assert_eq!(opened.stream(name).as_deref(), Some(*body), "{name}");
  }
}

/// The directory entries of a file `compound` wrote: each one's name, and the numbers of the
/// entries to its left, to its right, and below it.
fn directory(file: &[u8]) -> Vec<(String, u32, u32, u32)> {
  let opened = CompoundFile::open(file).unwrap();
  let count = opened.streams().len() + 64;
  let first = u32::from_le_bytes(file[0x30..0x34].try_into().unwrap());
  let u32_at = |at: usize| u32::from_le_bytes(file[at..at + 4].try_into().unwrap());
  // The directory of these small files is written in sectors that follow one another.
  let start = (usize::try_from(first).unwrap() + 1) * 512;
  (0..count)
    .map(|index| start + index * 128)
    .take_while(|at| at + 128 <= file.len() && file[at + 0x42] != 0)
    .map(|at| {
      let units: Vec<u16> = file[at..at + 64]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
      (
        String::from_utf16_lossy(&units),
        u32_at(at + 0x44),
        u32_at(at + 0x48),
        u32_at(at + 0x4C),
      )
    })
    .collect()
}

/// The names under the entry numbered `top`, walked left, then the entry, then right.
fn in_order(entries: &[(String, u32, u32, u32)], top: u32, out: &mut Vec<String>) {
  let Some((name, left, right, _)) = entries.get(top as usize) else {
    return;
  };
  in_order(entries, *left, out);
  out.push(name.clone());
  in_order(entries, *right, out);
}

#[test]
fn the_entries_of_a_storage_are_ordered_shorter_names_first_and_then_by_their_letters() {
  let file = compound(&[
    ("Workbook", b"1"),
    ("b", b"2"),
    ("\u{5}SummaryInformation", b"3"),
    ("Abc", b"4"),
    ("abd", b"5"),
    ("Z", b"6"),
    ("CompObj", b"7"),
  ])
  .unwrap();
  let entries = directory(&file);
  assert_eq!(entries[0].0, "Root Entry");
  let mut walked = Vec::new();
  in_order(&entries, entries[0].3, &mut walked);
  assert_eq!(
    walked,
    [
      "b",
      "Z",
      "Abc",
      "abd",
      "CompObj",
      "Workbook",
      "\u{5}SummaryInformation"
    ]
  );
}

#[test]
fn what_the_format_cannot_hold_is_refused() {
  let long = "n".repeat(32);
  assert_eq!(
    compound(&[(long.as_str(), b"x")]).err(),
    Some(CfbError::Unwritable("a name is longer than 31 characters"))
  );
  assert_eq!(
    compound(&[("a", b"1"), ("A", b"2")]).err(),
    Some(CfbError::Unwritable(
      "two entries of one storage have the same name"
    ))
  );
  assert_eq!(
    compound(&[("a/b", b"1"), ("a", b"2")]).err(),
    Some(CfbError::Unwritable(
      "two entries of one storage have the same name"
    ))
  );
  assert_eq!(
    compound(&[("", b"1")]).err(),
    Some(CfbError::Unwritable("a name is empty"))
  );
  assert!(compound(&[]).is_ok());
}
