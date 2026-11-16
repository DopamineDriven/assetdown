//! Where pages split.
//!
//! A docx records a flow, not pages. Two things in it say where a page ended: the breaks an
//! author asked for, and the marker Word leaves in the first run of each page when it saves.
//! The reader counts both as it walks the body, and every block, link and picture carries the
//! page it starts on. A file no layout engine has saved carries no markers, so all of it is
//! page 1; and Word does not always mark the last pages it laid out, so the count can fall short
//! of the one the application recorded. `Metadata` reports both.

use crate::model::{Block, Document};
use crate::walk::row_page;

impl Document {
  /// The body's text, one string for each page the reader counted. A paragraph belongs to the
  /// page it starts on, however far it runs; a table is split between its rows. Joined by line
  /// breaks, the pages are [`Document::text`].
  pub fn text_per_page(&self) -> Vec<String> {
    let count = self.page_count.max(1) as usize;
    let mut pages: Vec<Vec<String>> = vec![Vec::new(); count];
    // Pages only go forward, whatever a block says.
    let mut current = 1;
    let mut put = |page: u32, line: String, pages: &mut Vec<Vec<String>>| {
      current = current.max(page).min(count as u32);
      pages[current as usize - 1].push(line);
    };
    for block in &self.blocks {
      match block {
        Block::Paragraph(paragraph) => put(paragraph.page, paragraph.line(), &mut pages),
        Block::Table(table) => {
          for row in &table.rows {
            put(row_page(row).unwrap_or(table.page), row.text(), &mut pages);
          }
        }
      }
    }
    pages.into_iter().map(|lines| lines.join("\n")).collect()
  }
}
