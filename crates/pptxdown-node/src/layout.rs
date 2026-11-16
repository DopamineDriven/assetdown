//! A deck's slides as pages of positioned words, as JavaScript sees them.

use napi_derive::napi;
use pptxdown::Presentation;

/// One word of a slide, with its box in the page frame: points, y up, the slide's bottom-left
/// corner at the origin.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct PptxWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `PptxPage.lines`.
  pub line: u32,
  /// The size of the text in points, and whether it is bold or italic, resolved through the
  /// slide's layout and master. Text PowerPoint shrank to fit its box has the size it was shrunk
  /// to.
  pub font_size: Option<f64>,
  /// The font the text names; absent when it names none of its own and takes the theme's.
  pub font_name: Option<String>,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount` address a contiguous
/// slice of `PptxPage.words`.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct PptxLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// The box a picture is shown in, in the page frame the words use.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct PptxPlacedImage {
  /// The picture's part in the package (`ppt/media/image1.png`), or the address of a linked
  /// picture: the `name` `images()` gives it.
  pub name: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

/// One slide as a page. A slide records where every shape is, so the boxes are the file's own:
/// which box a word is in, the cell of a table it is in, where a picture sits, and the size and
/// weight of the text. What a slide does not record is where a line breaks inside its box; that
/// is estimated (a character is taken to be half its font size wide, a line 1.2 times its size
/// tall), and text the estimate makes too tall is set closer together so it stays on the slide.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct PptxPage {
  /// The slide's number, from 1: the same `page` every other reader names.
  pub page: u32,
  /// The slide's size in points.
  pub width: f64,
  pub height: f64,
  pub words: Vec<PptxWord>,
  pub lines: Vec<PptxLine>,
  /// The pictures of the slide, in reading order.
  pub images: Vec<PptxPlacedImage>,
  pub warnings: Vec<String>,
}

pub(crate) fn pages_of(deck: &Presentation) -> Vec<PptxPage> {
  deck
    .layout()
    .into_iter()
    .map(|page| PptxPage {
      page: page.page,
      width: page.width,
      height: page.height,
      words: page
        .words
        .into_iter()
        .map(|word| PptxWord {
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
        .map(|line| PptxLine {
          text: line.text,
          x0: line.x0,
          y0: line.y0,
          x1: line.x1,
          y1: line.y1,
          first_word: line.first_word,
          word_count: line.word_count,
        })
        .collect(),
      images: page
        .images
        .into_iter()
        .map(|image| PptxPlacedImage {
          name: image.xobject_name,
          x0: image.x0,
          y0: image.y0,
          x1: image.x1,
          y1: image.y1,
        })
        .collect(),
      warnings: page.warnings,
    })
    .collect()
}
