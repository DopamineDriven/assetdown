//! Counts of what a workbook holds. What it says about itself (title, author, dates) is in
//! [`Workbook::props`].

use crate::address::CellRange;
use crate::workbook::{CellValue, Sheet, Workbook};

/// One sheet's counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetInfo {
  pub name: String,
  /// Position in the workbook, 0-based.
  pub index: u32,
  pub hidden: bool,
  /// The declared `<dimension>`, which a producer may overstate.
  pub dimension: Option<CellRange>,
  /// From the first to the last row and column that hold a cell; `None` for a sheet with no
  /// cells. A merged range counts by its first cell only, as that is where its content is.
  pub used_range: Option<CellRange>,
  /// The rows and the columns the used range spans.
  pub row_count: u32,
  pub column_count: u32,
  /// Cells that hold a value or a formula: the length of [`Sheet::cells`].
  pub cell_count: u32,
  /// Cells with a formula, the ones that follow a shared formula included.
  pub formula_count: u32,
  pub merged_range_count: u32,
  pub table_count: u32,
  /// Pictures on the sheet's drawing.
  pub image_count: u32,
  pub chart_count: u32,
  pub pivot_table_count: u32,
  /// Links the sheet lists. One link may cover a range of cells.
  pub link_count: u32,
  /// Cells with a note or a comment thread.
  pub comment_count: u32,
}

/// A workbook's counts: each sheet's, and their sums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
  /// Every sheet, the hidden ones included.
  pub sheet_count: u32,
  pub hidden_sheet_count: u32,
  pub cell_count: u32,
  pub formula_count: u32,
  pub merged_range_count: u32,
  /// Words of the cells that hold text, counted from what those cells show.
  pub word_count: u32,
  /// Characters of those cells that are not white space.
  pub character_count: u32,
  pub table_count: u32,
  pub image_count: u32,
  pub chart_count: u32,
  pub pivot_table_count: u32,
  /// Names the workbook defines, the built-in ones left out.
  pub defined_name_count: u32,
  /// Links and comments.
  pub annotation_count: u32,
  pub link_count: u32,
  pub comment_count: u32,
  pub sheets: Vec<SheetInfo>,
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

/// What one pass over a sheet's cells gives.
struct CellCounts {
  used_range: Option<CellRange>,
  formulas: usize,
  words: usize,
  characters: usize,
}

fn count_cells(sheet: &Sheet) -> CellCounts {
  let mut counts = CellCounts {
    used_range: None,
    formulas: 0,
    words: 0,
    characters: 0,
  };
  for (&(row, col), cell) in &sheet.cells {
    counts.used_range = Some(match counts.used_range {
      Some(range) => CellRange {
        r0: range.r0.min(row),
        c0: range.c0.min(col),
        r1: range.r1.max(row),
        c1: range.c1.max(col),
      },
      None => CellRange {
        r0: row,
        c0: col,
        r1: row,
        c1: col,
      },
    });
    // A follower of a shared formula has one whether or not its text was written out.
    if cell.formula.is_some() || cell.shared_master.is_some() {
      counts.formulas += 1;
    }
    if matches!(cell.value, CellValue::Text(_)) {
      counts.words += cell.text.split_whitespace().count();
      counts.characters += cell.text.chars().filter(|c| !c.is_whitespace()).count();
    }
  }
  counts
}

impl Workbook {
  /// Counts of what the workbook holds, sheet by sheet and in total. Hidden sheets, rows and
  /// columns are counted like any other: this describes the file, not a selection from it.
  pub fn metadata(&self) -> Metadata {
    let (mut words, mut characters) = (0, 0);
    let sheets: Vec<SheetInfo> = self
      .sheets
      .iter()
      .map(|sheet| {
        let cells = count_cells(sheet);
        words += cells.words;
        characters += cells.characters;
        let (row_count, column_count) = cells.used_range.map_or((0, 0), |range| {
          (
            (range.r1 - range.r0).saturating_add(1),
            (range.c1 - range.c0).saturating_add(1),
          )
        });
        SheetInfo {
          name: sheet.name.clone(),
          index: sheet.index,
          hidden: sheet.hidden,
          dimension: sheet.dimension,
          used_range: cells.used_range,
          row_count,
          column_count,
          cell_count: count(sheet.cells.len()),
          formula_count: count(cells.formulas),
          merged_range_count: count(sheet.merges.len()),
          table_count: count(sheet.tables),
          image_count: count(sheet.pictures),
          chart_count: count(sheet.charts),
          pivot_table_count: count(sheet.pivot_tables),
          link_count: count(sheet.links),
          comment_count: count(sheet.comments),
        }
      })
      .collect();
    let sum = |of: fn(&SheetInfo) -> u32| {
      sheets
        .iter()
        .fold(0_u32, |total, sheet| total.saturating_add(of(sheet)))
    };
    let (link_count, comment_count) = (sum(|s| s.link_count), sum(|s| s.comment_count));
    Metadata {
      sheet_count: count(sheets.len()),
      hidden_sheet_count: count(sheets.iter().filter(|sheet| sheet.hidden).count()),
      cell_count: sum(|s| s.cell_count),
      formula_count: sum(|s| s.formula_count),
      merged_range_count: sum(|s| s.merged_range_count),
      word_count: count(words),
      character_count: count(characters),
      table_count: sum(|s| s.table_count),
      image_count: sum(|s| s.image_count),
      chart_count: sum(|s| s.chart_count),
      pivot_table_count: sum(|s| s.pivot_table_count),
      defined_name_count: count(self.defined_names),
      annotation_count: link_count.saturating_add(comment_count),
      link_count,
      comment_count,
      sheets,
    }
  }
}

#[cfg(test)]
mod tests;
