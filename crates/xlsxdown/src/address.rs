//! Cell references: `BC23` and `A1:C3` to 0-based coordinates and back.

/// Excel's grid. A reference past it is malformed, which also bounds every per-row and
/// per-column allocation downstream.
pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;

/// `BC23` to 0-based `(row, column)`. Letters may be in either case, and an absolute marker may
/// stand before the column, the row or both (`$BC$23`). Anything else is `None`, including a
/// reference outside the grid.
pub fn parse_cell_ref(text: &str) -> Option<(u32, u32)> {
  let text = text.strip_prefix('$').unwrap_or(text);
  let letters_len = text
    .bytes()
    .take_while(|byte| byte.is_ascii_alphabetic())
    .count();
  // Every byte counted is ASCII, so the split falls on a character boundary.
  let (letters, rest) = text.split_at(letters_len);
  let digits = rest.strip_prefix('$').unwrap_or(rest);
  Some((row_index(digits)?, column_index(letters)?))
}

/// Column letters in either case to a 0-based column: `A` is 0, `XFD` the last. `None` for
/// anything that is not letters, and for a column outside the grid.
pub(crate) fn column_index(letters: &str) -> Option<u32> {
  if letters.is_empty() || !letters.bytes().all(|byte| byte.is_ascii_alphabetic()) {
    return None;
  }
  // Column letters are bijective base 26: A is 1, Z is 26, AA is 27.
  let column = letters.bytes().try_fold(0u32, |column, byte| {
    let letter = u32::from(byte.to_ascii_uppercase() - b'A') + 1;
    column
      .checked_mul(26)?
      .checked_add(letter)
      .filter(|column| *column <= MAX_COLS)
  })?;
  Some(column - 1)
}

/// A 1-based row number in digits to a 0-based row; `None` for anything else, and for a row
/// outside the grid.
pub(crate) fn row_index(digits: &str) -> Option<u32> {
  if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }
  let row = digits
    .parse::<u32>()
    .ok()
    .filter(|row| (1..=MAX_ROWS).contains(row))?;
  Some(row - 1)
}

/// A 0-based column to its letters: 0 is `A`, 26 is `AA`.
pub(crate) fn column_letters(col: u32) -> String {
  let mut letters = Vec::new();
  let mut n = u64::from(col) + 1;
  while n > 0 {
    // The remainder is below 26, so the cast is exact.
    letters.push(char::from(b'A' + ((n - 1) % 26) as u8));
    n = (n - 1) / 26;
  }
  letters.into_iter().rev().collect()
}

/// 0-based `(row, column)` to `BC23`.
pub fn cell_ref(row: u32, col: u32) -> String {
  format!("{}{}", column_letters(col), u64::from(row) + 1)
}

/// A rectangular range as 0-based inclusive corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRange {
  pub r0: u32,
  pub c0: u32,
  pub r1: u32,
  pub c1: u32,
}

impl CellRange {
  /// `A1:C3`, the same corners in any order, or a single `B2`. Whole rows and columns (`A:A`,
  /// `1:1`) are not ranges here.
  pub fn parse(text: &str) -> Option<CellRange> {
    let (first, last) = match text.split_once(':') {
      Some((first, last)) => (parse_cell_ref(first)?, parse_cell_ref(last)?),
      None => {
        let only = parse_cell_ref(text)?;
        (only, only)
      }
    };
    Some(CellRange {
      r0: first.0.min(last.0),
      c0: first.1.min(last.1),
      r1: first.0.max(last.0),
      c1: first.1.max(last.1),
    })
  }

  /// The range in A1 form, top-left corner first; a single cell is `B2:B2`.
  pub fn a1(&self) -> String {
    format!(
      "{}:{}",
      cell_ref(self.r0, self.c0),
      cell_ref(self.r1, self.c1)
    )
  }
}

#[cfg(test)]
mod tests;
