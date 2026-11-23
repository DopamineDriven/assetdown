//! Workbooks in the format Excel wrote from 1997 to 2003: an `.xls` read into the cell model
//! `xlsxdown` gives, so that number formats, sheet geometry, pages, and invoices are read from
//! it by the code that reads them from an `.xlsx`.
//!
//! The file is a compound file (`cfbdown`) whose `Workbook` stream is a run of records.
//! `record` walks them, `text` reads their strings, `globals` reads the workbook's own
//! records (sheets, shared strings, fonts, formats, and names), and `sheet` reads each sheet's.
//!
//! A formula gives the value Excel last left beside it. Its text is not read: a cell with a
//! formula has `formula` set and empty, and one that follows a shared formula names the cell
//! that holds it. Nothing here depends on Node.

mod globals;
mod record;
mod sheet;
#[cfg(test)]
mod testing;
mod text;

use cfbdown::{CfbError, CompoundFile};
use xlsxdown::{DocProps, MAX_CELLS, MAX_MERGES, SpreadsheetError, Workbook};

use crate::sheet::Context;

/// The storage Excel keeps a workbook's macros in.
const MACROS: &str = "_VBA_PROJECT_CUR";

/// Whether the bytes begin as a worksheet of Excel 2, 3, or 4 does: records with no compound
/// file around them, the first of which says which version wrote it.
fn older_worksheet(bytes: &[u8]) -> bool {
  matches!(
    bytes,
    [0x09, 0x00, 0x04, 0x00, ..] | [0x09, 0x02 | 0x04, 0x06, 0x00, ..]
  )
}

/// Read a workbook from the bytes of an `.xls` file. A file of an older Excel, and one saved
/// with a password, is an unsupported format, named for what it is.
pub fn open(bytes: &[u8]) -> Result<Workbook, SpreadsheetError> {
  let file = match CompoundFile::open(bytes) {
    Ok(file) => file,
    Err(CfbError::NotCompound) if older_worksheet(bytes) => {
      return Err(SpreadsheetError::UnsupportedFormat(
        "an Excel 2, 3, or 4 worksheet".to_string(),
      ));
    }
    Err(error) => return Err(SpreadsheetError::Malformed(error.to_string())),
  };
  let Some(stream) = file.stream("Workbook") else {
    let named = |name: &str| {
      file
        .children("")
        .iter()
        .any(|child| !child.is_storage && child.name.eq_ignore_ascii_case(name))
    };
    return Err(if named("Workbook") {
      SpreadsheetError::Malformed("the workbook's records cannot be read".to_string())
    } else if named("Book") {
      // Excel 5 and 95 name the stream `Book`, and keep most records another way.
      SpreadsheetError::UnsupportedFormat("an Excel 5 or 95 workbook".to_string())
    } else {
      SpreadsheetError::UnsupportedFormat("a compound file that holds no workbook".to_string())
    });
  };
  let globals = globals::read(&stream)?;
  let context = Context {
    strings: &globals.strings,
    styles: &globals.styles,
    date1904: globals.date1904,
  };
  let mut sheets = Vec::with_capacity(globals.sheets.len());
  let mut cells_left = MAX_CELLS;
  let mut merges = 0usize;
  for (index, bound) in globals.sheets.iter().enumerate() {
    let print_area = globals.print_areas.get(&index).copied().flatten();
    let index = u32::try_from(index).unwrap_or(u32::MAX);
    let sheet = sheet::read(&stream, bound, index, context, print_area, &mut cells_left)?;
    merges += sheet.merges.len();
    if merges > MAX_MERGES {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the workbook has more than {MAX_MERGES} merged ranges"
      )));
    }
    sheets.push(sheet);
  }
  Ok(Workbook {
    sheets,
    date1904: globals.date1904,
    styles: globals.styles,
    props: DocProps::default(),
    defined_names: globals.defined_names,
    has_macros: file.is_storage(MACROS),
    is_template: false,
  })
}

#[cfg(test)]
mod tests;
