//! Workbooks for the tests, built a record at a time.

/// One record: its number, the length of its data, and the data.
pub(crate) fn record(id: u16, data: &[u8]) -> Vec<u8> {
  let mut out = id.to_le_bytes().to_vec();
  out.extend(u16::try_from(data.len()).unwrap().to_le_bytes());
  out.extend(data);
  out
}

fn joined(parts: &[&[u8]]) -> Vec<u8> {
  parts.concat()
}

/// The characters of a string after its count: a byte that says how they are kept, and then
/// one byte to a character when every one fits in a byte, or two.
pub(crate) fn characters(text: &str) -> Vec<u8> {
  let units: Vec<u16> = text.encode_utf16().collect();
  if units.iter().all(|unit| *unit < 256) {
    let mut out = vec![0u8];
    out.extend(units.iter().map(|unit| *unit as u8));
    out
  } else {
    let mut out = vec![1u8];
    out.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    out
  }
}

/// A string with its count in two bytes.
pub(crate) fn text(text: &str) -> Vec<u8> {
  let count = u16::try_from(text.encode_utf16().count()).unwrap();
  joined(&[&count.to_le_bytes(), &characters(text)])
}

/// A string with its count in one byte.
pub(crate) fn short_text(text: &str) -> Vec<u8> {
  let count = u8::try_from(text.encode_utf16().count()).unwrap();
  joined(&[&[count], &characters(text)])
}

pub(crate) const GLOBALS: u16 = 0x0005;
pub(crate) const WORKSHEET: u16 = 0x0010;
pub(crate) const CHART: u16 = 0x0020;

/// The record that opens a run of records, for the version of Excel 97 to 2003.
pub(crate) fn bof(kind: u16) -> Vec<u8> {
  bof_of(0x0600, kind)
}

pub(crate) fn bof_of(version: u16, kind: u16) -> Vec<u8> {
  let mut data = version.to_le_bytes().to_vec();
  data.extend(kind.to_le_bytes());
  data.extend([0u8; 12]);
  record(0x0809, &data)
}

pub(crate) fn eof() -> Vec<u8> {
  record(0x000A, &[])
}

fn cell(id: u16, row: u16, col: u16, xf: u16, value: &[u8]) -> Vec<u8> {
  record(
    id,
    &joined(&[
      &row.to_le_bytes(),
      &col.to_le_bytes(),
      &xf.to_le_bytes(),
      value,
    ]),
  )
}

pub(crate) fn number(row: u16, col: u16, xf: u16, value: f64) -> Vec<u8> {
  cell(0x0203, row, col, xf, &value.to_le_bytes())
}

pub(crate) fn rk(row: u16, col: u16, xf: u16, value: u32) -> Vec<u8> {
  cell(0x027E, row, col, xf, &value.to_le_bytes())
}

/// A row of numbers kept the short way, from the column `first` on, each with its style.
pub(crate) fn mulrk(row: u16, first: u16, values: &[(u16, u32)]) -> Vec<u8> {
  let mut data = row.to_le_bytes().to_vec();
  data.extend(first.to_le_bytes());
  for (xf, value) in values {
    data.extend(xf.to_le_bytes());
    data.extend(value.to_le_bytes());
  }
  let last = first + u16::try_from(values.len()).unwrap() - 1;
  data.extend(last.to_le_bytes());
  record(0x00BD, &data)
}

pub(crate) fn label_sst(row: u16, col: u16, xf: u16, index: u32) -> Vec<u8> {
  cell(0x00FD, row, col, xf, &index.to_le_bytes())
}

pub(crate) fn label(row: u16, col: u16, xf: u16, value: &str) -> Vec<u8> {
  cell(0x0204, row, col, xf, &text(value))
}

pub(crate) fn blank(row: u16, col: u16, xf: u16) -> Vec<u8> {
  cell(0x0201, row, col, xf, &[])
}

pub(crate) fn boolean(row: u16, col: u16, xf: u16, on: bool) -> Vec<u8> {
  cell(0x0205, row, col, xf, &[u8::from(on), 0])
}

pub(crate) fn error(row: u16, col: u16, xf: u16, code: u8) -> Vec<u8> {
  cell(0x0205, row, col, xf, &[code, 1])
}

/// The formula tokens of a cell that follows the shared formula whose first cell is here.
pub(crate) fn follows(row: u16, col: u16) -> Vec<u8> {
  joined(&[&[0x01], &row.to_le_bytes(), &col.to_le_bytes()])
}

/// A formula cell: the result it was left with, and its tokens.
pub(crate) fn formula(row: u16, col: u16, xf: u16, result: [u8; 8], tokens: &[u8]) -> Vec<u8> {
  let mut value = result.to_vec();
  value.extend([0u8; 6]);
  value.extend(u16::try_from(tokens.len()).unwrap().to_le_bytes());
  value.extend(tokens);
  cell(0x0006, row, col, xf, &value)
}

/// A formula's result that is not a number: 0 text (which follows in a record of its own),
/// 1 a boolean, 2 an error, 3 text with nothing in it.
pub(crate) fn result_of(kind: u8, value: u8) -> [u8; 8] {
  [kind, 0, value, 0, 0, 0, 0xFF, 0xFF]
}

/// The text a formula gave, in the record that follows the formula's.
pub(crate) fn string(value: &str) -> Vec<u8> {
  record(0x0207, &text(value))
}

/// The table of the strings cells share, in one record.
pub(crate) fn sst(strings: &[&str]) -> Vec<u8> {
  let count = u32::try_from(strings.len()).unwrap().to_le_bytes();
  let mut data = joined(&[&count, &count]);
  for string in strings {
    data.extend(text(string));
  }
  record(0x00FC, &data)
}

pub(crate) fn font(name: &str, twips: u16, bold: bool, italic: bool) -> Vec<u8> {
  let weight: u16 = if bold { 700 } else { 400 };
  let flags: u16 = if italic { 0x0002 } else { 0 };
  record(
    0x0031,
    &joined(&[
      &twips.to_le_bytes(),
      &flags.to_le_bytes(),
      &0x7FFFu16.to_le_bytes(),
      &weight.to_le_bytes(),
      &[0u8; 6],
      &short_text(name),
    ]),
  )
}

/// A number format the file defines: its number and its code.
pub(crate) fn format(id: u16, code: &str) -> Vec<u8> {
  record(0x041E, &joined(&[&id.to_le_bytes(), &text(code)]))
}

/// A cell format: the font and the number format it selects, how it is aligned (the byte the
/// file keeps: three bits across, one for wrapping, three for up and down), and its indent.
pub(crate) fn xf(font: u16, format: u16, alignment: u8, indent: u8) -> Vec<u8> {
  let mut data = joined(&[&font.to_le_bytes(), &format.to_le_bytes(), &[0u8; 2]]);
  data.extend([alignment, 0, indent]);
  data.resize(20, 0);
  record(0x00E0, &data)
}

/// A name the workbook defines for the sheet numbered `sheet` from 1, or for none with 0.
pub(crate) fn name(builtin: Option<u8>, own: &str, sheet: u16, tokens: &[u8]) -> Vec<u8> {
  let (flags, spelled): (u16, Vec<u8>) = match builtin {
    Some(code) => (0x0020, vec![0, code]),
    None => (0, characters(own)),
  };
  let count = match builtin {
    Some(_) => 1,
    None => u8::try_from(own.encode_utf16().count()).unwrap(),
  };
  let mut data = flags.to_le_bytes().to_vec();
  data.extend([0, count]);
  data.extend(u16::try_from(tokens.len()).unwrap().to_le_bytes());
  data.extend([0u8; 2]);
  data.extend(sheet.to_le_bytes());
  data.extend([0u8; 4]);
  data.extend(spelled);
  data.extend(tokens);
  record(0x0018, &data)
}

/// The token for a range of one sheet: rows `rows.0` to `rows.1`, columns `cols.0` to `cols.1`.
pub(crate) fn area(rows: (u16, u16), cols: (u16, u16)) -> Vec<u8> {
  joined(&[
    &[0x3B],
    &0u16.to_le_bytes(),
    &rows.0.to_le_bytes(),
    &rows.1.to_le_bytes(),
    &cols.0.to_le_bytes(),
    &cols.1.to_le_bytes(),
  ])
}

/// The first and last row and column that a sheet says it uses, the last ones one past.
pub(crate) fn dimensions(rows: (u32, u32), cols: (u16, u16)) -> Vec<u8> {
  record(
    0x0200,
    &joined(&[
      &rows.0.to_le_bytes(),
      &rows.1.to_le_bytes(),
      &cols.0.to_le_bytes(),
      &cols.1.to_le_bytes(),
      &[0u8; 2],
    ]),
  )
}

/// The columns from `first` to `last`: their width in 256ths of a character, and whether
/// they are hidden.
pub(crate) fn colinfo(first: u16, last: u16, width: u16, hidden: bool) -> Vec<u8> {
  record(
    0x007D,
    &joined(&[
      &first.to_le_bytes(),
      &last.to_le_bytes(),
      &width.to_le_bytes(),
      &0u16.to_le_bytes(),
      &u16::from(hidden).to_le_bytes(),
      &[0u8; 2],
    ]),
  )
}

/// A row: its height as the file keeps it (twips, with the top bit set for a row left at the
/// default), and whether it is hidden.
pub(crate) fn row(index: u16, height: u16, hidden: bool) -> Vec<u8> {
  let flags: u16 = if hidden { 0x0020 } else { 0 };
  record(
    0x0208,
    &joined(&[
      &index.to_le_bytes(),
      &[0u8; 4],
      &height.to_le_bytes(),
      &[0u8; 4],
      &flags.to_le_bytes(),
      &[0u8; 2],
    ]),
  )
}

/// Merged ranges, each rows first to last and columns first to last.
pub(crate) fn merged(ranges: &[(u16, u16, u16, u16)]) -> Vec<u8> {
  let mut data = u16::try_from(ranges.len()).unwrap().to_le_bytes().to_vec();
  for (r0, r1, c0, c1) in ranges {
    for value in [r0, r1, c0, c1] {
      data.extend(value.to_le_bytes());
    }
  }
  record(0x00E5, &data)
}

/// An object on the sheet, of the kind the file numbers: 8 a picture, 5 a chart, 25 a note.
pub(crate) fn object(kind: u16) -> Vec<u8> {
  let mut data = joined(&[
    &0x0015u16.to_le_bytes(),
    &0x0012u16.to_le_bytes(),
    &kind.to_le_bytes(),
  ]);
  data.resize(22, 0);
  record(0x005D, &data)
}

/// A sheet of a workbook under construction.
pub(crate) struct SheetSpec {
  name: String,
  /// 0 shown, 1 hidden, 2 hidden so that only a program can show it.
  visibility: u8,
  kind: u16,
  records: Vec<u8>,
}

/// A workbook under construction: the records of the workbook itself, and its sheets.
#[derive(Default)]
pub(crate) struct Book {
  globals: Vec<u8>,
  sheets: Vec<SheetSpec>,
  version: Option<u16>,
}

impl Book {
  pub fn new() -> Self {
    Book::default()
  }

  /// A record of the workbook itself.
  pub fn global(mut self, record: Vec<u8>) -> Self {
    self.globals.extend(record);
    self
  }

  pub fn sheet(self, name: &str, records: &[Vec<u8>]) -> Self {
    self.sheet_of(name, 0, WORKSHEET, records)
  }

  pub fn sheet_of(mut self, name: &str, visibility: u8, kind: u16, records: &[Vec<u8>]) -> Self {
    self.sheets.push(SheetSpec {
      name: name.to_string(),
      visibility,
      kind,
      records: records.concat(),
    });
    self
  }

  /// The version the workbook says it is, where a test wants one that is not Excel 97's.
  pub fn version(mut self, version: u16) -> Self {
    self.version = Some(version);
    self
  }

  /// The record that lists a sheet: where its records start, how it is shown, and its name.
  fn bound(sheet: &SheetSpec, at: usize) -> Vec<u8> {
    let kind = match sheet.kind {
      CHART => 2,
      _ => 0,
    };
    record(
      0x0085,
      &joined(&[
        &u32::try_from(at).unwrap().to_le_bytes(),
        &[sheet.visibility, kind],
        &short_text(&sheet.name),
      ]),
    )
  }

  /// The stream of records: the workbook's own, and then each sheet's.
  pub fn stream(&self) -> Vec<u8> {
    let version = self.version.unwrap_or(0x0600);
    let listed: usize = self
      .sheets
      .iter()
      .map(|sheet| Book::bound(sheet, 0).len())
      .sum();
    let mut at = bof(GLOBALS).len() + self.globals.len() + listed + eof().len();
    let mut out = bof_of(version, GLOBALS);
    out.extend(&self.globals);
    let mut bodies = Vec::new();
    for sheet in &self.sheets {
      out.extend(Book::bound(sheet, at));
      let body = joined(&[&bof_of(version, sheet.kind), &sheet.records, &eof()]);
      at += body.len();
      bodies.push(body);
    }
    out.extend(eof());
    out.extend(bodies.concat());
    out
  }

  /// The file: a compound file with the records in its `Workbook` stream.
  pub fn file(&self) -> Vec<u8> {
    cfbdown::compound(&[("Workbook", &self.stream())]).unwrap()
  }
}

/// A property stream of one set, in the Western code page: each property a number and
/// either text or a moment, in tenths of a microsecond since 1601.
pub(crate) fn properties(stated: &[(u32, Result<&str, u64>)]) -> Vec<u8> {
  let mut values: Vec<Vec<u8>> = vec![joined(&[
    &2u32.to_le_bytes(),
    &1252u16.to_le_bytes(),
    &[0; 2],
  ])];
  for (_, value) in stated {
    let mut kept = match value {
      Ok(text) => joined(&[
        &0x1Eu32.to_le_bytes(),
        &u32::try_from(text.len() + 1).unwrap().to_le_bytes(),
        text.as_bytes(),
        &[0],
      ]),
      Err(ticks) => joined(&[&0x40u32.to_le_bytes(), &ticks.to_le_bytes()]),
    };
    kept.resize(kept.len().next_multiple_of(4), 0);
    values.push(kept);
  }
  // The set: its length, how many properties, where each is, and then the values. The
  // first property is the code page.
  let ids = std::iter::once(1).chain(stated.iter().map(|(id, _)| *id));
  let mut at = 8 + values.len() * 8;
  let mut listed = Vec::new();
  for (id, value) in ids.zip(&values) {
    listed.extend(id.to_le_bytes());
    listed.extend(u32::try_from(at).unwrap().to_le_bytes());
    at += value.len();
  }
  let mut out = vec![0xFE, 0xFF, 0, 0];
  out.extend([0u8; 20]);
  out.extend(1u32.to_le_bytes());
  out.extend([0u8; 16]);
  out.extend(48u32.to_le_bytes());
  out.extend(u32::try_from(at).unwrap().to_le_bytes());
  out.extend(u32::try_from(values.len()).unwrap().to_le_bytes());
  out.extend(listed);
  out.extend(values.concat());
  out
}

/// The record that defines a formula many cells share, after the first of them: the rows
/// and the columns it covers, and its tokens.
pub(crate) fn shared_formula(rows: (u16, u16), cols: (u8, u8), tokens: &[u8]) -> Vec<u8> {
  record(
    0x04BC,
    &joined(&[
      &rows.0.to_le_bytes(),
      &rows.1.to_le_bytes(),
      &[cols.0, cols.1, 0, 0],
      &u16::try_from(tokens.len()).unwrap().to_le_bytes(),
      tokens,
    ]),
  )
}

/// The record that defines a formula a range of cells is filled by together.
pub(crate) fn array_formula(rows: (u16, u16), cols: (u8, u8), tokens: &[u8]) -> Vec<u8> {
  record(
    0x0221,
    &joined(&[
      &rows.0.to_le_bytes(),
      &rows.1.to_le_bytes(),
      &[cols.0, cols.1],
      &[0u8; 6],
      &u16::try_from(tokens.len()).unwrap().to_le_bytes(),
      tokens,
    ]),
  )
}

/// The workbook's own entry among the books its formulas refer to.
pub(crate) fn own_book(sheets: u16) -> Vec<u8> {
  record(
    0x01AE,
    &joined(&[&sheets.to_le_bytes(), &0x0401u16.to_le_bytes()]),
  )
}

/// The entry for the functions add-ins give, and one such function's name after it.
pub(crate) fn add_in_book(function: &str) -> Vec<u8> {
  joined(&[
    &record(
      0x01AE,
      &joined(&[&1u16.to_le_bytes(), &0x3A01u16.to_le_bytes()]),
    ),
    &record(0x0023, &joined(&[&[0u8; 6], &short_text(function)])),
  ])
}

/// Which sheets references name: for each, a book by its place, and its first and last sheet.
pub(crate) fn sheet_spans(spans: &[(u16, i16, i16)]) -> Vec<u8> {
  let mut data = u16::try_from(spans.len()).unwrap().to_le_bytes().to_vec();
  for (book, first, last) in spans {
    data.extend(book.to_le_bytes());
    data.extend(first.to_le_bytes());
    data.extend(last.to_le_bytes());
  }
  record(0x0017, &data)
}
