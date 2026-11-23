//! SpreadsheetML workbooks: an `.xlsx` read into a sparse cell model.
//!
//! [`Workbook::open`] takes the bytes of a file. The package layer (`ooxmldown`) opens the zip
//! within limits and resolves parts through relationships; `workbook` reads the parts into
//! sheets of typed cells; `format` renders display text from number-format codes and says which
//! codes are dates; `formula` writes a shared formula out for the cells that follow it;
//! `geometry` lays a sheet out in points; `meta` counts what the workbook holds. Nothing here
//! depends on Node.

mod address;
mod error;
mod format;
mod formula;
pub mod geometry;
pub mod layout;
mod meta;
mod source;
#[cfg(test)]
mod testing;
mod workbook;

pub use address::{CellRange, MAX_COLS, MAX_ROWS, cell_ref, column_letters, parse_cell_ref};
pub use error::SpreadsheetError;
pub use geometry::{
  CellBox, SheetColumn, SheetGeometry, SheetRow, column_width_points, sheet_geometry,
};
pub use layout::{SheetLayout, sheet_layout};
pub use meta::{Metadata, SheetInfo};
// What a workbook says about itself is `ooxmldown`'s type, so it is part of this crate's API.
pub use ooxmldown::DocProps;
pub use workbook::{
  Cell, CellValue, ColSpec, FontSpec, Horizontal, MAX_CELLS, MAX_EXPANDED_FORMULA_BYTES,
  MAX_FORMULA_CHARS, MAX_MERGES, RowSpec, Sheet, Stored, Styles, Vertical, Workbook, Xf,
  format_code, parse_workbook,
};
