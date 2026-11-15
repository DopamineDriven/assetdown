//! A sheet laid out as a page of positioned words, and invoice extraction over those pages.
//!
//! `geometry` already places every cell in points. This module breaks each cell's text into
//! words inside its rectangle and hands them to `contentdown`'s line builder, which gives a sheet
//! the same shape a PDF page has. The invoice extractor reads that shape, so a workbook invoice
//! goes through it unchanged.

use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};
use contentdown::{FontMeta, PageLayout, TextSource, Word, words_to_lines_indexed};

use crate::address::cell_ref;
use crate::geometry::{CHAR_EM, LINE_EM, PageFrame, Prepared, build, page_range};
use crate::workbook::{CellValue, FontSpec, Horizontal, Sheet, Styles, Vertical, Workbook};

/// One sheet as a logical page.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetLayout {
  /// The sheet this page projects.
  pub sheet: String,
  /// The page: its size in points, its words and the lines they form. The source is `Native`:
  /// the text is the cells' own.
  pub page: PageLayout,
  /// For each of `page.words`, in the same order, the reference of the cell it was laid out
  /// from (`"C3"`); the anchor for a merged range.
  pub cells: Vec<String>,
}

impl Workbook {
  /// The logical pages of the selected sheets, numbered from 1 in selection order. Sheets are
  /// selected as [`Workbook::selected_sheets`] selects them; `hidden_cells` includes the cells
  /// of hidden rows and columns.
  pub fn layout(
    &self,
    names: Option<&[String]>,
    hidden_sheets: bool,
    hidden_cells: bool,
  ) -> Vec<SheetLayout> {
    self
      .selected_sheets(names, hidden_sheets)
      .into_iter()
      .enumerate()
      .map(|(i, sheet)| sheet_layout(sheet, &self.styles, i as u32 + 1, hidden_cells))
      .collect()
  }

  /// Invoice fields read from the workbook's visible sheets, each laid out as a page.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    let pages: Vec<PageLayout> = self
      .layout(None, false, false)
      .into_iter()
      .map(|layout| layout.page)
      .collect();
    extract_from_pages(&pages, options)
  }
}

fn font_meta(font: &FontSpec) -> FontMeta {
  FontMeta {
    name: font.name.clone(),
    bold: font.bold,
    italic: font.italic,
    symbolic: false,
    weight_known: true,
  }
}

/// The words of `text` with their character offsets: `(start, end, word)`.
fn word_spans(text: &str) -> Vec<(usize, usize, String)> {
  let mut spans = Vec::new();
  let mut start: Option<usize> = None;
  let mut current = String::new();
  for (i, c) in text.chars().enumerate() {
    if c.is_whitespace() {
      if let Some(s) = start.take() {
        spans.push((s, i, std::mem::take(&mut current)));
      }
    } else {
      if start.is_none() {
        start = Some(i);
      }
      current.push(c);
    }
  }
  if let Some(s) = start {
    spans.push((s, text.chars().count(), current));
  }
  spans
}

/// Place the words of one prepared cell inside its rectangle. Each word pushed to `words` gets
/// `cell_index` pushed to `cells`, so the two stay parallel.
fn place_cell(
  p: &Prepared<'_>,
  frame: &PageFrame,
  cell_index: usize,
  words: &mut Vec<Word>,
  cells: &mut Vec<usize>,
) {
  let cw = CHAR_EM * p.size;
  let fh = LINE_EM * p.size;
  let (x0, y0, x1, y1) = frame.rect(p.row, p.col, p.r1, p.c1);
  let block = fh * p.fragments.len() as f64;
  let top = match p.vertical {
    Vertical::Top => y1,
    Vertical::Center => y1 - ((y1 - y0) - block) / 2.0,
    Vertical::Bottom => y0 + block,
  }
  .min(y1);
  let general_right = matches!(
    p.cell.value,
    CellValue::Number(_) | CellValue::Date(_) | CellValue::Time(_)
  );
  let general_center = matches!(p.cell.value, CellValue::Boolean(_) | CellValue::Error(_));
  for (i, fragment) in p.fragments.iter().enumerate() {
    let fy1 = top - fh * i as f64;
    let fy0 = fy1 - fh;
    let fw = fragment.chars().count() as f64 * cw;
    let fx0 = match p.horizontal {
      Horizontal::Left => x0 + p.indent,
      Horizontal::Right => x1 - p.indent - fw,
      Horizontal::Center => x0 + ((x1 - x0) - fw) / 2.0,
      Horizontal::General if general_right => x1 - fw,
      Horizontal::General if general_center => x0 + ((x1 - x0) - fw) / 2.0,
      Horizontal::General => x0,
    };
    for (start, end, text) in word_spans(fragment) {
      // Unwrapped text overflows its cell sideways; everything is clipped to the frame.
      let wx0 = (fx0 + start as f64 * cw).clamp(0.0, frame.width);
      let wx1 = (fx0 + end as f64 * cw).clamp(0.0, frame.width);
      let wy0 = fy0.clamp(0.0, frame.height);
      let wy1 = fy1.clamp(0.0, frame.height);
      if wx1 <= wx0 || wy1 <= wy0 {
        continue;
      }
      words.push(Word {
        text,
        x0: wx0,
        y0: wy0,
        x1: wx1,
        y1: wy1,
        baseline: wy0,
        font_size: Some(p.size),
        font: Some(p.font_index),
        confidence: None,
      });
      cells.push(cell_index);
    }
  }
}

/// One sheet as page number `page`.
pub fn sheet_layout(sheet: &Sheet, styles: &Styles, page: u32, hidden_cells: bool) -> SheetLayout {
  let mut warnings = sheet.warnings.clone();
  let mut fonts: Vec<FontMeta> = styles.fonts.iter().map(font_meta).collect();
  fonts.push(font_meta(&styles.normal_font));

  let Some(range) = page_range(sheet) else {
    warnings.push("the sheet has no content".to_string());
    return finish(page, sheet, (0.0, 0.0), Placed::default(), &fonts, warnings);
  };
  let built = build(sheet, styles, range, hidden_cells);
  let off_page = sheet
    .cells
    .keys()
    .filter(|&&(row, col)| !built.frame.contains(row, col))
    .count();

  let mut placed = Placed::default();
  for p in &built.prepared {
    placed.refs.push(cell_ref(p.row, p.col));
    place_cell(
      p,
      &built.frame,
      placed.refs.len() - 1,
      &mut placed.words,
      &mut placed.cells,
    );
  }

  if let Some(area) = sheet.print_area {
    let bloated = sheet
      .dimension
      .is_some_and(|d| d.r0 < area.r0 || d.c0 < area.c0 || d.r1 > area.r1 || d.c1 > area.c1);
    if off_page > 0 || bloated {
      let dimension = sheet
        .dimension
        .map(|d| d.a1())
        .unwrap_or_else(|| "unknown".to_string());
      warnings.push(format!(
        "print area {} is smaller than the sheet dimension {dimension}; {off_page} cell(s) outside it are off the page",
        area.a1()
      ));
    }
  }
  if built.omitted > 0 {
    warnings.push(format!(
      "{} cell(s) in hidden rows or columns are omitted; pass hiddenCells to include them",
      built.omitted
    ));
  }
  if built.wrapped > 0 {
    warnings.push(format!(
      "{} wrapped cell(s) use estimated widths",
      built.wrapped
    ));
  }
  if built.grown_rows > 0 {
    warnings.push(format!(
      "{} row(s) grown to fit their text (auto-fit)",
      built.grown_rows
    ));
  }
  if sheet.drawings > 0 {
    warnings.push(format!("{} drawing(s) are not projected", sheet.drawings));
  }
  finish(
    page,
    sheet,
    (built.frame.width, built.frame.height),
    placed,
    &fonts,
    warnings,
  )
}

/// The words placed on a page, the cell each came from, and the references of those cells.
#[derive(Default)]
struct Placed {
  words: Vec<Word>,
  /// For each word, an index into `refs`.
  cells: Vec<usize>,
  refs: Vec<String>,
}

fn finish(
  page: u32,
  sheet: &Sheet,
  (width, height): (f64, f64),
  placed: Placed,
  fonts: &[FontMeta],
  warnings: Vec<String>,
) -> SheetLayout {
  let (words, lines, origins) = words_to_lines_indexed(placed.words, fonts);
  let cells = origins
    .into_iter()
    .map(|origin| placed.refs[placed.cells[origin]].clone())
    .collect();
  SheetLayout {
    sheet: sheet.name.clone(),
    page: PageLayout {
      page,
      width,
      height,
      rotate: 0,
      content_rotation: 0,
      source: TextSource::Native,
      words,
      lines,
      images: Vec::new(),
      warnings,
    },
    cells,
  }
}

#[cfg(test)]
mod tests;
