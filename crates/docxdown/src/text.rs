//! A document as plain text.

use crate::body::inline_text;
use crate::model::{Block, BreakKind, Cell, Document, Inline, Link, Paragraph, Row, Table};

impl Paragraph {
  /// The paragraph's own text: tabs and line breaks kept, nothing added in front. A page or
  /// column break at either end of the paragraph is not text; inside it, it parts the text on
  /// either side as a line break does.
  pub fn text(&self) -> String {
    let ends =
      |inline: &Inline| matches!(inline, Inline::Break(BreakKind::Page | BreakKind::Column));
    let mut inlines = self.inlines.as_slice();
    while let [first, rest @ ..] = inlines
      && ends(first)
    {
      inlines = rest;
    }
    while let [rest @ .., last] = inlines
      && ends(last)
    {
      inlines = rest;
    }
    let mut out = String::new();
    inline_text(inlines, &mut out);
    out
  }

  /// The paragraph as a line of the document: a list item starts with its marker, as it does
  /// on the page.
  pub fn line(&self) -> String {
    match &self.list {
      Some(item) if !item.marker.is_empty() => format!("{} {}", item.marker, self.text()),
      _ => self.text(),
    }
  }
}

impl Link {
  /// The text the link covers.
  pub fn text(&self) -> String {
    let mut out = String::new();
    inline_text(&self.inlines, &mut out);
    out
  }
}

impl Cell {
  /// The cell's text on one line: its paragraphs joined by spaces, so that a row stays a row.
  pub fn text(&self) -> String {
    self
      .blocks
      .iter()
      .map(|block| match block {
        Block::Paragraph(paragraph) => paragraph.line(),
        Block::Table(table) => table.text(),
      })
      .filter(|text| !text.is_empty())
      .collect::<Vec<_>>()
      .join(" ")
      .replace(['\n', '\t'], " ")
  }

  /// The cell's text as it is laid out: one line per paragraph.
  pub fn lines(&self) -> String {
    blocks_text(&self.blocks)
  }
}

impl Row {
  /// The row on one line, cells separated by tabs.
  pub fn text(&self) -> String {
    self
      .cells
      .iter()
      .map(Cell::text)
      .collect::<Vec<_>>()
      .join("\t")
  }
}

impl Table {
  /// One line per row, cells separated by tabs.
  pub fn text(&self) -> String {
    self
      .rows
      .iter()
      .map(Row::text)
      .collect::<Vec<_>>()
      .join("\n")
  }
}

impl Block {
  /// The page the block starts on.
  pub fn page(&self) -> u32 {
    match self {
      Block::Paragraph(paragraph) => paragraph.page,
      Block::Table(table) => table.page,
    }
  }

  /// Index into [`Document::sections`].
  pub fn section(&self) -> u32 {
    match self {
      Block::Paragraph(paragraph) => paragraph.section,
      Block::Table(table) => table.section,
    }
  }

  pub fn text(&self) -> String {
    match self {
      Block::Paragraph(paragraph) => paragraph.text(),
      Block::Table(table) => table.text(),
    }
  }
}

/// The text of blocks, one line per paragraph and per table row.
pub(crate) fn blocks_text(blocks: &[Block]) -> String {
  blocks
    .iter()
    .map(|block| match block {
      Block::Paragraph(paragraph) => paragraph.line(),
      Block::Table(table) => table.text(),
    })
    .collect::<Vec<_>>()
    .join("\n")
}

impl Document {
  /// The body as plain text, in reading order.
  pub fn text(&self) -> String {
    blocks_text(&self.blocks)
  }
}
