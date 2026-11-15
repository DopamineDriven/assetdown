//! The shapes JavaScript sees. Plain data: each value is built from `xlsxdown`'s model in `cells`
//! and copied out. Only the options are ever read back from JavaScript.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use napi_derive::napi;

/// Which sheets and cells a reader returns.
#[derive(Debug, Clone, Default)]
#[napi(object)]
pub struct SpreadsheetOptions {
  /// Sheet names to include, returned in workbook order; default: every visible sheet. A sheet
  /// asked for by name is included even when it is hidden.
  pub sheets: Option<Vec<String>>,
  /// Include hidden sheets (default false).
  pub hidden_sheets: Option<bool>,
  /// Cells in hidden rows and columns (default true): they are listed with `hidden: true`. With
  /// false they are left out, and their rows and columns have no extent.
  pub hidden_cells: Option<bool>,
}

/// Whether a Number cell's value is a whole number JavaScript can hold exactly, or a fraction.
/// A percentage is a fraction (50% is 0.5); the subtype is about the value, not the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum NumberSubtype {
  Integer,
  Float,
}

/// A rectangle in the sheet frame: points, y up, with A1's top-left corner at `(0, height)`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[napi(object)]
pub struct CellBox {
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

impl From<xlsxdown::CellBox> for CellBox {
  fn from(rectangle: xlsxdown::CellBox) -> Self {
    CellBox {
      x0: rectangle.x0,
      y0: rectangle.y0,
      x1: rectangle.x1,
      y1: rectangle.y1,
    }
  }
}

/// A column whose width differs from the sheet default, or that is hidden. 0-based.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct SheetColumn {
  pub index: u32,
  /// Points; zero for a hidden column when hidden cells are excluded.
  pub width: f64,
  pub hidden: bool,
}

impl From<xlsxdown::SheetColumn> for SheetColumn {
  fn from(column: xlsxdown::SheetColumn) -> Self {
    SheetColumn {
      index: column.index,
      width: column.width,
      hidden: column.hidden,
    }
  }
}

/// A row whose height differs from the sheet default (auto-fit growth included), or that is
/// hidden. 0-based.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct SheetRow {
  pub index: u32,
  /// Points; zero for a hidden row when hidden cells are excluded.
  pub height: f64,
  pub hidden: bool,
}

impl From<xlsxdown::SheetRow> for SheetRow {
  fn from(row: xlsxdown::SheetRow) -> Self {
    SheetRow {
      index: row.index,
      height: row.height,
      hidden: row.hidden,
    }
  }
}

/// The fields every cell carries, however it is typed.
pub(crate) struct CellCommon {
  pub reference: String,
  pub row: u32,
  pub col: u32,
  pub text: String,
  pub raw: String,
  pub format: Option<String>,
  pub format_supported: bool,
  pub formula: Option<String>,
  pub shared_formula: Option<String>,
  pub cached: bool,
  pub merge: Option<String>,
  pub hidden: bool,
  pub r#box: CellBox,
}

macro_rules! sheet_cell {
  // Variant payloads are written in brackets and forwarded as raw token trees, so that the napi
  // attribute sees the types themselves rather than an opaque fragment.
  ($($variant:ident, $ctor:ident { $($field:ident : [$($ty:tt)+]),* $(,)? }),+ $(,)?) => {
    /// One cell of a worksheet: a discriminated union on `type` whose typed value is `value`
    /// (none for `Blank`). A Number carries the full double plus its `subtype` and the
    /// `currencySymbol` its format names. A Date carries an ISO date (`YYYY-MM-DD`) or date-time
    /// (`YYYY-MM-DDTHH:MM:SS`) and a Time a clock (`HH:MM:SS`), all 24-hour and with no time
    /// zone, because a workbook records none.
    ///
    /// `text` is the display text and `raw` the stored spelling (the serial for dates and
    /// times). `format` is the cell's number-format code; `formatSupported` says whether `text`
    /// was rendered from it, and when false `text` is the stored spelling.
    ///
    /// `formula` is the cell's formula without the leading `=`. On a cell that follows a shared
    /// formula it is the master's formula with its relative references moved to this cell, and
    /// `sharedFormula` names the master. `cached` says whether a result was stored.
    ///
    /// `merge` is the range this cell anchors, `hidden` whether its row or column is hidden, and
    /// `box` its rectangle in the sheet frame (a merged anchor spans its range).
    #[derive(Debug, Clone)]
    #[napi(object_from_js = false)]
    pub enum SheetCell {
      $($variant {
        r#ref: String,
        row: u32,
        col: u32,
        text: String,
        raw: String,
        format: Option<String>,
        format_supported: bool,
        formula: Option<String>,
        shared_formula: Option<String>,
        cached: bool,
        merge: Option<String>,
        hidden: bool,
        r#box: CellBox,
        $($field: $($ty)+,)*
      },)+
    }

    impl SheetCell {
      $(pub(crate) fn $ctor(c: CellCommon $(, $field: $($ty)+)*) -> SheetCell {
        SheetCell::$variant {
          r#ref: c.reference,
          row: c.row,
          col: c.col,
          text: c.text,
          raw: c.raw,
          format: c.format,
          format_supported: c.format_supported,
          formula: c.formula,
          shared_formula: c.shared_formula,
          cached: c.cached,
          merge: c.merge,
          hidden: c.hidden,
          r#box: c.r#box,
          $($field,)*
        }
      })+
    }
  };
}

sheet_cell! {
  Text, text { value: [String] },
  Number, number { value: [f64], subtype: [NumberSubtype], currency_symbol: [Option<String>] },
  Boolean, boolean { value: [bool] },
  Error, error { value: [String] },
  Date, date { value: [String] },
  Time, time { value: [String] },
  Blank, blank {}
}

/// One selected worksheet's cells, with the sheet facts a reader needs beside them.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetCells {
  pub sheet: String,
  /// Position in the workbook, 0-based.
  pub index: u32,
  pub hidden: bool,
  /// The declared `<dimension>`, which a producer may overstate.
  pub dimension: Option<String>,
  pub print_area: Option<String>,
  /// Merged ranges in A1 form.
  pub merges: Vec<String>,
  /// The sheet frame's size in points: A1 to the last non-empty cell or merged range, with hidden
  /// rows and columns counted or not per `hiddenCells`.
  pub width: f64,
  pub height: f64,
  /// Points; what a column or row absent from `columns` / `rows` measures.
  pub default_column_width: f64,
  pub default_row_height: f64,
  /// Only the columns and rows that differ from the default or are hidden.
  pub columns: Vec<SheetColumn>,
  pub rows: Vec<SheetRow>,
  pub warnings: Vec<String>,
  pub cells: Vec<SheetCell>,
}

/// A column's letters, ordered the way columns are: `Z` before `AA`. Plain text order would put
/// `AA` before `B`, and JavaScript keeps an object's letter keys in the order they were added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnKey(pub String);

impl AsRef<str> for ColumnKey {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl Ord for ColumnKey {
  fn cmp(&self, other: &Self) -> Ordering {
    self
      .0
      .len()
      .cmp(&other.0.len())
      .then_with(|| self.0.cmp(&other.0))
  }
}

impl PartialOrd for ColumnKey {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

/// One selected worksheet's cells keyed for lookup: `rows["12"]["C"]` is cell C12, and
/// `rows["12"]` is the whole of row 12. Only non-empty cells are present, so a row or a column
/// with nothing in it has no key.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetGrid {
  pub sheet: String,
  /// Position in the workbook, 0-based.
  pub index: u32,
  pub hidden: bool,
  /// The declared `<dimension>`, which a producer may overstate.
  pub dimension: Option<String>,
  /// Merged ranges in A1 form.
  pub merges: Vec<String>,
  pub warnings: Vec<String>,
  /// Keyed by row number as a reference writes it (`"12"`), then by column letters (`"C"`), each
  /// in sheet order.
  #[napi(ts_type = "Record<string, Record<string, SheetCell>>")]
  pub rows: BTreeMap<String, BTreeMap<ColumnKey, SheetCell>>,
}
