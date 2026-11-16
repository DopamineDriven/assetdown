//! Tables as grids: every cell at its row and column, the way a sheet's cells are addressed.

pub use ooxmldown::{cell_ref, column_letters};

use crate::model::{Block, Document, Table};

/// One cell of a table, at the top-left corner of the range it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCell {
  /// From 0.
  pub row: u32,
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
  /// The cell's text, one line per paragraph.
  pub text: String,
}

/// A table with its cells addressed.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGrid {
  /// The table's place among the document's tables, in reading order, from 0. A table inside a
  /// cell follows the table that holds it.
  pub index: u32,
  /// The table whose cell holds this one, when it sits inside another.
  pub parent: Option<u32>,
  /// The page the table starts on, from 1.
  pub page: u32,
  /// Column widths in points, from the table grid.
  pub columns: Vec<f64>,
  pub row_count: u32,
  pub column_count: u32,
  /// How many rows at the top repeat on each page: the header rows.
  pub header_rows: u32,
  /// The ranges merged cells cover, in A1 form: `A1:B2`.
  pub merges: Vec<String>,
  /// Row by row, left to right. A cell merged into another is not listed.
  pub cells: Vec<GridCell>,
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

fn grid(table: &Table, index: u32, parent: Option<u32>) -> TableGrid {
  let mut cells = Vec::new();
  let mut merges = Vec::new();
  let mut column_count = count(table.columns.len());
  for (row, cells_of_row) in table.rows.iter().enumerate() {
    let row = count(row);
    for cell in &cells_of_row.cells {
      let (last_row, last_column) = (
        row.saturating_add(cell.row_span.max(1) - 1),
        cell.column.saturating_add(cell.col_span.max(1) - 1),
      );
      column_count = column_count.max(last_column.saturating_add(1));
      if (last_row, last_column) != (row, cell.column) {
        merges.push(format!(
          "{}:{}",
          cell_ref(row, cell.column),
          cell_ref(last_row, last_column)
        ));
      }
      cells.push(GridCell {
        row,
        column: cell.column,
        col_span: cell.col_span,
        row_span: cell.row_span,
        text: cell.lines(),
      });
    }
  }
  TableGrid {
    index,
    parent,
    page: table.page,
    columns: table.columns.clone(),
    row_count: count(table.rows.len()),
    column_count,
    header_rows: count(table.rows.iter().take_while(|row| row.header).count()),
    merges,
    cells,
  }
}

fn collect(blocks: &[Block], parent: Option<u32>, out: &mut Vec<TableGrid>) {
  for block in blocks {
    if let Block::Table(table) = block {
      let index = count(out.len());
      out.push(grid(table, index, parent));
      for cell in table.rows.iter().flat_map(|row| &row.cells) {
        collect(&cell.blocks, Some(index), out);
      }
    }
  }
}

impl Document {
  /// The tables of the body, the ones inside cells included.
  pub fn tables(&self) -> Vec<TableGrid> {
    let mut out = Vec::new();
    collect(&self.blocks, None, &mut out);
    out
  }
}

#[cfg(test)]
mod tests;
