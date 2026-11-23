//! A formula's text. The file keeps a formula as a run of tokens, each thing it works on
//! before the operator or the function that works on it; this writes it out as a sheet
//! shows it, without the `=` it opens with: `SUM(B2:B9)*Rate`.
//!
//! The brackets a formula was typed with are tokens of their own, so the text needs none
//! added. Spaces and line ends typed into a formula are noted by the file and are not
//! written here. A formula many cells share is kept once, with its references as steps
//! from the cell that uses it, and is written for each cell with the steps taken.

mod functions;

use xlsxdown::column_letters;

use self::functions::{NAMED_BY_ITS_FIRST, function};

/// The sheets a reference names: the book they are in, by its place among those the
/// workbook lists, and the first and the last of them. A sheet below nought is one that
/// has been deleted, or the whole of a book.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SheetSpan {
  pub book: usize,
  pub first: i32,
  pub last: i32,
}

/// A book that formulas refer to.
#[derive(Debug, Clone)]
pub(crate) enum Book {
  /// The workbook itself.
  Own,
  /// The functions add-ins give, by name.
  AddIn { names: Vec<String> },
  /// Another workbook: its sheets, and the names it defines.
  Other {
    sheets: Vec<String>,
    names: Vec<String>,
  },
}

/// What a formula's references are resolved against.
#[derive(Debug, Clone, Default)]
pub(crate) struct Links {
  /// The workbook's sheets, in order.
  pub sheets: Vec<String>,
  /// The names the workbook defines, in the order it lists them.
  pub names: Vec<String>,
  pub sheet_spans: Vec<SheetSpan>,
  pub books: Vec<Book>,
}

/// The last row and the last column a sheet of this format has.
const LAST_ROW: u16 = 65535;
const LAST_COLUMN: u16 = 255;

/// The tokens of a formula, read from the front.
struct Tokens<'a> {
  rest: &'a [u8],
}

impl<'a> Tokens<'a> {
  fn bytes(&mut self, count: usize) -> Option<&'a [u8]> {
    let taken = self.rest.get(..count)?;
    self.rest = &self.rest[count..];
    Some(taken)
  }

  fn u8(&mut self) -> Option<u8> {
    Some(self.bytes(1)?[0])
  }

  fn u16(&mut self) -> Option<u16> {
    Some(u16::from_le_bytes(self.bytes(2)?.try_into().ok()?))
  }

  fn u32(&mut self) -> Option<u32> {
    Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
  }

  fn f64(&mut self) -> Option<f64> {
    Some(f64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
  }

  /// Text with its count in one byte or two, and a byte that says whether each character is
  /// one byte or two.
  fn string(&mut self, count: usize) -> Option<String> {
    let wide = self.u8()? & 0x01 != 0;
    let units: Vec<u16> = match wide {
      true => self
        .bytes(count.checked_mul(2)?)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect(),
      false => self
        .bytes(count)?
        .iter()
        .map(|byte| u16::from(*byte))
        .collect(),
    };
    Some(String::from_utf16_lossy(&units))
  }
}

/// One corner of a reference: its row, its column, and which of the two move when the
/// formula is copied.
#[derive(Clone, Copy)]
struct Corner {
  row: u16,
  col: u16,
  row_moves: bool,
  col_moves: bool,
}

impl Corner {
  /// A corner as the file keeps it: the row, and the column with two flags above it.
  fn read(row: u16, col: u16) -> Corner {
    Corner {
      row,
      col: col & 0x00FF,
      row_moves: col & 0x8000 != 0,
      col_moves: col & 0x4000 != 0,
    }
  }

  /// The same of a formula many cells share: where a part moves, the file keeps the step
  /// from the cell that uses the formula, and a step off one edge of the sheet comes round
  /// to the other.
  fn stepped(self, base: (u32, u32)) -> Corner {
    let row = match self.row_moves {
      true => u16::try_from((u32::from(self.row) + base.0) % 65536).unwrap_or(self.row),
      false => self.row,
    };
    let col = match self.col_moves {
      true => u16::try_from((u32::from(self.col) + base.1) % 256).unwrap_or(self.col),
      false => self.col,
    };
    Corner { row, col, ..self }
  }

  fn column(self) -> String {
    let fixed = if self.col_moves { "" } else { "$" };
    format!("{fixed}{}", column_letters(u32::from(self.col)))
  }

  fn row(self) -> String {
    let fixed = if self.row_moves { "" } else { "$" };
    format!("{fixed}{}", u32::from(self.row) + 1)
  }

  fn cell(self) -> String {
    format!("{}{}", self.column(), self.row())
  }
}

/// A range by its two corners. One of whole columns names no rows, and one of whole rows
/// no columns.
fn range(first: Corner, last: Corner) -> String {
  if first.row == 0 && last.row == LAST_ROW {
    format!("{}:{}", first.column(), last.column())
  } else if first.col == 0 && last.col == LAST_COLUMN {
    format!("{}:{}", first.row(), last.row())
  } else {
    format!("{}:{}", first.cell(), last.cell())
  }
}

/// An error by the code a sheet shows for it.
fn error(code: u8) -> &'static str {
  match code {
    0x00 => "#NULL!",
    0x07 => "#DIV/0!",
    0x0F => "#VALUE!",
    0x17 => "#REF!",
    0x1D => "#NAME?",
    0x24 => "#NUM!",
    _ => "#N/A",
  }
}

/// Text as a formula writes it: in quotation marks, with a mark inside it written twice.
fn quoted(text: &str) -> String {
  format!("\"{}\"", text.replace('"', "\"\""))
}

/// A sheet's name as a reference writes it. A name that is not plain letters, digits, and
/// underscores, or that opens with a digit, or that reads as a cell (`Q4`), is put in
/// quotation marks, with a mark inside it written twice.
fn sheet_name(name: &str) -> String {
  let plain = name.chars().all(|c| c.is_alphanumeric() || c == '_');
  let opens_with_digit = name.chars().next().is_none_or(|c| c.is_ascii_digit());
  let letters = name.chars().take_while(char::is_ascii_alphabetic).count();
  let reads_as_cell = (1..=3).contains(&letters)
    && letters < name.len()
    && name[letters..].chars().all(|c| c.is_ascii_digit());
  if plain && !opens_with_digit && !reads_as_cell {
    name.to_string()
  } else {
    format!("'{}'", name.replace('\'', "''"))
  }
}

impl Links {
  /// What goes before the `!` of a reference to other sheets. Sheets of another workbook
  /// are named after that book's place among those the file links to, in brackets.
  fn sheets_of(&self, span: u16) -> Option<String> {
    let span = self.sheet_spans.get(usize::from(span))?;
    let (sheets, book) = match self.books.get(span.book)? {
      Book::Own => (&self.sheets, String::new()),
      Book::Other { sheets, .. } => (sheets, format!("[{}]", self.linked(span.book))),
      Book::AddIn { .. } => return None,
    };
    let named = |index: i32| {
      usize::try_from(index)
        .ok()
        .and_then(|index| sheets.get(index))
    };
    let (Some(first), Some(last)) = (named(span.first), named(span.last)) else {
      // The sheet has been deleted.
      return Some("#REF".to_string());
    };
    // The book's number is inside the quotation marks when there are any.
    let name = match span.first == span.last {
      true => format!("{book}{first}"),
      false => format!("{book}{first}:{last}"),
    };
    Some(match span.first == span.last && book.is_empty() {
      true => sheet_name(first),
      false
        if name
          .chars()
          .all(|c| c.is_alphanumeric() || "_[]:".contains(c)) =>
      {
        name
      }
      false => format!("'{}'", name.replace('\'', "''")),
    })
  }

  /// The place of a book among the other workbooks the file links to, from 1.
  fn linked(&self, book: usize) -> usize {
    self.books[..=book]
      .iter()
      .filter(|book| matches!(book, Book::Other { .. }))
      .count()
  }

  /// A name another book defines, or a function an add-in gives, by its place in that
  /// book's list, from 1.
  fn name_of(&self, span: u16, index: u32) -> Option<String> {
    let span = self.sheet_spans.get(usize::from(span))?;
    let index = usize::try_from(index).ok()?.checked_sub(1)?;
    match self.books.get(span.book)? {
      Book::Own => self.names.get(index).cloned(),
      Book::AddIn { names } => names.get(index).cloned(),
      Book::Other { names, .. } => Some(format!(
        "[{}]!{}",
        self.linked(span.book),
        names.get(index)?
      )),
    }
  }
}

/// A list of constants, from what follows a formula's tokens: how many columns and rows it
/// has, each less one, and then its values, a row at a time.
fn constants(extra: &mut Tokens<'_>) -> Option<String> {
  let columns = usize::from(extra.u8()?) + 1;
  let rows = usize::from(extra.u16()?) + 1;
  let mut written = Vec::with_capacity(rows);
  for _ in 0..rows {
    let mut row = Vec::with_capacity(columns);
    for _ in 0..columns {
      row.push(match extra.u8()? {
        0x01 => number(extra.f64()?),
        0x02 => {
          let count = usize::from(extra.u16()?);
          quoted(&extra.string(count)?)
        }
        0x04 => {
          let value = extra.bytes(8)?[0];
          if value != 0 { "TRUE" } else { "FALSE" }.to_string()
        }
        0x10 => error(extra.bytes(8)?[0]).to_string(),
        // Nothing in this place.
        0x00 => {
          extra.bytes(8)?;
          String::new()
        }
        _ => return None,
      });
    }
    written.push(row.join(","));
  }
  Some(format!("{{{}}}", written.join(";")))
}

/// A number as a formula writes it: a whole number without a point.
fn number(value: f64) -> String {
  value.to_string()
}

/// The text of a formula, without the `=` before it. `extra` is what follows the tokens in
/// their record, which holds the lists of constants a formula uses. `base` is the cell the
/// formula is written for, which a formula many cells share takes its steps from. None for
/// tokens that are not a formula this release can write out.
pub(crate) fn text(tokens: &[u8], extra: &[u8], base: (u32, u32), links: &Links) -> Option<String> {
  let mut tokens = Tokens { rest: tokens };
  let mut extra = Tokens { rest: extra };
  // What has been written and waits for the operator or the function that works on it.
  let mut waiting: Vec<String> = Vec::new();
  while !tokens.rest.is_empty() {
    let token = tokens.u8()?;
    // A token for a thing the formula works on comes in three kinds, which say how the
    // thing is to be taken and not what it is.
    let kind = match token {
      0x20.. => token & 0x1F | 0x20,
      _ => token,
    };
    let join = |waiting: &mut Vec<String>, sign: &str| -> Option<()> {
      let right = waiting.pop()?;
      let left = waiting.pop()?;
      waiting.push(format!("{left}{sign}{right}"));
      Some(())
    };
    let call = |waiting: &mut Vec<String>, name: &str, count: usize| -> Option<()> {
      let from = waiting.len().checked_sub(count)?;
      let given = waiting.split_off(from).join(",");
      waiting.push(format!("{name}({given})"));
      Some(())
    };
    match kind {
      0x03 => join(&mut waiting, "+")?,
      0x04 => join(&mut waiting, "-")?,
      0x05 => join(&mut waiting, "*")?,
      0x06 => join(&mut waiting, "/")?,
      0x07 => join(&mut waiting, "^")?,
      0x08 => join(&mut waiting, "&")?,
      0x09 => join(&mut waiting, "<")?,
      0x0A => join(&mut waiting, "<=")?,
      0x0B => join(&mut waiting, "=")?,
      0x0C => join(&mut waiting, ">=")?,
      0x0D => join(&mut waiting, ">")?,
      0x0E => join(&mut waiting, "<>")?,
      // Where two ranges cross, two ranges together, and from one reference to another.
      0x0F => join(&mut waiting, " ")?,
      0x10 => join(&mut waiting, ",")?,
      0x11 => join(&mut waiting, ":")?,
      0x12 | 0x13 => {
        let sign = if kind == 0x12 { '+' } else { '-' };
        let on = waiting.pop()?;
        waiting.push(format!("{sign}{on}"));
      }
      0x14 => {
        let on = waiting.pop()?;
        waiting.push(format!("{on}%"));
      }
      0x15 => {
        let inside = waiting.pop()?;
        waiting.push(format!("({inside})"));
      }
      // An argument left out.
      0x16 => waiting.push(String::new()),
      0x17 => {
        let count = usize::from(tokens.u8()?);
        waiting.push(quoted(&tokens.string(count)?));
      }
      // A note for the program that works the formula out: where a choice jumps to, that
      // the formula changes with the time, the spaces it was typed with. One note is a sum
      // of one thing.
      0x19 => {
        let note = tokens.u8()?;
        let value = tokens.u16()?;
        if note & 0x04 != 0 {
          // The places a choice of many jumps to.
          tokens.bytes((usize::from(value) + 1) * 2)?;
        } else if note & 0x10 != 0 {
          call(&mut waiting, "SUM", 1)?;
        }
      }
      0x1C => waiting.push(error(tokens.u8()?).to_string()),
      0x1D => waiting.push(if tokens.u8()? != 0 { "TRUE" } else { "FALSE" }.to_string()),
      0x1E => waiting.push(tokens.u16()?.to_string()),
      0x1F => waiting.push(number(tokens.f64()?)),
      0x20 => {
        tokens.bytes(7)?;
        waiting.push(constants(&mut extra)?);
      }
      0x21 => {
        let (name, takes) = function(tokens.u16()?)?;
        call(&mut waiting, name, takes?)?;
      }
      0x22 => {
        let count = usize::from(tokens.u8()? & 0x7F);
        let which = tokens.u16()? & 0x7FFF;
        if which == NAMED_BY_ITS_FIRST {
          let from = waiting.len().checked_sub(count)?;
          let name = waiting.get(from)?.clone();
          call(&mut waiting, &name, count.checked_sub(1)?)?;
          // The name was the first of what the function was given.
          let written = waiting.pop()?;
          waiting.pop()?;
          waiting.push(written);
        } else {
          call(&mut waiting, function(which)?.0, count)?;
        }
      }
      0x23 => {
        let index = usize::try_from(tokens.u32()?).ok()?.checked_sub(1)?;
        waiting.push(links.names.get(index)?.clone());
      }
      0x24 | 0x2C => {
        let corner = Corner::read(tokens.u16()?, tokens.u16()?);
        let corner = if kind == 0x2C {
          corner.stepped(base)
        } else {
          corner
        };
        waiting.push(corner.cell());
      }
      0x25 | 0x2D => {
        let (first_row, last_row) = (tokens.u16()?, tokens.u16()?);
        let first = Corner::read(first_row, tokens.u16()?);
        let last = Corner::read(last_row, tokens.u16()?);
        waiting.push(match kind {
          0x2D => range(first.stepped(base), last.stepped(base)),
          _ => range(first, last),
        });
      }
      // What a part of the formula comes to, kept for the program that works it out. The
      // part itself follows as tokens of its own; one kind keeps a list after the formula.
      0x26 => {
        tokens.bytes(6)?;
        let count = usize::from(extra.u16()?);
        extra.bytes(count * 8)?;
      }
      0x27 | 0x28 => {
        tokens.bytes(6)?;
      }
      0x29 | 0x2E | 0x2F => {
        tokens.bytes(2)?;
      }
      // A reference to what has been deleted.
      0x2A => {
        tokens.bytes(4)?;
        waiting.push("#REF!".to_string());
      }
      0x2B => {
        tokens.bytes(8)?;
        waiting.push("#REF!".to_string());
      }
      0x39 => {
        let span = tokens.u16()?;
        waiting.push(links.name_of(span, tokens.u32()?)?);
      }
      0x3A => {
        let sheets = links.sheets_of(tokens.u16()?)?;
        let corner = Corner::read(tokens.u16()?, tokens.u16()?);
        waiting.push(format!("{sheets}!{}", corner.cell()));
      }
      0x3B => {
        let sheets = links.sheets_of(tokens.u16()?)?;
        let (first_row, last_row) = (tokens.u16()?, tokens.u16()?);
        let first = Corner::read(first_row, tokens.u16()?);
        let last = Corner::read(last_row, tokens.u16()?);
        waiting.push(format!("{sheets}!{}", range(first, last)));
      }
      0x3C | 0x3D => {
        let sheets = links.sheets_of(tokens.u16()?)?;
        tokens.bytes(if kind == 0x3C { 4 } else { 8 })?;
        waiting.push(format!("{sheets}!#REF!"));
      }
      // A formula that only says which cell it is shared from, and tokens this release
      // does not know.
      _ => return None,
    }
  }
  // A formula comes to one thing.
  match waiting.len() {
    1 => waiting.pop(),
    _ => None,
  }
}

#[cfg(test)]
mod tests;
