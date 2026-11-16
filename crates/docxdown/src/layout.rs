//! A document laid out as pages of positioned words, and invoice extraction over those pages.
//!
//! A docx records a flow of text, not where anything lands. This module gives the flow the shape
//! a PDF page has, by estimate: a paragraph is set left to right and wrapped between the
//! section's margins, a table's cells inside the columns of its grid, and the header and footer
//! a page uses above and below. Pages split where the reader counted them (see `pages`), so a
//! page here is the same page a block names.
//!
//! What the estimate keeps is what an invoice reader works from: reading order, the cells of a
//! row on one line with their columns aligned, a label beside its value, bold and the font
//! size. What it does not have is where Word really broke a line, or a font's real widths.

use contentdown::flow::{
  CHAR_EM, Fonts, LINE_EM, SLACK, Token, Tokens, base_size, line_height, set_lines,
};
use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};
use contentdown::{PageLayout, TextSource, Word, words_to_lines};

use crate::model::{
  Align, Block, BreakKind, Document, Inline, Paragraph, PartRefs, Row, Section, Table, TextRun,
};
use crate::walk::{each_inline, row_page};

/// The space after a paragraph as a fraction of the font size.
const PARAGRAPH_GAP_EM: f64 = 0.7;
/// The size of text whose size nothing states.
const DEFAULT_SIZE: f64 = 11.0;
/// Letter, which is what a section that states no page size gets.
const DEFAULT_PAGE: (f64, f64) = (612.0, 792.0);
const DEFAULT_MARGIN: f64 = 72.0;
/// How much further in each level of a list starts.
const LIST_INDENT: f64 = 18.0;
/// The space between a cell's edge and its text.
const CELL_PAD: f64 = 5.0;
/// How far a header starts from the top edge, and a footer ends from the bottom one.
const EDGE_DISTANCE: f64 = 36.0;
const ESTIMATED: &str =
  "word positions are estimated from the flow of the text; a docx records no layout";

/// A word on a page, measured down from the top edge.
#[derive(Debug, Clone)]
struct Placed {
  text: String,
  x0: f64,
  x1: f64,
  top: f64,
  bottom: f64,
  size: f64,
  font: usize,
}

/// The page a section describes, with the band its text is set in.
#[derive(Debug, Clone, Copy)]
struct Frame {
  width: f64,
  height: f64,
  left: f64,
  right: f64,
  top: f64,
  bottom: f64,
}

impl Frame {
  fn of(section: Option<&Section>) -> Frame {
    let (width, height) = section
      .and_then(|s| Some((s.width?, s.height?)))
      .filter(|(w, h)| *w > 0.0 && *h > 0.0)
      .unwrap_or(DEFAULT_PAGE);
    let margins = section.map(|s| s.margins).unwrap_or_default();
    // A section that states no margins states all four as nothing.
    let stated = [margins.top, margins.right, margins.bottom, margins.left]
      .iter()
      .any(|m| *m > 0.0);
    let side = |margin: f64| if stated { margin } else { DEFAULT_MARGIN };
    let (mut left, mut right) = (side(margins.left), width - side(margins.right));
    // Margins that leave no room for a line are not margins to set text by.
    if right - left < DEFAULT_MARGIN {
      (left, right) = (
        EDGE_DISTANCE.min(width / 4.0),
        width - EDGE_DISTANCE.min(width / 4.0),
      );
    }
    Frame {
      width,
      height,
      left,
      right,
      top: side(margins.top),
      bottom: side(margins.bottom),
    }
  }
}

/// A table grid narrower than this in all is a placeholder, not a layout: Word writes one for a
/// table it sizes to its content.
const MIN_GRID: f64 = 72.0;

/// The width each column needs to hold the longest line of its cells, which is how a table
/// sized to its content is laid out.
fn natural_widths(table: &Table, count: usize) -> Vec<f64> {
  let narrowest = 2.0 * CELL_PAD + 3.0 * CHAR_EM * DEFAULT_SIZE;
  let mut widths = vec![narrowest; count];
  for cell in table.rows.iter().flat_map(|row| &row.cells) {
    if cell.col_span > 1 {
      continue;
    }
    let longest = cell
      .lines()
      .lines()
      .map(|line| line.chars().count())
      .max()
      .unwrap_or(0);
    if let Some(width) = widths.get_mut(cell.column as usize) {
      *width = width.max(2.0 * CELL_PAD + longest as f64 * CHAR_EM * DEFAULT_SIZE);
    }
  }
  widths
}

/// The left edge of each column of a table and the right edge of the last, between `left` and
/// `right`. Widths come from the table grid when it states a believable one for every column,
/// and from the cells' content otherwise. A table too wide for its band is scaled to fit.
fn column_edges(table: &Table, left: f64, right: f64) -> Vec<f64> {
  let count = table
    .rows
    .iter()
    .flat_map(|row| &row.cells)
    .map(|cell| cell.column.saturating_add(cell.col_span.max(1)) as usize)
    .max()
    .unwrap_or(0)
    .max(table.columns.len())
    .max(1);
  let stated = table.columns.len() == count
    && table.columns.iter().all(|width| *width > 0.0)
    && table.columns.iter().sum::<f64>() >= MIN_GRID;
  let widths = if stated {
    table.columns.clone()
  } else {
    natural_widths(table, count)
  };
  let available = (right - left).max(1.0);
  let total: f64 = widths.iter().sum();
  let scale = if total > available {
    available / total
  } else {
    1.0
  };
  let mut edges = Vec::with_capacity(count + 1);
  let mut x = left;
  edges.push(x);
  for width in widths {
    x += width * scale;
    edges.push(x);
  }
  edges
}

struct Flow<'a> {
  document: &'a Document,
  fonts: Fonts,
  /// The words of the page being set.
  words: Vec<Placed>,
}

impl Flow<'_> {
  /// A paragraph as words, spaces, tabs and line breaks.
  fn tokens(&mut self, paragraph: &Paragraph) -> Vec<Token> {
    let mut tokens = Tokens::default();
    // A list item's marker is set like the text it stands in front of.
    if let Some(item) = paragraph.list.as_ref().filter(|i| !i.marker.is_empty()) {
      let mut first = None;
      each_inline(&paragraph.inlines, &mut |inline| {
        if let (None, Inline::Text(run)) = (&first, inline) {
          first = Some(run);
        }
      });
      let plain = TextRun::default();
      let run = first.unwrap_or(&plain);
      let font = self.fonts.intern(run.font.as_deref(), run.bold, run.italic);
      tokens.word(
        &item.marker,
        run.size.unwrap_or(DEFAULT_SIZE).max(1.0),
        font,
      );
    }
    each_inline(&paragraph.inlines, &mut |inline| match inline {
      Inline::Text(run) => {
        let font = self.fonts.intern(run.font.as_deref(), run.bold, run.italic);
        tokens.text(&run.text, run.size.unwrap_or(DEFAULT_SIZE).max(1.0), font);
      }
      Inline::Tab => tokens.tab(),
      Inline::Break(BreakKind::Line) => tokens.newline(),
      // What a link holds follows it; a picture or a reference ends the word before it.
      Inline::Link(_) => {}
      Inline::Image(_)
      | Inline::NoteRef(_)
      | Inline::CommentRef(_)
      | Inline::Break(BreakKind::Page | BreakKind::Column) => tokens.end_word(),
    });
    tokens.finish()
  }

  /// Sets a paragraph between `left` and `right`, starting `top` down the page, and returns how
  /// far down it ends, the space after it included.
  fn paragraph(&mut self, paragraph: &Paragraph, left: f64, right: f64, top: f64) -> f64 {
    let level = paragraph.list.as_ref().map_or(0, |item| item.level);
    let left = (left + LIST_INDENT * f64::from(level)).min(right - 1.0);
    let tokens = self.tokens(paragraph);
    let base = base_size(&tokens).unwrap_or(DEFAULT_SIZE);

    let mut cursor = top;
    for line in set_lines(tokens, left, right, true) {
      let height = line_height(&line, base);
      let end = line.last().map_or(left, |set| set.x1);
      let shift = match paragraph.align {
        Some(Align::Center) => ((right - end) / 2.0).max(0.0),
        Some(Align::Right) => (right - end).max(0.0),
        _ => 0.0,
      };
      let bottom = cursor + height;
      for set in line {
        self.words.push(Placed {
          text: set.text,
          x0: set.x0 + shift,
          x1: set.x1 + shift,
          top: bottom - LINE_EM * set.size,
          bottom,
          size: set.size,
          font: set.font,
        });
      }
      cursor = bottom;
    }
    cursor + PARAGRAPH_GAP_EM * base
  }

  /// Sets one row of a table whose columns stand at `edges`, and returns how far down it ends.
  fn row(&mut self, edges: &[f64], row: &Row, top: f64) -> f64 {
    let last = edges.len().saturating_sub(1);
    let mut end = top + LINE_EM * DEFAULT_SIZE;
    for cell in &row.cells {
      let from = (cell.column as usize).min(last);
      let to = (cell.column.saturating_add(cell.col_span.max(1)) as usize).min(last);
      let (x0, x1) = (edges[from], edges[to.max(from)]);
      let (left, right) = if x1 - x0 > 4.0 * CELL_PAD {
        (x0 + CELL_PAD, x1 - CELL_PAD)
      } else {
        (x0, x1.max(x0 + 1.0))
      };
      let cell_end = self.blocks(&cell.blocks, left, right, top);
      // A cell that spans rows does not make the first of them as tall as itself.
      end = end.max(top + (cell_end - top) / f64::from(cell.row_span.max(1)));
    }
    end
  }

  /// Sets blocks one under the other, with no regard for pages: the content of a cell, a
  /// header or a footer.
  fn blocks(&mut self, blocks: &[Block], left: f64, right: f64, top: f64) -> f64 {
    let mut cursor = top;
    for block in blocks {
      cursor = match block {
        Block::Paragraph(paragraph) => self.paragraph(paragraph, left, right, cursor),
        Block::Table(table) => {
          let edges = column_edges(table, left, right);
          let mut end = cursor;
          for row in &table.rows {
            end = self.row(&edges, row, end);
          }
          end + PARAGRAPH_GAP_EM * DEFAULT_SIZE
        }
      };
    }
    cursor
  }

  /// The header or footer a page uses: the first-page one on a title page, which has none when
  /// the section names none, and the default one on every other page.
  fn part_of(refs: PartRefs, title_page: bool) -> Option<usize> {
    let part = if title_page { refs.first } else { refs.default };
    part.map(|at| at as usize)
  }

  /// Starts a page: sets its header and returns how far down the body starts.
  fn open(&mut self, frame: &Frame, section: Option<&Section>, first_of_section: bool) -> f64 {
    let header = section
      .and_then(|s| Self::part_of(s.headers, s.title_page && first_of_section))
      .and_then(|at| self.document.headers.get(at));
    let Some(header) = header else {
      return frame.top;
    };
    let top = EDGE_DISTANCE.min(frame.top / 2.0);
    let end = self.blocks(&header.blocks, frame.left, frame.right, top);
    frame.top.max(end)
  }

  /// Ends a page whose body stops `body_end` down it: sets its footer, and turns what was set
  /// into a page. The page is as tall as its section says, or as tall as its content needs.
  fn close(
    &mut self,
    number: u32,
    frame: &Frame,
    section: Option<&Section>,
    first_of_section: bool,
    body_end: f64,
  ) -> PageLayout {
    let footer = section
      .and_then(|s| Self::part_of(s.footers, s.title_page && first_of_section))
      .and_then(|at| self.document.footers.get(at));
    let mut content_end = body_end;
    if let Some(footer) = footer {
      // Set from the top to learn how tall it is, then moved to where it belongs: above the
      // bottom edge, or under the body when the body runs that far.
      let body = std::mem::take(&mut self.words);
      let height = self.blocks(&footer.blocks, frame.left, frame.right, 0.0);
      let top = (frame.height - EDGE_DISTANCE.min(frame.bottom / 2.0) - height).max(body_end);
      let mut footer_words = std::mem::replace(&mut self.words, body);
      for word in &mut footer_words {
        word.top += top;
        word.bottom += top;
      }
      if !footer_words.is_empty() {
        content_end = top + height;
      }
      self.words.append(&mut footer_words);
    }
    let needed = content_end + EDGE_DISTANCE.min(frame.bottom / 2.0);
    let height = if needed > frame.height + SLACK {
      needed
    } else {
      frame.height
    };
    let mut warnings = vec![ESTIMATED.to_owned()];
    if height > frame.height {
      warnings.push(format!(
        "the content is estimated to run past the paper ({:.0} points); the page is {:.0} points tall to hold it",
        frame.height, height
      ));
    }
    let words: Vec<Word> = std::mem::take(&mut self.words)
      .into_iter()
      .filter_map(|placed| {
        let x0 = placed.x0.clamp(0.0, frame.width);
        let x1 = placed.x1.clamp(0.0, frame.width);
        let (y0, y1) = (height - placed.bottom, height - placed.top);
        (x1 > x0 && y1 > y0).then_some(Word {
          text: placed.text,
          x0,
          y0,
          x1,
          y1,
          baseline: y0,
          font_size: Some(placed.size),
          font: Some(placed.font),
          confidence: None,
        })
      })
      .collect();
    let (mut words, lines) = words_to_lines(words, self.fonts.metas());
    for word in &mut words {
      word.font_name = word.font_name.take().filter(|name| !name.is_empty());
    }
    PageLayout {
      page: number,
      width: frame.width,
      height,
      rotate: 0,
      content_rotation: 0,
      source: TextSource::Native,
      words,
      lines,
      images: Vec::new(),
      warnings,
    }
  }
}

/// The page being set and where its body has got to.
struct Open {
  number: u32,
  section: u32,
  frame: Frame,
  first_of_section: bool,
  cursor: f64,
}

impl Document {
  /// The document as pages of positioned words: one page for each the reader counted, each
  /// with the header and footer its section gives it. Positions are estimates; see the module.
  pub fn layout(&self) -> Vec<PageLayout> {
    let mut flow = Flow {
      document: self,
      fonts: Fonts::default(),
      words: Vec::new(),
    };
    let mut pages = Vec::new();
    // Whether a page of each section has been started, which tells a section's first page.
    let mut started = vec![false; self.sections.len()];
    let mut start = |flow: &mut Flow<'_>, number: u32, section: u32| {
      let known = self.sections.get(section as usize);
      let first_of_section = started
        .get_mut(section as usize)
        .is_some_and(|started| !std::mem::replace(started, true));
      let frame = Frame::of(known);
      let cursor = flow.open(&frame, known, first_of_section);
      Open {
        number,
        section,
        frame,
        first_of_section,
        cursor,
      }
    };
    let finish = |flow: &mut Flow<'_>, open: &Open| {
      flow.close(
        open.number,
        &open.frame,
        self.sections.get(open.section as usize),
        open.first_of_section,
        open.cursor,
      )
    };
    let first_section = self.blocks.first().map_or(0, Block::section);
    let mut open = start(&mut flow, 1, first_section);
    // Moves on to `page`, which starts in `section`, setting every page on the way.
    let mut turn_to = |flow: &mut Flow<'_>,
                       open: &mut Open,
                       pages: &mut Vec<PageLayout>,
                       page: u32,
                       section: u32| {
      while open.number < page {
        pages.push(finish(flow, open));
        // A page with no block of its own belongs to the section the last one did.
        let next = open.number + 1;
        let section = if next == page { section } else { open.section };
        *open = start(flow, next, section);
      }
    };
    for block in &self.blocks {
      match block {
        Block::Paragraph(paragraph) => {
          turn_to(
            &mut flow,
            &mut open,
            &mut pages,
            paragraph.page,
            paragraph.section,
          );
          open.cursor = flow.paragraph(paragraph, open.frame.left, open.frame.right, open.cursor);
        }
        Block::Table(table) => {
          for row in &table.rows {
            let page = row_page(row).unwrap_or(table.page);
            turn_to(&mut flow, &mut open, &mut pages, page, table.section);
            let edges = column_edges(table, open.frame.left, open.frame.right);
            open.cursor = flow.row(&edges, row, open.cursor);
          }
          open.cursor += PARAGRAPH_GAP_EM * DEFAULT_SIZE;
        }
      }
    }
    // A break at the very end leaves pages with nothing on them; they are still pages.
    let last_section = open.section;
    turn_to(
      &mut flow,
      &mut open,
      &mut pages,
      self.page_count.max(1),
      last_section,
    );
    pages.push(finish(&mut flow, &open));
    pages
  }

  /// Invoice fields read from the document's pages as [`Document::layout`] sets them.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    extract_from_pages(&self.layout(), options)
  }
}

#[cfg(test)]
mod tests;
