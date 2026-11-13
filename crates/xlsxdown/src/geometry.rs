//! Sheet geometry: the frame a sheet's cells sit in, in points.
//!
//! The frame runs from A1 to the last non-empty cell or merged range. Columns and rows get their
//! points from the stored widths and heights (hidden ones contribute nothing unless the caller
//! asks for hidden cells); a row grows to fit text taller than it, as Excel's auto-fit does on
//! open. Coordinates are y-up from the frame's bottom-left, like a page.
//!
//! [`build`] is the groundwork both readers share: [`sheet_geometry`] reads the frame and the
//! cell rectangles off it, and a projection of the sheet onto a page places each prepared cell's
//! text inside it.

use std::collections::{BTreeSet, HashMap};

use crate::address::CellRange;
use crate::workbook::{Cell, Horizontal, Sheet, Styles, Vertical};

/// A rectangle in the sheet frame: points, y up, with A1's top-left corner at `(0, height)`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CellBox {
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

/// A column whose width differs from the sheet default, or that is hidden. 0-based.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetColumn {
  pub index: u32,
  /// Points; zero for a hidden column when hidden cells are excluded.
  pub width: f64,
  pub hidden: bool,
}

/// A row whose height differs from the sheet default (auto-fit growth included), or that is
/// hidden. 0-based.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetRow {
  pub index: u32,
  /// Points; zero for a hidden row when hidden cells are excluded.
  pub height: f64,
  pub hidden: bool,
}

/// Maximum digit width of the Normal style's font (Calibri 11) in pixels at 96 dpi.
pub const MDW_PX: f64 = 7.0;
/// Row height when the sheet states none.
pub const DEFAULT_ROW_HEIGHT_PT: f64 = 15.0;
/// What Excel stores for a default column: `baseColWidth` 8 plus padding, 64 px.
pub const DEFAULT_COL_WIDTH: f64 = 9.140625;
/// Estimated advance of one character as a fraction of the font size.
pub const CHAR_EM: f64 = 0.5;
/// Height of one text fragment as a fraction of the font size.
pub const LINE_EM: f64 = 1.2;
const PT_PER_PX: f64 = 72.0 / 96.0;

/// A stored `<col width>` (characters of the maximum digit width) to points: the SpreadsheetML
/// pixel formula, then 72/96.
pub fn column_width_points(width: f64) -> f64 {
  let px = (((256.0 * width + (128.0 / MDW_PX).trunc()) / 256.0) * MDW_PX).trunc();
  px * PT_PER_PX
}

/// The sheet frame's extents, the columns and rows that differ from the defaults, and every
/// cell's rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetGeometry {
  pub width: f64,
  pub height: f64,
  pub default_column_width: f64,
  pub default_row_height: f64,
  pub columns: Vec<SheetColumn>,
  pub rows: Vec<SheetRow>,
  pub boxes: HashMap<(u32, u32), CellBox>,
}

/// The sheet frame: A1 to the last non-empty cell or merged range, in the same y-up points as
/// the pages, with rows grown to fit their text. Hidden rows and columns keep their extents when
/// `hidden_cells` is set and have none otherwise.
pub fn sheet_geometry(sheet: &Sheet, styles: &Styles, hidden_cells: bool) -> SheetGeometry {
  let default_column_width = column_width_points(default_col_width(sheet));
  let default_row_height = default_row_height(sheet);
  let Some(range) = sheet_range(sheet) else {
    return SheetGeometry {
      width: 0.0,
      height: 0.0,
      default_column_width,
      default_row_height,
      columns: Vec::new(),
      rows: Vec::new(),
      boxes: HashMap::new(),
    };
  };
  let built = build(sheet, styles, range, hidden_cells);
  let columns = built
    .widths
    .iter()
    .enumerate()
    .filter_map(|(i, &width)| {
      let index = range.c0 + i as u32;
      let hidden = sheet.col_hidden(index);
      ((width - default_column_width).abs() > 1e-9 || hidden).then_some(SheetColumn {
        index,
        width,
        hidden,
      })
    })
    .collect();
  let rows = built
    .heights
    .iter()
    .enumerate()
    .filter_map(|(i, &height)| {
      let index = range.r0 + i as u32;
      let hidden = sheet.row_hidden(index);
      ((height - default_row_height).abs() > 1e-9 || hidden).then_some(SheetRow {
        index,
        height,
        hidden,
      })
    })
    .collect();
  let boxes = sheet
    .cells
    .iter()
    .map(|(&(row, col), cell)| {
      let (r1, c1) = built.frame.span_end(row, col, cell.merge);
      let (x0, y0, x1, y1) = built.frame.rect(row, col, r1, c1);
      ((row, col), CellBox { x0, y0, x1, y1 })
    })
    .collect();
  SheetGeometry {
    width: built.frame.width,
    height: built.frame.height,
    default_column_width,
    default_row_height,
    columns,
    rows,
    boxes,
  }
}

/// Cumulative offsets of consecutive extents: `offsets[i]` is where item `i` starts and the last
/// entry the total.
fn offsets(extents: &[f64]) -> Vec<f64> {
  let mut out = Vec::with_capacity(extents.len() + 1);
  let mut total = 0.0;
  out.push(total);
  for extent in extents {
    total += extent;
    out.push(total);
  }
  out
}

fn default_col_width(sheet: &Sheet) -> f64 {
  sheet
    .default_col_width
    .filter(|w| *w > 0.0)
    .unwrap_or(DEFAULT_COL_WIDTH)
}

fn default_row_height(sheet: &Sheet) -> f64 {
  if sheet.default_row_height > 0.0 {
    sheet.default_row_height
  } else {
    DEFAULT_ROW_HEIGHT_PT
  }
}

fn column_points(sheet: &Sheet, col: u32, hidden_cells: bool) -> f64 {
  if sheet.col_hidden(col) && !hidden_cells {
    return 0.0;
  }
  let stored = sheet
    .cols
    .iter()
    .find(|c| col + 1 >= c.min && col < c.max && c.width > 0.0)
    .map(|c| c.width)
    .unwrap_or_else(|| default_col_width(sheet));
  column_width_points(stored)
}

fn row_points(sheet: &Sheet, row: u32, hidden_cells: bool) -> f64 {
  if sheet.row_hidden(row) && !hidden_cells {
    return 0.0;
  }
  sheet
    .rows
    .get(&row)
    .and_then(|r| r.height)
    .filter(|h| *h > 0.0)
    .unwrap_or_else(|| default_row_height(sheet))
}

fn extend(range: &mut Option<CellRange>, r0: u32, c0: u32, r1: u32, c1: u32) {
  *range = Some(match *range {
    None => CellRange { r0, c0, r1, c1 },
    Some(r) => CellRange {
      r0: r.r0.min(r0),
      c0: r.c0.min(c0),
      r1: r.r1.max(r1),
      c1: r.c1.max(c1),
    },
  });
}

/// The bounding box of the non-empty cells and the ranges they anchor.
fn content_range(sheet: &Sheet) -> Option<CellRange> {
  let mut range = None;
  for (&(row, col), cell) in &sheet.cells {
    extend(&mut range, row, col, row, col);
    if let Some(m) = cell.merge {
      extend(&mut range, m.r0, m.c0, m.r1, m.c1);
    }
  }
  range
}

/// The sheet frame: A1 to the last non-empty cell or merged range.
pub fn sheet_range(sheet: &Sheet) -> Option<CellRange> {
  content_range(sheet).map(|r| CellRange {
    r0: 0,
    c0: 0,
    r1: r.r1,
    c1: r.c1,
  })
}

/// The print area, else the content range.
pub fn page_range(sheet: &Sheet) -> Option<CellRange> {
  sheet.print_area.or_else(|| content_range(sheet))
}

/// Greedy word wrap at `max_chars` characters; a single word longer than the width stands alone.
fn wrap(text: &str, max_chars: usize) -> Vec<String> {
  let mut lines: Vec<String> = Vec::new();
  let mut current = String::new();
  let mut current_len = 0usize;
  for word in text.split_whitespace() {
    let len = word.chars().count();
    if current.is_empty() {
      current.push_str(word);
      current_len = len;
    } else if current_len + 1 + len <= max_chars {
      current.push(' ');
      current.push_str(word);
      current_len += 1 + len;
    } else {
      lines.push(std::mem::take(&mut current));
      current.push_str(word);
      current_len = len;
    }
  }
  if !current.is_empty() || lines.is_empty() {
    lines.push(current);
  }
  lines
}

/// A range of the sheet laid out in points: where each of its columns and rows starts.
#[derive(Debug, Clone, PartialEq)]
pub struct PageFrame {
  pub range: CellRange,
  /// Where each column of the range starts, left to right, with the total width last.
  pub col_x: Vec<f64>,
  /// Where each row of the range starts, top down, with the total height last.
  pub row_y: Vec<f64>,
  pub width: f64,
  pub height: f64,
}

impl PageFrame {
  pub fn contains(&self, row: u32, col: u32) -> bool {
    (self.range.r0..=self.range.r1).contains(&row) && (self.range.c0..=self.range.c1).contains(&col)
  }

  /// The last row and column a cell covers: its merged range clipped to the frame.
  pub fn span_end(&self, row: u32, col: u32, merge: Option<CellRange>) -> (u32, u32) {
    match merge {
      Some(m) => (
        m.r1.min(self.range.r1).max(row),
        m.c1.min(self.range.c1).max(col),
      ),
      None => (row, col),
    }
  }

  /// The rectangle from `(row, col)` through `(r1, c1)`: `(x0, y0, x1, y1)`.
  pub fn rect(&self, row: u32, col: u32, r1: u32, c1: u32) -> (f64, f64, f64, f64) {
    let x0 = self.col_x[(col - self.range.c0) as usize];
    let x1 = self.col_x[(c1 - self.range.c0 + 1) as usize];
    let y1 = self.height - self.row_y[(row - self.range.r0) as usize];
    let y0 = self.height - self.row_y[(r1 - self.range.r0 + 1) as usize];
    (x0, y0, x1, y1)
  }
}

/// A non-empty, visible cell inside the frame with its text already broken into fragments.
#[derive(Debug, Clone)]
pub struct Prepared<'a> {
  pub row: u32,
  pub col: u32,
  pub r1: u32,
  pub c1: u32,
  pub cell: &'a Cell,
  pub font_index: usize,
  pub size: f64,
  pub horizontal: Horizontal,
  pub vertical: Vertical,
  pub indent: f64,
  pub fragments: Vec<String>,
}

/// A frame with its cells prepared: widths, heights after auto-fit, and the counts the page warns about.
#[derive(Debug, Clone)]
pub struct Built<'a> {
  pub frame: PageFrame,
  /// The width of each column of the range, in points.
  pub widths: Vec<f64>,
  /// The height of each row of the range after auto-fit, in points.
  pub heights: Vec<f64>,
  pub prepared: Vec<Prepared<'a>>,
  /// Cells left out because their row or column is hidden.
  pub omitted: usize,
  /// Cells whose text was wrapped to an estimated width.
  pub wrapped: usize,
  /// Rows that grew to fit their text.
  pub grown_rows: usize,
}

/// Lay `range` of the sheet out in points and prepare the text of every visible, non-empty cell
/// in it.
pub fn build<'a>(
  sheet: &'a Sheet,
  styles: &'a Styles,
  range: CellRange,
  hidden_cells: bool,
) -> Built<'a> {
  let widths: Vec<f64> = (range.c0..=range.c1)
    .map(|col| column_points(sheet, col, hidden_cells))
    .collect();
  let col_x = offsets(&widths);
  let normal_index = styles.fonts.len();
  let mut prepared: Vec<Prepared<'a>> = Vec::new();
  let mut omitted = 0usize;
  let mut wrapped = 0usize;
  for (&(row, col), cell) in &sheet.cells {
    if !(range.r0..=range.r1).contains(&row) || !(range.c0..=range.c1).contains(&col) {
      continue;
    }
    if (sheet.row_hidden(row) || sheet.col_hidden(col)) && !hidden_cells {
      omitted += 1;
      continue;
    }
    if cell.text.trim().is_empty() {
      continue;
    }
    let xf = styles.xfs.get(cell.style);
    let (font, font_index) = xf
      .and_then(|xf| styles.fonts.get(xf.font_id).map(|f| (f, xf.font_id)))
      .unwrap_or((&styles.normal_font, normal_index));
    let size = if font.size > 0.0 { font.size } else { 11.0 };
    let indent = xf.map(|x| x.indent as f64 * size).unwrap_or(0.0);
    let (r1, c1) = match cell.merge {
      Some(m) => (m.r1.min(range.r1).max(row), m.c1.min(range.c1).max(col)),
      None => (row, col),
    };
    let x0 = col_x[(col - range.c0) as usize];
    let x1 = col_x[(c1 - range.c0 + 1) as usize];
    let cw = CHAR_EM * size;
    let avail = (x1 - x0 - indent).max(cw);
    let wrap_on = xf.is_some_and(|x| x.wrap);
    let mut fragments: Vec<String> = Vec::new();
    let mut did_wrap = false;
    for part in cell.text.split('\n') {
      let part = part.trim_end_matches('\r');
      if wrap_on {
        let lines = wrap(part, ((avail / cw).floor() as usize).max(1));
        did_wrap |= lines.len() > 1;
        fragments.extend(lines);
      } else {
        fragments.push(part.to_string());
      }
    }
    wrapped += usize::from(did_wrap);
    prepared.push(Prepared {
      row,
      col,
      r1,
      c1,
      cell,
      font_index,
      size,
      horizontal: xf.map(|x| x.horizontal).unwrap_or_default(),
      vertical: xf.map(|x| x.vertical).unwrap_or_default(),
      indent,
      fragments,
    });
  }

  // Auto-fit: a row grows to hold the tallest text block anchored in it, the way Excel sizes a
  // row it opens. A block over a merged range grows the tallest of its rows.
  let mut heights: Vec<f64> = (range.r0..=range.r1)
    .map(|row| row_points(sheet, row, hidden_cells))
    .collect();
  let mut grown: BTreeSet<usize> = BTreeSet::new();
  for p in &prepared {
    let block = LINE_EM * p.size * p.fragments.len() as f64;
    let first = (p.row - range.r0) as usize;
    let last = (p.r1 - range.r0) as usize;
    let have: f64 = heights[first..=last].iter().sum();
    if block > have + 1e-9 {
      let target = (first..=last)
        .rev()
        .max_by(|&a, &b| heights[a].total_cmp(&heights[b]))
        .unwrap_or(last);
      heights[target] += block - have;
      grown.insert(target);
    }
  }
  let row_y = offsets(&heights);
  let frame = PageFrame {
    range,
    width: *col_x.last().unwrap_or(&0.0),
    height: *row_y.last().unwrap_or(&0.0),
    col_x,
    row_y,
  };
  Built {
    frame,
    widths,
    heights,
    prepared,
    omitted,
    wrapped,
    grown_rows: grown.len(),
  }
}

#[cfg(test)]
mod tests;
