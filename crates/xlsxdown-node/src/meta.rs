//! A workbook's metadata as JavaScript sees it.

use napi_derive::napi;
use xlsxdown::{CellRange, SheetInfo, Workbook};

/// One sheet's counts.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetMeta {
  pub name: String,
  /// Position in the workbook, 0-based.
  pub index: u32,
  pub hidden: bool,
  /// The declared `<dimension>`, which a producer may overstate.
  pub dimension: Option<String>,
  /// From the first to the last row and column that hold a cell, in A1 form: `A1:F20`. Absent
  /// for a sheet with no cells. A merged range counts by its first cell only.
  pub used_range: Option<String>,
  /// The rows and the columns the used range spans.
  pub row_count: u32,
  pub column_count: u32,
  /// Cells that hold a value or a formula: what `spreadsheetCells` returns for the sheet with
  /// `hiddenCells` on.
  pub cell_count: u32,
  /// Cells with a formula, the ones that follow a shared formula included.
  pub formula_count: u32,
  pub merged_range_count: u32,
  pub table_count: u32,
  /// Pictures on the sheet.
  pub image_count: u32,
  pub chart_count: u32,
  pub pivot_table_count: u32,
  /// Links the sheet lists. One link may cover a range of cells.
  pub link_count: u32,
  /// Cells with a note or a comment thread.
  pub comment_count: u32,
}

/// What a workbook says about itself, and counts of what it holds. Hidden sheets, rows and
/// columns are counted like any other: this describes the file, not a selection from it.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct XlsxMeta {
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  /// As written: one string, usually separated by commas or semicolons.
  pub keywords: Option<String>,
  pub description: Option<String>,
  pub category: Option<String>,
  pub language: Option<String>,
  pub last_modified_by: Option<String>,
  pub revision: Option<String>,
  /// When the workbook was created and last saved, as the file writes them.
  pub created: Option<String>,
  pub modified: Option<String>,
  /// The application that last saved the file, and its version.
  pub application: Option<String>,
  pub app_version: Option<String>,
  pub company: Option<String>,
  /// Every sheet, the hidden ones included.
  pub sheet_count: u32,
  pub hidden_sheet_count: u32,
  /// Cells that hold a value or a formula, over every sheet.
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
  /// Names the workbook defines, the built-in ones (a print area, a filter) left out.
  pub defined_name_count: u32,
  /// Links and comments.
  pub annotation_count: u32,
  pub link_count: u32,
  pub comment_count: u32,
  /// Whether dates count from 1904, as workbooks from old versions of Excel for Mac do.
  pub date1904: bool,
  /// The file is macro-enabled. Macros are never read or run.
  pub has_macros: bool,
  /// The file is a template.
  pub is_template: bool,
  pub sheets: Vec<SheetMeta>,
}

fn sheet_meta(sheet: SheetInfo) -> SheetMeta {
  let a1 = |range: Option<CellRange>| range.map(|range| range.a1());
  SheetMeta {
    dimension: a1(sheet.dimension),
    used_range: a1(sheet.used_range),
    name: sheet.name,
    index: sheet.index,
    hidden: sheet.hidden,
    row_count: sheet.row_count,
    column_count: sheet.column_count,
    cell_count: sheet.cell_count,
    formula_count: sheet.formula_count,
    merged_range_count: sheet.merged_range_count,
    table_count: sheet.table_count,
    image_count: sheet.image_count,
    chart_count: sheet.chart_count,
    pivot_table_count: sheet.pivot_table_count,
    link_count: sheet.link_count,
    comment_count: sheet.comment_count,
  }
}

/// The workbook's properties and counts, joined into the one object JavaScript is given.
pub fn metadata_of(workbook: &Workbook) -> XlsxMeta {
  let props = workbook.props.clone();
  let counts = workbook.metadata();
  XlsxMeta {
    title: props.title,
    author: props.creator,
    subject: props.subject,
    keywords: props.keywords,
    description: props.description,
    category: props.category,
    language: props.language,
    last_modified_by: props.last_modified_by,
    revision: props.revision,
    created: props.created,
    modified: props.modified,
    application: props.application,
    app_version: props.app_version,
    company: props.company,
    sheet_count: counts.sheet_count,
    hidden_sheet_count: counts.hidden_sheet_count,
    cell_count: counts.cell_count,
    formula_count: counts.formula_count,
    merged_range_count: counts.merged_range_count,
    word_count: counts.word_count,
    character_count: counts.character_count,
    table_count: counts.table_count,
    image_count: counts.image_count,
    chart_count: counts.chart_count,
    pivot_table_count: counts.pivot_table_count,
    defined_name_count: counts.defined_name_count,
    annotation_count: counts.annotation_count,
    link_count: counts.link_count,
    comment_count: counts.comment_count,
    date1904: workbook.date1904,
    has_macros: workbook.has_macros,
    is_template: workbook.is_template,
    sheets: counts.sheets.into_iter().map(sheet_meta).collect(),
  }
}

#[cfg(test)]
mod tests;
