//! Blocks and inlines in reading order.

use crate::model::{Block, Inline, Paragraph, Row};

/// Calls `f` with every paragraph under `blocks` in reading order, the ones in table cells
/// included.
pub(crate) fn each_paragraph<'a>(blocks: &'a [Block], f: &mut impl FnMut(&'a Paragraph)) {
  for block in blocks {
    match block {
      Block::Paragraph(paragraph) => f(paragraph),
      Block::Table(table) => {
        for cell in table.rows.iter().flat_map(|row| &row.cells) {
          each_paragraph(&cell.blocks, f);
        }
      }
    }
  }
}

/// Calls `f` with every inline under `inlines`, a link before what it holds.
pub(crate) fn each_inline<'a>(inlines: &'a [Inline], f: &mut impl FnMut(&'a Inline)) {
  for inline in inlines {
    f(inline);
    if let Inline::Link(link) = inline {
      each_inline(&link.inlines, f);
    }
  }
}

/// The page a row starts on: where the first block of its first cell is.
pub(crate) fn row_page(row: &Row) -> Option<u32> {
  row
    .cells
    .first()
    .and_then(|cell| cell.blocks.first())
    .map(Block::page)
}
