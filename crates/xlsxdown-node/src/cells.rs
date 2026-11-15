//! `xlsxdown`'s workbook model to the cells JavaScript sees.

use std::collections::BTreeMap;

use xlsxdown::{
  Cell, CellRange, CellValue, Sheet, SheetGeometry, Workbook, cell_ref, column_letters,
  sheet_geometry,
};

use crate::types::{
  CellBox, CellCommon, ColumnKey, NumberSubtype, SheetCell, SheetCells, SheetGrid,
  SpreadsheetOptions,
};

/// The largest whole number a JavaScript number holds exactly: 2^53.
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// The cells of the sheets `opts` selects, each sheet with its geometry.
pub(crate) fn cells_of(workbook: &Workbook, opts: &SpreadsheetOptions) -> Vec<SheetCells> {
  let hidden_cells = opts.hidden_cells.unwrap_or(true);
  workbook
    .selected_sheets(opts.sheets.as_deref(), opts.hidden_sheets.unwrap_or(false))
    .into_iter()
    .map(|sheet| {
      let geometry = sheet_geometry(sheet, &workbook.styles, hidden_cells);
      let cells = sheet_cells(sheet, &geometry, hidden_cells)
        .map(|(_, _, cell)| cell)
        .collect();
      SheetCells {
        sheet: sheet.name.clone(),
        index: sheet.index,
        hidden: sheet.hidden,
        dimension: sheet.dimension.map(|d| d.a1()),
        print_area: sheet.print_area.map(|p| p.a1()),
        merges: sheet.merges.iter().map(CellRange::a1).collect(),
        width: geometry.width,
        height: geometry.height,
        default_column_width: geometry.default_column_width,
        default_row_height: geometry.default_row_height,
        columns: geometry.columns.into_iter().map(Into::into).collect(),
        rows: geometry.rows.into_iter().map(Into::into).collect(),
        warnings: sheet.warnings.clone(),
        cells,
      }
    })
    .collect()
}

/// The cells of the sheets `opts` selects, keyed by row number and then column letters.
pub(crate) fn grid_of(workbook: &Workbook, opts: &SpreadsheetOptions) -> Vec<SheetGrid> {
  let hidden_cells = opts.hidden_cells.unwrap_or(true);
  workbook
    .selected_sheets(opts.sheets.as_deref(), opts.hidden_sheets.unwrap_or(false))
    .into_iter()
    .map(|sheet| {
      let geometry = sheet_geometry(sheet, &workbook.styles, hidden_cells);
      let mut rows: BTreeMap<String, BTreeMap<ColumnKey, SheetCell>> = BTreeMap::new();
      for (row, col, cell) in sheet_cells(sheet, &geometry, hidden_cells) {
        rows
          .entry((u64::from(row) + 1).to_string())
          .or_default()
          .insert(ColumnKey(column_letters(col)), cell);
      }
      SheetGrid {
        sheet: sheet.name.clone(),
        index: sheet.index,
        hidden: sheet.hidden,
        dimension: sheet.dimension.map(|d| d.a1()),
        merges: sheet.merges.iter().map(CellRange::a1).collect(),
        warnings: sheet.warnings.clone(),
        rows,
      }
    })
    .collect()
}

/// A sheet's cells as JavaScript sees them, with their 0-based row and column, in sheet order.
/// Cells of hidden rows and columns are left out unless `hidden_cells` is set.
fn sheet_cells<'a>(
  sheet: &'a Sheet,
  geometry: &'a SheetGeometry,
  hidden_cells: bool,
) -> impl Iterator<Item = (u32, u32, SheetCell)> + 'a {
  sheet.cells.iter().filter_map(move |(&(row, col), cell)| {
    let hidden = sheet.row_hidden(row) || sheet.col_hidden(col);
    if hidden && !hidden_cells {
      return None;
    }
    let rectangle = geometry.boxes.get(&(row, col)).copied().unwrap_or_default();
    Some((
      row,
      col,
      public_cell(row, col, cell, hidden, rectangle.into()),
    ))
  })
}

/// Whole numbers JavaScript holds exactly are integers; everything else is a float.
fn number_subtype(n: f64) -> NumberSubtype {
  if n.fract() == 0.0 && n.abs() <= MAX_EXACT_INTEGER {
    NumberSubtype::Integer
  } else {
    NumberSubtype::Float
  }
}

fn public_cell(row: u32, col: u32, cell: &Cell, hidden: bool, rectangle: CellBox) -> SheetCell {
  let common = CellCommon {
    reference: cell_ref(row, col),
    row,
    col,
    text: cell.text.clone(),
    raw: cell.raw.clone(),
    format: cell.format.clone(),
    format_supported: cell.format_supported,
    formula: cell.formula.clone(),
    shared_formula: cell.shared_master.map(|(r, c)| cell_ref(r, c)),
    cached: cell.cached,
    merge: cell.merge.map(|m| m.a1()),
    hidden,
    r#box: rectangle,
  };
  match &cell.value {
    CellValue::Text(v) => SheetCell::text(common, v.clone()),
    CellValue::Number(n) => {
      SheetCell::number(common, *n, number_subtype(*n), cell.currency_symbol.clone())
    }
    CellValue::Time(t) => SheetCell::time(common, t.clone()),
    CellValue::Boolean(b) => SheetCell::boolean(common, *b),
    CellValue::Error(e) => SheetCell::error(common, e.clone()),
    CellValue::Date(iso) => SheetCell::date(common, iso.clone()),
    CellValue::Blank => SheetCell::blank(common),
  }
}

#[cfg(test)]
mod tests;
