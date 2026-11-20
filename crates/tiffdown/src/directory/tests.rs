use super::*;

fn low_first(bytes: &[u8]) -> Reader<'_> {
  Reader {
    bytes,
    little: true,
  }
}

/// An entry of this type and count whose four bytes hold `field`, low byte first.
fn entry(kind: u16, count: u32, field: u32) -> Entry {
  Entry {
    tag: 258,
    kind,
    count,
    field: field.to_le_bytes(),
  }
}

#[test]
fn values_stand_in_the_entry_when_they_fit_and_where_it_points_when_they_do_not() {
  let file = [&[0u8; 16][..], &[8, 0, 8, 0, 8, 0]].concat();
  let reader = low_first(&file);
  // Two two-byte numbers fit the four bytes.
  assert_eq!(reader.numbers(&entry(3, 2, 0x0002_0001)), Some(vec![1, 2]));
  // Three do not, and stand at offset 16.
  assert_eq!(reader.numbers(&entry(3, 3, 16)), Some(vec![8, 8, 8]));
  assert_eq!(
    reader.numbers(&entry(1, 4, 0x0403_0201)),
    Some(vec![1, 2, 3, 4])
  );
  assert_eq!(reader.numbers(&entry(4, 1, 70_000)), Some(vec![70_000]));
}

#[test]
fn values_that_do_not_lie_within_the_file_are_not_read() {
  let file = [0u8; 64];
  let reader = low_first(&file);
  // They would start past the end, or run past it.
  assert_eq!(reader.numbers(&entry(3, 3, 1_000)), None);
  assert_eq!(reader.numbers(&entry(4, 20, 8)), None);
  assert_eq!(reader.fractions(&entry(5, 1, 60)), None);
  // An offset at the very top of what four bytes can say.
  assert_eq!(reader.numbers(&entry(4, 2, u32::MAX)), None);
}

#[test]
fn a_count_no_page_could_need_is_not_believed() {
  // A file of three megabytes holds a million and a half two-byte numbers, and a damaged
  // count asks for all of them: no page has that many strips, and none is read.
  let file = vec![0u8; 3 << 20];
  let reader = low_first(&file);
  assert_eq!(reader.numbers(&entry(3, 1_500_000, 8)), None);
  assert_eq!(reader.numbers(&entry(3, u32::MAX, 8)), None);
  assert_eq!(reader.fractions(&entry(5, 300_000, 8)), None);
  // As many as a page of many strips has are read.
  assert_eq!(
    reader
      .numbers(&entry(3, 100_000, 8))
      .map(|values| values.len()),
    Some(100_000)
  );
}

#[test]
fn values_of_a_type_that_is_not_a_whole_number_are_not_numbers() {
  let file = [0u8; 64];
  let reader = low_first(&file);
  assert_eq!(reader.numbers(&entry(2, 4, 0)), None);
  assert_eq!(reader.numbers(&entry(5, 1, 8)), None);
  assert_eq!(reader.fractions(&entry(4, 1, 8)), None);
}
