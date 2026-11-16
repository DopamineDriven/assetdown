//! Cell references as a sheet writes them. A workbook's cells are named this way, and the table
//! grids of a document or a presentation borrow the same names.

/// A 0-based column as a sheet letters it: 0 is `A`, 26 is `AA`.
pub fn column_letters(column: u32) -> String {
  let mut letters = Vec::new();
  let mut n = u64::from(column) + 1;
  while n > 0 {
    // The remainder is below 26, so it always fits.
    let digit = u8::try_from((n - 1) % 26).unwrap_or_default();
    letters.push(char::from(b'A' + digit));
    n = (n - 1) / 26;
  }
  letters.into_iter().rev().collect()
}

/// 0-based `(row, column)` as a sheet refers to a cell: `(1, 2)` is `C2`.
pub fn cell_ref(row: u32, column: u32) -> String {
  format!("{}{}", column_letters(column), u64::from(row) + 1)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn columns_are_lettered_as_a_sheet_letters_them() {
    let letters: Vec<String> = [0, 25, 26, 27, 701, 702].map(column_letters).to_vec();
    assert_eq!(letters, ["A", "Z", "AA", "AB", "ZZ", "AAA"]);
    assert_eq!(column_letters(u32::MAX), "MWLQKWV");
    assert_eq!(cell_ref(0, 0), "A1");
    assert_eq!(cell_ref(11, 2), "C12");
    assert_eq!(cell_ref(u32::MAX, 0), "A4294967296");
  }
}
