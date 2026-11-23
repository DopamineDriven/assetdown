//! A Word document: its text as blocks, its paper, and its headers and footers.

use cfbdown::{CfbError, CompoundFile};
use contentdown::PageLayout;
use contentdown::document::{self, Block, Body, Paper, Running};
use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};

use crate::body::{self, Source};
use crate::boxes::Boxes;
use crate::embedded::{self, Embedded};
use crate::error::DocError;
use crate::fib::{Fib, Part};
use crate::pictures::{self, Picture};
use crate::pieces::{self, Unit};
use crate::props::{self, Of, Stated, Styles, properties};

const ESTIMATED: &str = "word positions are estimated from the flow of the text; a Word file of this kind records no layout";
/// The storage Word keeps a document's macros in.
const MACROS: &str = "Macros";
/// Where a section's header and footer for odd pages, and those for its first page, are
/// among the headers and footers: six notes on footnotes come first, and each section has
/// six after them, of which the first and the third are for even pages.
const HEADER: usize = 7;
const FOOTER: usize = 9;
const FIRST_HEADER: usize = 10;
const FIRST_FOOTER: usize = 11;

/// A document of Word 97 to 2003, read.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
  body: Body,
  embedded: Vec<Embedded>,
  pictures: Vec<Picture>,
  warnings: Vec<String>,
  is_template: bool,
  has_macros: bool,
}

fn malformed(detail: &str) -> DocError {
  DocError::Malformed(detail.to_string())
}

/// The paper and its margins as the first section states them, and whether its first page
/// has a header and a footer of its own. What it does not state is letter paper with the
/// margins Word gives a new document.
fn paper_of(section: &[u8]) -> (Paper, bool) {
  let mut paper = Paper {
    width: 612.0,
    height: 792.0,
    left: 90.0,
    right: 90.0,
    top: 72.0,
    bottom: 72.0,
  };
  let mut first_page_differs = false;
  for property in properties(section) {
    let length = property.u16().map(|twips| f64::from(twips) / 20.0);
    // A margin above or below may be stated as a length below nought, which says that the
    // header may not push the text down. It is as far from the edge either way.
    let either_way = property
      .i16()
      .map(|twips| f64::from(twips.unsigned_abs()) / 20.0);
    match (property.code, length, either_way) {
      (0xB01F, Some(length), _) => paper.width = length,
      (0xB020, Some(length), _) => paper.height = length,
      (0xB021, Some(length), _) => paper.left = length,
      (0xB022, Some(length), _) => paper.right = length,
      (0x9023, _, Some(length)) => paper.top = length,
      (0x9024, _, Some(length)) => paper.bottom = length,
      (0x300A, _, _) => first_page_differs = property.u8().is_some_and(|value| value != 0),
      _ => {}
    }
  }
  (paper, first_page_differs)
}

/// The headers and footers: their text is one run after the document's own and its
/// footnotes, and a table says where each of them starts in it.
fn running_of(
  story: &[Unit],
  starts: Option<&[u8]>,
  source: &Source<'_>,
  first_page_differs: bool,
) -> Running {
  let starts: Vec<usize> = starts
    .unwrap_or(&[])
    .as_chunks::<4>()
    .0
    .iter()
    .filter_map(|start| usize::try_from(u32::from_le_bytes(*start)).ok())
    .collect();
  let part = |index: usize| -> Vec<Block> {
    let units = starts
      .get(index)
      .zip(starts.get(index + 1))
      .and_then(|(start, end)| story.get(*start..*end));
    units.map_or(Vec::new(), |units| body::blocks(units, source, None))
  };
  Running {
    header: part(HEADER),
    footer: part(FOOTER),
    first_header: first_page_differs.then(|| part(FIRST_HEADER)),
    first_footer: first_page_differs.then(|| part(FIRST_FOOTER)),
  }
}

impl Doc {
  /// Reads a file. Fails with `unsupported format: …` for a document of Word 6 or 95, for one
  /// saved with a password, and for a compound file that holds no Word document, and with
  /// `malformed document: …` for bytes that are no compound file or a document that is damaged.
  pub fn open(bytes: &[u8]) -> Result<Doc, DocError> {
    let file = CompoundFile::open(bytes).map_err(|error| match error {
      CfbError::NotCompound => malformed("not a compound file"),
      other => DocError::Malformed(other.to_string()),
    })?;
    let Some(main) = file.stream("WordDocument") else {
      return Err(DocError::UnsupportedFormat(
        "a compound file that holds no Word document".to_string(),
      ));
    };
    let fib = Fib::read(&main)?;
    let table = file
      .stream(fib.table)
      .ok_or_else(|| malformed("the stream that describes the text is missing"))?;
    let mut warnings = Vec::new();
    let pieces = fib
      .part(Part::Pieces, &table)
      .ok_or_else(|| malformed("the table of the text's pieces cannot be read"))?;
    let units = pieces::read(&main, pieces, &mut warnings)?;

    let paragraphs = Stated::read(
      &main,
      fib.part(Part::ParagraphProperties, &table),
      Of::Paragraphs,
    );
    let characters = Stated::read(
      &main,
      fib.part(Part::CharacterProperties, &table),
      Of::Characters,
    );
    let styles = Styles::read(fib.part(Part::StyleSheet, &table));
    let fonts = props::fonts(fib.part(Part::Fonts, &table));
    let source = Source {
      paragraphs: &paragraphs,
      characters: &characters,
      styles: &styles,
      fonts: &fonts,
    };
    let own = units.get(..fib.text).unwrap_or(&units);
    let section = props::first_section(&main, fib.part(Part::Sections, &table));
    let (paper, first_page_differs) = paper_of(section);
    let after = fib.text.saturating_add(fib.footnotes);
    let story = units
      .get(after..)
      .map_or(&[][..], |rest| &rest[..fib.running.min(rest.len())]);
    let running = running_of(
      story,
      fib.part(Part::Running, &table),
      &source,
      first_page_differs,
    );
    // The text boxes' text comes after every other run of text.
    let after = after
      .saturating_add(fib.running)
      .saturating_add(fib.before_boxes);
    let boxed = units
      .get(after..)
      .map_or(&[][..], |rest| &rest[..fib.boxes.min(rest.len())]);
    let boxes = Boxes::read(
      boxed,
      fib.part(Part::Boxes, &table),
      fib.part(Part::Shapes, &table),
      fib.part(Part::Drawings, &table),
    );
    Ok(Doc {
      body: Body {
        blocks: body::blocks(own, &source, Some(&boxes)),
        paper,
        running,
      },
      embedded: embedded::read(&file),
      pictures: pictures::read(
        &units,
        &characters,
        file.stream("Data").as_deref(),
        fib.part(Part::Drawings, &table),
        &main,
      ),
      warnings,
      is_template: fib.is_template,
      has_macros: file.is_storage(MACROS),
    })
  }

  /// The document's text: a line to a paragraph, and a line to a table's row with a tab
  /// between its cells. The text of a text box is where its shape is anchored. Headers,
  /// footers, and footnotes are not in it.
  pub fn text(&self) -> String {
    self.body.text()
  }

  /// The document's paragraphs, the rows of its tables, and its page breaks, in order.
  pub fn blocks(&self) -> &[Block] {
    &self.body.blocks
  }

  /// The paper the first section states, with its margins; letter where it states none.
  pub fn paper(&self) -> Paper {
    self.body.paper
  }

  /// The header and the footer of the pages, and those of the first page where the file says
  /// it has its own. They are on the pages `layout` gives, and not in `text`.
  pub fn running(&self) -> &Running {
    &self.body.running
  }

  /// The document set on pages, by estimate: a paragraph wrapped between the margins the
  /// file states, the cells of a row between the edges the file gives them, and a page turned
  /// at a page break and where the text reaches the foot of one. Every page says that its
  /// positions are estimated.
  pub fn layout(&self) -> Vec<PageLayout> {
    document::pages(&self.body, ESTIMATED)
  }

  /// The files embedded in the document, in the order they were put in it. Each is given as
  /// it was before it was embedded, and is not read: it goes to the reader of what it is.
  pub fn embedded(&self) -> &[Embedded] {
    &self.embedded
  }

  /// The document's pictures: those set in the text, in the order they come in it, and then
  /// those of shapes that float over it. The picture that stands for an embedded object is
  /// not among them.
  pub fn pictures(&self) -> &[Picture] {
    &self.pictures
  }

  /// Invoice fields read from the pages as [`Doc::layout`] sets them. The files the document
  /// carries are not read here, and an invoice may be one of them: when it carries any, the
  /// first of the warnings says so.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    let mut invoice = extract_from_pages(&self.layout(), options);
    let carried = match self.embedded.len() {
      0 => None,
      1 => Some(
        "the document carries 1 embedded file, which is not read here: the invoice may be in it"
          .to_string(),
      ),
      count => Some(format!(
        "the document carries {count} embedded files, which are not read here: the invoice may be in one of them"
      )),
    };
    if let Some(carried) = carried {
      invoice.warnings.insert(0, carried);
    }
    invoice
  }

  /// The file is a template.
  pub fn is_template(&self) -> bool {
    self.is_template
  }

  /// The file holds macros. They are never read or run.
  pub fn has_macros(&self) -> bool {
    self.has_macros
  }

  /// What to know of the reading: a part of the text that could not be found.
  pub fn warnings(&self) -> &[String] {
    &self.warnings
  }
}
