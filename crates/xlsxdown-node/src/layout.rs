//! A sheet's logical page as JavaScript sees it.

use napi_derive::napi;
use xlsxdown::{CellRange, Workbook, sheet_layout};

use crate::types::SpreadsheetOptions;

/// One word of a sheet's logical page, with its box in the page frame: points, y up, the page's
/// bottom-left corner at the origin.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `SheetPage.lines`.
  pub line: u32,
  /// The cell's font: its size in points, its name, and whether it is bold or italic.
  pub font_size: Option<f64>,
  pub font_name: Option<String>,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  /// The reference of the cell this word was laid out from (`"C3"`); the anchor for a merged
  /// range.
  pub cell: String,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount` address a contiguous
/// slice of `SheetPage.words`.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// One selected sheet laid out as a page: every cell's text placed inside the cell's rectangle,
/// broken into words and grouped into lines, with the sheet facts a reader needs beside them.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetPage {
  /// 1-based, in selection order.
  pub page: u32,
  pub sheet: String,
  /// Position in the workbook, 0-based.
  pub index: u32,
  pub hidden: bool,
  /// The declared `<dimension>`, which a producer may overstate.
  pub dimension: Option<String>,
  pub print_area: Option<String>,
  /// Merged ranges in A1 form.
  pub merges: Vec<String>,
  /// The page's size in points: the print area when the sheet has one, else A1 to the last
  /// non-empty cell, with rows grown to fit their text.
  pub width: f64,
  pub height: f64,
  pub words: Vec<SheetWord>,
  pub lines: Vec<SheetLine>,
  pub warnings: Vec<String>,
}

/// The logical pages of the sheets `opts` selects. Unlike the cell readers, cells in hidden rows
/// and columns are left off the page unless `hiddenCells` is set.
pub(crate) fn pages_of(workbook: &Workbook, opts: &SpreadsheetOptions) -> Vec<SheetPage> {
  let hidden_cells = opts.hidden_cells.unwrap_or(false);
  workbook
    .selected_sheets(opts.sheets.as_deref(), opts.hidden_sheets.unwrap_or(false))
    .into_iter()
    .zip(1u32..)
    .map(|(sheet, number)| {
      let layout = sheet_layout(sheet, &workbook.styles, number, hidden_cells);
      let words = layout
        .page
        .words
        .into_iter()
        .zip(layout.cells)
        .map(|(word, cell)| SheetWord {
          text: word.text,
          x0: word.x0,
          y0: word.y0,
          x1: word.x1,
          y1: word.y1,
          line: word.line,
          font_size: word.font_size,
          font_name: word.font_name,
          bold: word.bold,
          italic: word.italic,
          cell,
        })
        .collect();
      let lines = layout
        .page
        .lines
        .into_iter()
        .map(|line| SheetLine {
          text: line.text,
          x0: line.x0,
          y0: line.y0,
          x1: line.x1,
          y1: line.y1,
          first_word: line.first_word,
          word_count: line.word_count,
        })
        .collect();
      SheetPage {
        page: layout.page.page,
        sheet: layout.sheet,
        index: sheet.index,
        hidden: sheet.hidden,
        dimension: sheet.dimension.map(|d| d.a1()),
        print_area: sheet.print_area.map(|p| p.a1()),
        merges: sheet.merges.iter().map(CellRange::a1).collect(),
        width: layout.page.width,
        height: layout.page.height,
        words,
        lines,
        warnings: layout.page.warnings,
      }
    })
    .collect()
}
