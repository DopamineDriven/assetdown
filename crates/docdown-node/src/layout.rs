//! A document's pages of positioned words as JavaScript sees them.

use contentdown::PageLayout;
use napi_derive::napi;

/// One word of a page, with its box in the page frame: points, y up, the page's bottom-left
/// corner at the origin.
#[napi(object, object_from_js = false)]
pub struct DocWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `DocPage.lines`.
  pub line: u32,
  /// The size of the text in points.
  pub font_size: Option<f64>,
  /// The font the text names; absent when the file's table of fonts does not have it.
  pub font_name: Option<String>,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount` address a contiguous
/// slice of `DocPage.words`.
#[napi(object, object_from_js = false)]
pub struct DocLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// One page of a document. A file of this kind records a flow of text and not where it lands,
/// so the positions are worked out: a paragraph is set from the left and wrapped between the
/// margins the file states, the cells of a row between the edges the file gives them, and a
/// page is turned at a page break and at the foot of the page. The file's header is set at the
/// head of each page and its footer at the foot. A paragraph the file gives a place of its
/// own, as a program that writes a report gives every line, is set at that place, which is the
/// file's and no estimate.
#[napi(object, object_from_js = false)]
pub struct DocPage {
  /// From 1.
  pub page: u32,
  /// The page's size in points: the paper the file states, or letter when it states none.
  /// `height` is larger than the paper's when one block is estimated to run past it.
  pub width: f64,
  pub height: f64,
  pub words: Vec<DocWord>,
  pub lines: Vec<DocLine>,
  pub warnings: Vec<String>,
}

impl From<PageLayout> for DocPage {
  fn from(page: PageLayout) -> Self {
    DocPage {
      page: page.page,
      width: page.width,
      height: page.height,
      words: page
        .words
        .into_iter()
        .map(|word| DocWord {
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
        })
        .collect(),
      lines: page
        .lines
        .into_iter()
        .map(|line| DocLine {
          text: line.text,
          x0: line.x0,
          y0: line.y0,
          x1: line.x1,
          y1: line.y1,
          first_word: line.first_word,
          word_count: line.word_count,
        })
        .collect(),
      warnings: page.warnings,
    }
  }
}

pub(crate) fn pages_of(pages: Vec<PageLayout>) -> Vec<DocPage> {
  pages.into_iter().map(Into::into).collect()
}
