//! Pictures where they are placed.
//!
//! [`Document::images`] holds each picture once. A placement is one use of a picture: the same
//! logo in a header and on the first page is one image and two placements.

use crate::annotations::{PartKind, counted};
use crate::model::{Block, Document, Inline, NoteKind};
use crate::walk::{each_inline, each_paragraph};

/// One use of a picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
  /// From 1, by the reader's page count. Absent in a header or a footer, which belong to no one
  /// page, and in a note or a comment that nothing refers to.
  pub page: Option<u32>,
  pub part: PartKind,
  /// Its place among the pictures of the same page, from 0.
  pub image_index: u32,
  /// Index into [`Document::images`].
  pub image: u32,
  /// The size it is shown at, in points.
  pub display_width: Option<f64>,
  pub display_height: Option<f64>,
  pub alt_text: Option<String>,
}

fn collect(blocks: &[Block], part: PartKind, page: Option<u32>, out: &mut Vec<Placement>) {
  each_paragraph(blocks, &mut |paragraph| {
    each_inline(&paragraph.inlines, &mut |inline| {
      if let Inline::Image(used) = inline {
        out.push(Placement {
          page: if part == PartKind::Body {
            Some(used.page)
          } else {
            page
          },
          part,
          image_index: 0,
          image: used.image,
          display_width: used.display_width,
          display_height: used.display_height,
          alt_text: used.alt_text.clone(),
        });
      }
    });
  });
}

impl Document {
  /// Every place a picture is used, page by page and in reading order within a page. Those of
  /// headers and footers come last.
  pub fn placements(&self) -> Vec<Placement> {
    let mut out = Vec::new();
    collect(&self.blocks, PartKind::Body, None, &mut out);
    for note in &self.notes {
      let part = match note.kind {
        NoteKind::Footnote => PartKind::Footnote,
        NoteKind::Endnote => PartKind::Endnote,
      };
      collect(&note.blocks, part, counted(note.page), &mut out);
    }
    for comment in &self.comments {
      collect(
        &comment.blocks,
        PartKind::Comment,
        counted(comment.page),
        &mut out,
      );
    }
    for header in &self.headers {
      collect(&header.blocks, PartKind::Header, None, &mut out);
    }
    for footer in &self.footers {
      collect(&footer.blocks, PartKind::Footer, None, &mut out);
    }
    out.sort_by_key(|placement| placement.page.unwrap_or(u32::MAX));
    let mut previous = None;
    let mut next = 0;
    for placement in &mut out {
      if previous != Some(placement.page) {
        previous = Some(placement.page);
        next = 0;
      }
      placement.image_index = next;
      next += 1;
    }
    out
  }
}
