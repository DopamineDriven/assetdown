//! Formula text: moving a formula's relative references, which is how a shared formula reads on
//! each of the cells that follow it.
//!
//! A shared formula is stored once, on its master cell; every other cell of the group names the
//! group and nothing else. What such a cell computes is the master's formula with each relative
//! reference moved by the cell's offset from the master, exactly as a copy and paste would move
//! it. Doing that needs no formula parser, only a scan that can tell a reference from everything
//! that merely looks like one: text in quotes, sheet names, function names such as `LOG10`,
//! defined names, numbers such as `1E5`, and the brackets of a table reference.

use crate::address::{MAX_COLS, MAX_ROWS, column_index, column_letters, row_index};

/// What a reference that moves off the grid becomes, as it does in Excel.
const OFF_GRID: &str = "#REF!";

/// `formula` with every relative reference moved `rows` down and `cols` right; absolute parts
/// (`$A`, `$1`) stay where they are. Everything that is not a reference is copied unchanged.
pub(crate) fn shift_formula(formula: &str, rows: i64, cols: i64) -> String {
  let chars: Vec<char> = formula.chars().collect();
  let mut out = String::with_capacity(formula.len() + 8);
  let mut at = 0;
  while let Some(&c) = chars.get(at) {
    at = match c {
      '"' | '\'' => copy_quoted(&chars, at, &mut out),
      '[' => copy_bracketed(&chars, at, &mut out),
      '#' => copy_error(&chars, at, &mut out),
      '$' => match row_range_at(&chars, at) {
        Some(range) => range.write_moved(rows, MAX_ROWS, row_number, &mut out),
        None => reference_or_name(&chars, at, rows, cols, &mut out),
      },
      _ if c.is_ascii_digit() => match row_range_at(&chars, at) {
        Some(range) => range.write_moved(rows, MAX_ROWS, row_number, &mut out),
        None => copy_number(&chars, at, &mut out),
      },
      '.' if chars.get(at + 1).is_some_and(char::is_ascii_digit) => {
        copy_number(&chars, at, &mut out)
      }
      _ if starts_name(c) => reference_or_name(&chars, at, rows, cols, &mut out),
      _ => {
        out.push(c);
        at + 1
      }
    };
  }
  out
}

/// One coordinate of a reference: a 0-based row or column and whether `$` pins it.
#[derive(Debug, Clone, Copy)]
struct Coordinate {
  index: u32,
  absolute: bool,
}

impl Coordinate {
  /// The coordinate after a move, or `None` when a relative one leaves the grid of `size`.
  fn moved(self, by: i64, size: u32) -> Option<u32> {
    if self.absolute {
      return Some(self.index);
    }
    u32::try_from(i64::from(self.index) + by)
      .ok()
      .filter(|index| *index < size)
  }

  fn marker(self) -> &'static str {
    if self.absolute { "$" } else { "" }
  }
}

/// A span of the formula read as two coordinates of the same kind either side of a colon: the
/// columns of `A:C`, the rows of `1:5`.
struct Span {
  first: Coordinate,
  last: Coordinate,
  end: usize,
}

impl Span {
  /// Write both ends moved `by` within a grid of `size`, each spelled by `spell`, and say where
  /// the scan resumes.
  fn write_moved(
    &self,
    by: i64,
    size: u32,
    spell: impl Fn(u32) -> String,
    out: &mut String,
  ) -> usize {
    for (i, coordinate) in [self.first, self.last].into_iter().enumerate() {
      if i > 0 {
        out.push(':');
      }
      match coordinate.moved(by, size) {
        Some(index) => {
          out.push_str(coordinate.marker());
          out.push_str(&spell(index));
        }
        None => out.push_str(OFF_GRID),
      }
    }
    self.end
  }
}

/// A 0-based row as the 1-based number a formula spells.
fn row_number(row: u32) -> String {
  (u64::from(row) + 1).to_string()
}

/// Characters a name, a function or a sheet name may continue with.
fn continues_name(c: char) -> bool {
  c.is_alphanumeric() || matches!(c, '_' | '.' | '\\' | '?')
}

fn starts_name(c: char) -> bool {
  c.is_alphabetic() || matches!(c, '_' | '\\')
}

/// Whether what ends just before `end` stands alone as a reference: the next character does not
/// make it part of a longer name, a function call (`LOG10(`) or a sheet name (`AB12!`).
fn reference_ends(chars: &[char], end: usize) -> bool {
  !chars
    .get(end)
    .is_some_and(|&next| continues_name(next) || matches!(next, '(' | '!' | '$'))
}

/// An optional `$` and one to three column letters at `at`: the column and where it ends.
fn column_at(chars: &[char], at: usize) -> Option<(Coordinate, usize)> {
  let absolute = chars.get(at) == Some(&'$');
  let start = at + usize::from(absolute);
  let letters: String = chars
    .get(start..)?
    .iter()
    .take_while(|c| c.is_ascii_alphabetic())
    .collect();
  let index = column_index(&letters)?;
  Some((Coordinate { index, absolute }, start + letters.len()))
}

/// An optional `$` and a row number at `at`: the row and where it ends.
fn row_at(chars: &[char], at: usize) -> Option<(Coordinate, usize)> {
  let absolute = chars.get(at) == Some(&'$');
  let start = at + usize::from(absolute);
  let digits: String = chars
    .get(start..)?
    .iter()
    .take_while(|c| c.is_ascii_digit())
    .collect();
  let index = row_index(&digits)?;
  Some((Coordinate { index, absolute }, start + digits.len()))
}

/// Whole rows, `1:5` or `$1:$5`, at `at`.
fn row_range_at(chars: &[char], at: usize) -> Option<Span> {
  let (first, colon) = row_at(chars, at)?;
  if chars.get(colon) != Some(&':') {
    return None;
  }
  let (last, end) = row_at(chars, colon + 1)?;
  reference_ends(chars, end).then_some(Span { first, last, end })
}

/// Whole columns, `A:C` or `$A:$C`, at `at`.
fn column_range_at(chars: &[char], at: usize) -> Option<Span> {
  let (first, colon) = column_at(chars, at)?;
  if chars.get(colon) != Some(&':') {
    return None;
  }
  let (last, end) = column_at(chars, colon + 1)?;
  reference_ends(chars, end).then_some(Span { first, last, end })
}

/// At the start of a word: write it moved when it is a cell reference or a column range, and
/// unchanged when it is a name, a function or a sheet name. Returns where the scan resumes.
fn reference_or_name(chars: &[char], at: usize, rows: i64, cols: i64, out: &mut String) -> usize {
  let cell = column_at(chars, at).and_then(|(col, after)| {
    let (row, end) = row_at(chars, after)?;
    reference_ends(chars, end).then_some((col, row, end))
  });
  if let Some((col, row, end)) = cell {
    match (col.moved(cols, MAX_COLS), row.moved(rows, MAX_ROWS)) {
      (Some(c), Some(r)) => {
        out.push_str(col.marker());
        out.push_str(&column_letters(c));
        out.push_str(row.marker());
        out.push_str(&row_number(r));
      }
      _ => out.push_str(OFF_GRID),
    }
    return end;
  }
  if let Some(range) = column_range_at(chars, at) {
    return range.write_moved(cols, MAX_COLS, column_letters, out);
  }
  // A name: copy it whole, so that no later part of it is read as a reference.
  let mut end = at;
  while chars
    .get(end)
    .is_some_and(|&c| continues_name(c) || (c == '$' && end == at))
  {
    end += 1;
  }
  // A lone `$` that starts neither a reference nor a name is copied as it stands.
  let end = end.max(at + 1);
  out.extend(chars.get(at..end).unwrap_or_default());
  end
}

/// Copy a quoted run, text in `"…"` or a sheet name in `'…'`, through its closing quote; a
/// doubled quote inside it is an escaped one.
fn copy_quoted(chars: &[char], at: usize, out: &mut String) -> usize {
  let Some(&quote) = chars.get(at) else {
    return at;
  };
  out.push(quote);
  let mut i = at + 1;
  while let Some(&c) = chars.get(i) {
    out.push(c);
    i += 1;
    if c == quote {
      if chars.get(i) == Some(&quote) {
        out.push(quote);
        i += 1;
      } else {
        break;
      }
    }
  }
  i
}

/// Copy a bracketed run, a table reference or a workbook index, through the bracket that closes
/// it. Brackets nest, and inside them a `'` escapes the character after it.
fn copy_bracketed(chars: &[char], at: usize, out: &mut String) -> usize {
  let mut depth = 0usize;
  let mut i = at;
  while let Some(&c) = chars.get(i) {
    out.push(c);
    i += 1;
    match c {
      '[' => depth += 1,
      ']' => {
        depth = depth.saturating_sub(1);
        if depth == 0 {
          break;
        }
      }
      '\'' => {
        if let Some(&escaped) = chars.get(i) {
          out.push(escaped);
          i += 1;
        }
      }
      _ => {}
    }
  }
  i
}

/// Copy an error literal such as `#DIV/0!` or `#N/A`; a `#` that starts none is the spill
/// operator and is copied alone.
fn copy_error(chars: &[char], at: usize, out: &mut String) -> usize {
  out.push('#');
  let mut i = at + 1;
  if !chars.get(i).is_some_and(|c| c.is_ascii_alphabetic()) {
    return i;
  }
  while let Some(&c) = chars.get(i) {
    if !(c.is_ascii_alphanumeric() || matches!(c, '/' | '_')) {
      break;
    }
    out.push(c);
    i += 1;
  }
  if let Some(&end) = chars.get(i)
    && matches!(end, '!' | '?')
  {
    out.push(end);
    i += 1;
  }
  i
}

/// Copy a number, `12`, `1.5` or `2.5E-3`, so that its exponent is not read as a reference.
fn copy_number(chars: &[char], at: usize, out: &mut String) -> usize {
  let digits_from = |from: usize| {
    chars
      .get(from..)
      .unwrap_or_default()
      .iter()
      .take_while(|c| c.is_ascii_digit())
      .count()
  };
  let mut end = at + digits_from(at);
  if chars.get(end) == Some(&'.') {
    end += 1;
    end += digits_from(end);
  }
  if chars.get(end).is_some_and(|c| matches!(c, 'E' | 'e')) {
    let sign = usize::from(chars.get(end + 1).is_some_and(|c| matches!(c, '+' | '-')));
    let exponent = digits_from(end + 1 + sign);
    if exponent > 0 {
      end += 1 + sign + exponent;
    }
  }
  out.extend(chars.get(at..end).unwrap_or_default());
  end
}

#[cfg(test)]
mod tests;
