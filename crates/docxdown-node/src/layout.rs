//! A document's pages of positioned words as JavaScript sees them.

use docxdown::Document;
use napi_derive::napi;

/// One word of a page, with its box in the page frame: points, y up, the page's bottom-left
/// corner at the origin.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct DocxWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `DocxPage.lines`.
  pub line: u32,
  /// The size of the text in points, and whether it is bold or italic, resolved through the
  /// styles.
  pub font_size: Option<f64>,
  /// The font the text names; absent when neither the run nor a style names one.
  pub font_name: Option<String>,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount` address a contiguous
/// slice of `DocxPage.words`.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct DocxLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// One page of a document laid out by estimate. A docx records a flow of text, not where
/// anything lands, so the positions are worked out here: a paragraph is set left to right and
/// wrapped between its section's margins, a table's cells inside the columns of its grid, and
/// the header and footer the page uses above and below. Reading order, the cells of a row on one
/// line, a label beside its value, bold and the font size are kept. Where Word really broke a
/// line, and a font's real widths, are not.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct DocxPage {
  /// From 1, by the reader's page count: the same page a block, a picture or an annotation
  /// names.
  pub page: u32,
  /// The page's size in points: its section's paper, or Letter when the section states none.
  /// `height` is larger than the paper's when the content is estimated to run past it.
  pub width: f64,
  pub height: f64,
  pub words: Vec<DocxWord>,
  pub lines: Vec<DocxLine>,
  pub warnings: Vec<String>,
}

pub(crate) fn pages_of(document: &Document) -> Vec<DocxPage> {
  document
    .layout()
    .into_iter()
    .map(|page| DocxPage {
      page: page.page,
      width: page.width,
      height: page.height,
      words: page
        .words
        .into_iter()
        .map(|word| DocxWord {
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
        .map(|line| DocxLine {
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
    })
    .collect()
}
