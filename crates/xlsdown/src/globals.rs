//! The workbook's own records, which come before any sheet's: the sheets and where each
//! starts, the strings cells share, fonts, number formats, cell formats, the date system, and
//! the names the workbook defines.

use std::collections::HashMap;

use xlsxdown::{CellRange, FontSpec, Horizontal, SpreadsheetError, Styles, Vertical, Xf};

use crate::formula::{Book, Links, SheetSpan};
use crate::record::{BOF, EOF, Record, Records};
use crate::text::Parts;

/// The version a workbook of Excel 97 to 2003 states, and the one Excel 5 and 95 state.
const EXCEL_97: u16 = 0x0600;
const EXCEL_5: u16 = 0x0500;
/// A font may not be larger than this many points.
const MAX_FONT_SIZE: f64 = 409.0;
/// The built-in name of a sheet's print area.
const PRINT_AREA: u8 = 0x06;

/// One sheet as the workbook lists it.
pub(crate) struct Bound {
  pub name: String,
  pub hidden: bool,
  /// Where the sheet's records start in the stream.
  pub at: usize,
}

pub(crate) struct Globals {
  pub date1904: bool,
  pub styles: Styles,
  pub strings: Vec<String>,
  pub sheets: Vec<Bound>,
  /// The print area of the sheet at each index that has one named: a range, or `None` when
  /// the name is of more than one.
  pub print_areas: HashMap<usize, Option<CellRange>>,
  /// Names the workbook defines, the built-in ones left out.
  pub defined_names: usize,
  /// What a formula's references are resolved against: the sheets, the names, and the books
  /// formulas refer to.
  pub links: Links,
}

fn malformed(detail: &str) -> SpreadsheetError {
  SpreadsheetError::Malformed(detail.to_string())
}

fn font(record: Record<'_>) -> Option<FontSpec> {
  let twips = record.u16_at(0)?;
  let flags = record.u16_at(2)?;
  let weight = record.u16_at(6)?;
  Some(FontSpec {
    name: Parts::one(record.data.get(14..)?).short_string()?,
    size: (f64::from(twips) / 20.0).min(MAX_FONT_SIZE),
    bold: weight >= 700,
    italic: flags & 0x0002 != 0,
  })
}

fn xf(record: Record<'_>) -> Option<Xf> {
  let font = usize::from(record.u16_at(0)?);
  let alignment = *record.data.get(6)?;
  Some(Xf {
    num_fmt_id: u32::from(record.u16_at(2)?),
    // The file numbers its fonts with no fourth: the fifth it lists is numbered five.
    font_id: if font > 4 { font - 1 } else { font },
    horizontal: match alignment & 0x07 {
      1 => Horizontal::Left,
      // Centered, and centered across a selection.
      2 | 6 => Horizontal::Center,
      3 => Horizontal::Right,
      _ => Horizontal::General,
    },
    vertical: match (alignment >> 4) & 0x07 {
      0 => Vertical::Top,
      1 => Vertical::Center,
      _ => Vertical::Bottom,
    },
    wrap: alignment & 0x08 != 0,
    indent: u32::from(record.data.get(8)? & 0x0F),
  })
}

fn bound(record: Record<'_>) -> Option<Bound> {
  Some(Bound {
    at: usize::try_from(record.u32_at(0)?).ok()?,
    hidden: record.data.get(4)? & 0x03 != 0,
    name: Parts::one(record.data.get(6..)?).short_string()?,
  })
}

/// The range a name's formula is, when the formula is one range of one sheet and no more.
fn one_area(tokens: &[u8]) -> Option<CellRange> {
  let [0x3B | 0x5B | 0x7B, _, _, r0a, r0b, r1a, r1b, c0, _, c1, _] = tokens else {
    return None;
  };
  Some(CellRange {
    r0: u32::from(u16::from_le_bytes([*r0a, *r0b])),
    r1: u32::from(u16::from_le_bytes([*r1a, *r1b])),
    // The column is the low byte; the bits above it say whether the reference moves.
    c0: u32::from(*c0),
    c1: u32::from(*c1),
  })
}

/// A name the workbook lists: how a formula spells it, the sheet it is for (counted from 1,
/// or nought for the whole workbook), and its own formula. A built-in name is kept as a code.
struct Name<'a> {
  text: String,
  builtin: Option<u8>,
  sheet: u16,
  tokens: &'a [u8],
}

/// A built-in name, as a sheet spells it.
fn builtin_name(code: u8) -> &'static str {
  match code {
    0 => "Consolidate_Area",
    1 => "Auto_Open",
    2 => "Auto_Close",
    3 => "Extract",
    4 => "Database",
    5 => "Criteria",
    6 => "Print_Area",
    7 => "Print_Titles",
    8 => "Recorder",
    9 => "Data_Form",
    10 => "Auto_Activate",
    11 => "Auto_Deactivate",
    12 => "Sheet_Title",
    _ => "_FilterDatabase",
  }
}

fn name(record: Record<'_>) -> Option<Name<'_>> {
  let builtin = record.u16_at(0)? & 0x0020 != 0;
  let characters = usize::from(*record.data.get(3)?);
  let formula = usize::from(record.u16_at(4)?);
  let sheet = record.u16_at(8)?;
  // The name follows a byte that says how its characters are kept. A built-in name is one
  // character: its code.
  let wide = record.data.get(14)? & 0x01 != 0;
  let after = 15 + characters * if wide { 2 } else { 1 };
  let spelled = record.data.get(15..after)?;
  let units: Vec<u16> = match wide {
    true => spelled
      .as_chunks::<2>()
      .0
      .iter()
      .map(|pair| u16::from_le_bytes(*pair))
      .collect(),
    false => spelled.iter().map(|byte| u16::from(*byte)).collect(),
  };
  let code = builtin.then(|| spelled.first().copied()).flatten();
  Some(Name {
    text: match code {
      Some(code) => builtin_name(code).to_string(),
      None => String::from_utf16_lossy(&units),
    },
    builtin: code,
    sheet,
    tokens: record.data.get(after..after + formula).unwrap_or(&[]),
  })
}

/// A book formulas refer to: the workbook itself, the functions add-ins give, or another
/// workbook, whose sheets are listed after where it is kept.
fn book(record: Record<'_>, more: Vec<&[u8]>) -> Book {
  match record.u16_at(2) {
    Some(0x0401) => Book::Own,
    Some(0x3A01) => Book::AddIn { names: Vec::new() },
    _ => {
      let count = record.u16_at(0).unwrap_or(0);
      let mut parts = Parts::of(record.data.get(2..).unwrap_or(&[]), more);
      let _kept_at = parts.string();
      Book::Other {
        sheets: (0..count).map_while(|_| parts.string()).collect(),
        names: Vec::new(),
      }
    }
  }
}

/// The strings cells share: a count of them all, a count of the different ones, and the
/// strings, which run on across as many records as they need.
fn shared_strings(first: &[u8], more: Vec<&[u8]>) -> Vec<String> {
  let mut parts = Parts::of(first, more);
  let Some(count) = parts.u32().and_then(|_| parts.u32()) else {
    return Vec::new();
  };
  // A string that cannot be read ends the table: nothing after it can be found.
  (0..count).map_while(|_| parts.string()).collect()
}

/// The workbook's own records, from the start of the stream to the end of their run.
pub(crate) fn read(stream: &[u8]) -> Result<Globals, SpreadsheetError> {
  let mut records = Records::from(stream, 0);
  let version = records
    .next()
    .filter(|record| record.id == BOF)
    .and_then(|record| record.u16_at(0))
    .ok_or_else(|| malformed("the workbook's records do not begin as a workbook's do"))?;
  match version {
    EXCEL_97 => {}
    EXCEL_5 => {
      return Err(SpreadsheetError::UnsupportedFormat(
        "an Excel 5 or 95 workbook".to_string(),
      ));
    }
    _ => {
      return Err(malformed(
        "the workbook states a version that is not one of Excel's",
      ));
    }
  }
  let mut globals = Globals {
    date1904: false,
    styles: Styles::default(),
    strings: Vec::new(),
    sheets: Vec::new(),
    print_areas: HashMap::new(),
    defined_names: 0,
    links: Links::default(),
  };
  let mut fonts = Vec::new();
  while let Some(record) = records.next() {
    match record.id {
      EOF => break,
      0x0022 => globals.date1904 = record.u16_at(0) == Some(1),
      0x0031 => fonts.extend(font(record)),
      0x041E => {
        let code = record
          .data
          .get(2..)
          .and_then(|data| Parts::one(data).string());
        if let (Some(id), Some(code)) = (record.u16_at(0), code) {
          globals.styles.custom_formats.insert(u32::from(id), code);
        }
      }
      // A cell names its format by its place among these, so one that cannot be read still
      // takes its place.
      0x00E0 => globals.styles.xfs.push(xf(record).unwrap_or(Xf {
        num_fmt_id: 0,
        font_id: 0,
        horizontal: Horizontal::General,
        vertical: Vertical::Bottom,
        wrap: false,
        indent: 0,
      })),
      0x0085 => globals.sheets.extend(bound(record)),
      0x00FC => globals.strings = shared_strings(record.data, records.continued()),
      0x0018 => match name(record) {
        Some(named) => {
          match named.builtin {
            None => globals.defined_names += 1,
            Some(PRINT_AREA) if named.sheet > 0 => {
              globals
                .print_areas
                .insert(usize::from(named.sheet) - 1, one_area(named.tokens));
            }
            Some(_) => {}
          }
          globals.links.names.push(named.text);
        }
        // A formula names a name by its place among these, so one that cannot be read
        // still takes its place.
        None => globals.links.names.push("#NAME?".to_string()),
      },
      0x01AE => {
        let more = records.continued();
        globals.links.books.push(book(record, more));
      }
      // A name of the book listed last: a function an add-in gives, or a name another
      // workbook defines. It follows six bytes about it, with its length in one byte.
      0x0023 => {
        let named = record
          .data
          .get(6..)
          .and_then(|data| Parts::one(data).short_string())
          .unwrap_or_default();
        if let Some(Book::AddIn { names } | Book::Other { names, .. }) =
          globals.links.books.last_mut()
        {
          names.push(named);
        }
      }
      // Which sheets references name: a count, and for each a book and its first and last
      // sheet.
      0x0017 => {
        let mut data = record.data.to_vec();
        for more in records.continued() {
          data.extend_from_slice(more);
        }
        let count = usize::from(record.u16_at(0).unwrap_or(0));
        let spans = data.get(2..).unwrap_or(&[]).as_chunks::<6>().0;
        globals.links.sheet_spans = spans
          .iter()
          .take(count)
          .map(|span| SheetSpan {
            book: usize::from(u16::from_le_bytes([span[0], span[1]])),
            first: i32::from(i16::from_le_bytes([span[2], span[3]])),
            last: i32::from(i16::from_le_bytes([span[4], span[5]])),
          })
          .collect();
      }
      _ => {}
    }
  }
  if let Some(first) = fonts.first() {
    globals.styles.normal_font = first.clone();
    globals.styles.fonts = fonts;
  }
  globals.links.sheets = globals
    .sheets
    .iter()
    .map(|sheet| sheet.name.clone())
    .collect();
  Ok(globals)
}
