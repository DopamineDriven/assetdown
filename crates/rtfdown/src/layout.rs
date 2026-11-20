//! The document set on pages, by estimate. An RTF file records a flow of text and not where
//! anything lands, so the positions are worked out here as they are for a Word file: a
//! paragraph is set from the left and wrapped between the margins the file states, the cells
//! of a row between the edges the file gives them, and a page is turned when the text reaches
//! the foot of it, with the file's header at the head of each and its footer at the foot. A
//! paragraph the file gives a place of its own is set at that place, which
//! is the one thing here that is the file's and not an estimate. Where a program really broke
//! a line, and a font's real widths, are not known.

use contentdown::flow::{Fonts, LINE_EM, SLACK, Token, Tokens, base_size, line_height, set_lines};
use contentdown::{PageLayout, TextSource, Word, words_to_lines};

use crate::body::{Align, Block, Body, Inline, Paper, Paragraph, Row, Running};

/// The size of text in a paragraph that has none.
const DEFAULT_SIZE: f64 = 12.0;
/// The space between a cell's edge and its text.
const CELL_PAD: f64 = 5.0;
/// How far a header starts from the top edge, and a footer ends from the bottom one.
const EDGE_DISTANCE: f64 = 36.0;
const ESTIMATED: &str =
  "word positions are estimated from the flow of the text; an RTF file records no layout";

/// A word on a page, measured down from the top edge.
struct Placed {
  text: String,
  x0: f64,
  x1: f64,
  top: f64,
  bottom: f64,
  size: f64,
  font: usize,
}

/// The band of the page the text is set in, measured from its left and top edges.
struct Frame {
  width: f64,
  height: f64,
  left: f64,
  right: f64,
  top: f64,
  bottom: f64,
}

impl Frame {
  fn of(paper: Paper) -> Frame {
    // A paper or margins that leave no room for text are not believed.
    let believable = paper.width > paper.left + paper.right + 36.0
      && paper.height > paper.top + paper.bottom + 36.0
      && paper.left >= 0.0
      && paper.right >= 0.0
      && paper.top >= 0.0
      && paper.bottom >= 0.0;
    let paper = match believable {
      true => paper,
      false => Paper {
        width: 612.0,
        height: 792.0,
        left: 90.0,
        right: 90.0,
        top: 72.0,
        bottom: 72.0,
      },
    };
    Frame {
      width: paper.width,
      height: paper.height,
      left: paper.left,
      right: paper.width - paper.right,
      top: paper.top,
      bottom: paper.height - paper.bottom,
    }
  }
}

struct Flow {
  fonts: Fonts,
  /// The words of the page being set.
  words: Vec<Placed>,
}

impl Flow {
  /// A paragraph as words, spaces, tabs, and line breaks.
  fn tokens(&mut self, paragraph: &Paragraph) -> Vec<Token> {
    let mut tokens = Tokens::default();
    for inline in &paragraph.inlines {
      match inline {
        Inline::Text(run) => {
          let font = self.fonts.intern(run.font.as_deref(), run.bold, run.italic);
          tokens.text(&run.text, run.size.max(1.0), font);
        }
        Inline::Tab => tokens.tab(),
        Inline::LineBreak => tokens.newline(),
      }
    }
    tokens.finish()
  }

  /// Sets a paragraph between `left` and `right`, starting `top` down the page, and returns
  /// how far down it ends. The space above it and below it is what the file states, which is
  /// none where it states none.
  fn paragraph(&mut self, paragraph: &Paragraph, left: f64, right: f64, top: f64) -> f64 {
    let right = (right - paragraph.indent_right.max(0.0)).max(left + 1.0);
    let left = (left + paragraph.indent_left.max(0.0)).min(right - 1.0);
    let tokens = self.tokens(paragraph);
    let base = base_size(&tokens).unwrap_or(DEFAULT_SIZE);
    let mut cursor = top + paragraph.space_before.max(0.0);
    for line in set_lines(tokens, left, right, true) {
      let height = line_height(&line, base);
      let end = line.last().map_or(left, |set| set.x1);
      let shift = match paragraph.align {
        Align::Center => ((right - end) / 2.0).max(0.0),
        Align::Right => (right - end).max(0.0),
        Align::Left | Align::Justified => 0.0,
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
    cursor + paragraph.space_after.max(0.0)
  }

  /// Sets one row of a table, whose cells end at the edges the file gives, measured from
  /// `origin`. Returns how far down it ends.
  fn row(&mut self, row: &Row, origin: f64, limit: f64, top: f64) -> f64 {
    let mut end = top + LINE_EM * DEFAULT_SIZE;
    let mut start = origin + row.left;
    for (index, cell) in row.cells.iter().enumerate() {
      // A cell the row gives no edge for runs to the right margin.
      let edge = row.edges.get(index).map_or(limit, |edge| origin + edge);
      let (x0, x1) = (start, edge.max(start + 1.0));
      let (left, right) = match x1 - x0 > 4.0 * CELL_PAD {
        true => (x0 + CELL_PAD, x1 - CELL_PAD),
        false => (x0, x1),
      };
      let mut cursor = top;
      for paragraph in cell {
        cursor = self.paragraph(paragraph, left, right, cursor);
      }
      end = end.max(cursor);
      start = x1;
    }
    end
  }

  /// Sets blocks one under the other, with no regard for pages or for a place of their own:
  /// the content of a header or a footer.
  fn blocks(&mut self, blocks: &[Block], left: f64, right: f64, top: f64) -> f64 {
    let mut cursor = top;
    for block in blocks {
      cursor = match block {
        Block::Paragraph(paragraph) => self.paragraph(paragraph, left, right, cursor),
        Block::Row(row) => self.row(row, left, right, cursor),
        Block::PageBreak => cursor,
      };
    }
    cursor
  }

  /// Starts a page: sets its header and returns how far down the text starts, which is the
  /// top margin, or the header's end when it is taller than that.
  fn open(&mut self, number: u32, frame: &Frame, running: &Running) -> f64 {
    let header = match (number, &running.first_header) {
      (1, Some(first)) => first,
      _ => &running.header,
    };
    if header.is_empty() {
      return frame.top;
    }
    let top = EDGE_DISTANCE.min(frame.top / 2.0);
    let end = self.blocks(header, frame.left, frame.right, top);
    frame.top.max(end)
  }

  /// Ends a page whose text stops `body_end` down it: sets its footer, and gives the page.
  fn close(&mut self, number: u32, frame: &Frame, running: &Running, body_end: f64) -> PageLayout {
    let footer = match (number, &running.first_footer) {
      (1, Some(first)) => first,
      _ => &running.footer,
    };
    if !footer.is_empty() {
      // Set from the top to learn how tall it is, then moved to where it belongs: above the
      // bottom edge, or under the text when the text runs that far.
      let body = std::mem::take(&mut self.words);
      let height = self.blocks(footer, frame.left, frame.right, 0.0);
      let top = (frame.height - EDGE_DISTANCE.min(frame.bottom / 2.0) - height).max(body_end);
      let mut footer_words = std::mem::replace(&mut self.words, body);
      for word in &mut footer_words {
        word.top += top;
        word.bottom += top;
      }
      self.words.append(&mut footer_words);
    }
    let lowest = self
      .words
      .iter()
      .map(|word| word.bottom)
      .fold(0.0, f64::max);
    let mut warnings = vec![ESTIMATED.to_string()];
    // One block taller than the page stays whole, on a page made tall enough to hold it.
    let height = match lowest > frame.height + SLACK {
      true => {
        let height = lowest + (frame.height - frame.bottom);
        warnings.push(format!(
          "the content is estimated to run past the paper ({:.0} points); the page is {height:.0} points tall to hold it",
          frame.height
        ));
        height
      }
      false => frame.height,
    };
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

/// The document's pages, in order. There is always at least one.
pub(crate) fn pages(body: &Body) -> Vec<PageLayout> {
  let frame = Frame::of(body.paper);
  let running = &body.running;
  let mut flow = Flow {
    fonts: Fonts::default(),
    words: Vec::new(),
  };
  let mut pages = Vec::new();
  let mut number = 1;
  // Where the text of the page being set starts, and how far down it has got.
  let mut start = flow.open(number, &frame, running);
  let mut cursor = start;
  // How far down the frame being filled has been set.
  let mut frame_end = 0.0;
  for block in &body.blocks {
    // A paragraph with a place of its own is set there, and the flow goes on as though it
    // were not: a report is a page of such paragraphs, each put where its program wanted it.
    if let Block::Paragraph(paragraph) = block
      && let Some(placing) = paragraph.placing
    {
      let left = placing.x
        + if placing.from_page_side {
          0.0
        } else {
          frame.left
        };
      let top = placing.y
        + if placing.from_page_top {
          0.0
        } else {
          frame.top
        };
      let left = left.clamp(0.0, frame.width - 1.0);
      let right = match placing.width {
        Some(width) => (left + width).min(frame.width),
        None => frame.right.max(left + 1.0),
      };
      // A paragraph that goes on in the frame before it starts where that one ended.
      let top = match paragraph.continues_frame {
        true => frame_end,
        false => top.max(0.0),
      };
      frame_end = flow.paragraph(paragraph, left, right, top);
      continue;
    }
    let before = flow.words.len();
    let end = match block {
      Block::Paragraph(paragraph) => flow.paragraph(paragraph, frame.left, frame.right, cursor),
      Block::Row(row) => flow.row(row, frame.left, frame.right, cursor),
      Block::PageBreak => {
        pages.push(flow.close(number, &frame, running, cursor));
        number += 1;
        start = flow.open(number, &frame, running);
        cursor = start;
        continue;
      }
    };
    // How far down the block's own words reach: the space after a paragraph may hang over
    // the foot of the page, and its words may not.
    let lowest = flow.words[before..]
      .iter()
      .map(|word| word.bottom)
      .fold(cursor, f64::max);
    if lowest > frame.bottom + SLACK && cursor > start + SLACK {
      // The block goes to the top of the next page, whole, under that page's header.
      let moved = flow.words.split_off(before);
      pages.push(flow.close(number, &frame, running, cursor));
      number += 1;
      start = flow.open(number, &frame, running);
      let up = cursor - start;
      flow.words.extend(moved.into_iter().map(|mut word| {
        word.top -= up;
        word.bottom -= up;
        word
      }));
      cursor = end - up;
    } else {
      cursor = end;
    }
  }
  pages.push(flow.close(number, &frame, running, cursor));
  pages
}

#[cfg(test)]
mod tests;
