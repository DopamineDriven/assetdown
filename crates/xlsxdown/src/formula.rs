//! Formula text: moving a formula's relative references, which is how a shared formula reads on
//! each of the cells that follow it.
//!
//! A shared formula is stored once, on its master cell; every other cell of the group names the
//! group and nothing else. What such a cell computes is the master's formula with each relative
//! reference moved by the cell's offset from the master, exactly as a copy and paste would move
//! it. Doing that needs no formula parser, only a scan that can tell a reference from everything
//! that merely looks like one: text in quotes, sheet names, function names such as `LOG10`,
//! defined names, numbers such as `1E5`, and the brackets of a table reference.

use crate::address::{CellRange, MAX_COLS, MAX_ROWS, column_index, column_letters, row_index};

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

/// A cell or a range a formula refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reference {
  /// The sheet it is on, as the formula names it. None for the formula's own sheet.
  pub sheet: Option<String>,
  pub range: CellRange,
  /// Where the reference stands in the formula, its sheet's name included: the byte it starts
  /// at, and the one after its last.
  pub start: usize,
  pub end: usize,
}

/// Which sheet the reference that comes next is on.
enum On {
  /// The formula's own.
  Here,
  Sheet(String),
  /// Another workbook's sheet, or a run of sheets: not cells of this workbook to follow.
  Elsewhere,
}

/// A sheet's name as it stands between quotation marks, with each doubled mark single. A
/// name with a workbook in brackets, or a colon between two sheets, is not one sheet of this
/// workbook.
fn quoted_sheet(quoted: &str) -> On {
  let inner = quoted
    .strip_prefix('\'')
    .and_then(|rest| rest.strip_suffix('\''))
    .unwrap_or(quoted)
    .replace("''", "'");
  match inner.contains(['[', ']', ':']) {
    true => On::Elsewhere,
    false => On::Sheet(inner),
  }
}

/// A cell at `at`, with or without `$` before its column and its row: the cell, and where it
/// ends. None when what stands there is part of a longer name or the name of a function.
fn cell_at(chars: &[char], at: usize) -> Option<((u32, u32), usize)> {
  let (col, after) = column_at(chars, at)?;
  let (row, end) = row_at(chars, after)?;
  reference_ends(chars, end).then_some(((row.index, col.index), end))
}

/// Every cell and range `formula` refers to on this workbook's sheets, in the order they
/// stand in it. Text in quotation marks, names, the names of functions, numbers, errors, and
/// the columns of a table are passed over, as they are when a formula is moved.
pub(crate) fn references(formula: &str) -> Vec<Reference> {
  let chars: Vec<char> = formula.chars().collect();
  // Where each character starts in the text, and where the text ends.
  let mut offsets: Vec<usize> = formula.char_indices().map(|(at, _)| at).collect();
  offsets.push(formula.len());
  let mut found = Vec::new();
  let mut scratch = String::new();
  // The sheet named before a `!`, with where its name starts, until a reference takes it.
  let mut on = On::Here;
  let mut named_from: Option<usize> = None;
  // A workbook in brackets was just passed: the sheet named next is that workbook's.
  let mut after_workbook = false;
  let mut at = 0;
  while let Some(&c) = chars.get(at) {
    let range = match c {
      '"' => {
        at = copy_quoted(&chars, at, &mut scratch);
        None
      }
      '\'' => {
        scratch.clear();
        let end = copy_quoted(&chars, at, &mut scratch);
        if chars.get(end) == Some(&'!') {
          on = quoted_sheet(&scratch);
          named_from = Some(at);
          at = end + 1;
          continue;
        }
        at = end;
        None
      }
      '[' => {
        at = copy_bracketed(&chars, at, &mut scratch);
        after_workbook = true;
        continue;
      }
      '#' => {
        at = copy_error(&chars, at, &mut scratch);
        None
      }
      '$' | '0'..='9' if let Some(rows) = row_range_at(&chars, at) => {
        let start = at;
        at = rows.end;
        Some((
          start,
          CellRange {
            r0: rows.first.index.min(rows.last.index),
            c0: 0,
            r1: rows.first.index.max(rows.last.index),
            c1: MAX_COLS - 1,
          },
        ))
      }
      '0'..='9' => {
        at = copy_number(&chars, at, &mut scratch);
        None
      }
      '.' if chars.get(at + 1).is_some_and(char::is_ascii_digit) => {
        at = copy_number(&chars, at, &mut scratch);
        None
      }
      _ if c == '$' || starts_name(c) => {
        let start = at;
        if let Some((first, end)) = cell_at(&chars, at) {
          // A colon and another cell make the two the corners of a range.
          let second = (chars.get(end) == Some(&':'))
            .then(|| cell_at(&chars, end + 1))
            .flatten();
          let (last, end) = second.unwrap_or((first, end));
          at = end;
          Some((
            start,
            CellRange {
              r0: first.0.min(last.0),
              c0: first.1.min(last.1),
              r1: first.0.max(last.0),
              c1: first.1.max(last.1),
            },
          ))
        } else if let Some(columns) = column_range_at(&chars, at) {
          at = columns.end;
          Some((
            start,
            CellRange {
              r0: 0,
              c0: columns.first.index.min(columns.last.index),
              r1: MAX_ROWS - 1,
              c1: columns.first.index.max(columns.last.index),
            },
          ))
        } else {
          // A name. Before a `!` it is a sheet's; before a colon and another name and then a
          // `!`, it is the first of a run of sheets.
          let mut end = at;
          while chars
            .get(end)
            .is_some_and(|&c| continues_name(c) || (c == '$' && end == at))
          {
            end += 1;
          }
          let end = end.max(at + 1);
          match chars.get(end) {
            Some('!') => {
              let name: String = chars[at..end].iter().collect();
              // A sheet named after another and a colon ends a run of sheets.
              let ends_run = at > 0 && chars.get(at - 1) == Some(&':');
              on = match after_workbook || ends_run {
                true => On::Elsewhere,
                false => On::Sheet(name),
              };
              named_from = Some(at);
              after_workbook = false;
              at = end + 1;
              continue;
            }
            _ => at = end,
          }
          None
        }
      }
      _ => {
        at += 1;
        continue;
      }
    };
    // Whatever came after a sheet's name has taken it, reference or not.
    let sheet = std::mem::replace(&mut on, On::Here);
    let from = named_from.take();
    after_workbook = false;
    if let Some((start, range)) = range {
      let sheet = match sheet {
        On::Here => None,
        On::Sheet(name) => Some(name),
        On::Elsewhere => continue,
      };
      found.push(Reference {
        sheet,
        range,
        start: offsets[from.unwrap_or(start)],
        end: offsets[at],
      });
    }
  }
  found
}

#[cfg(test)]
mod tests;
