//! The verbs that read a page's words, built only with the `ocr` feature: the superset package,
//! `@d0paminedriven/tiffdown-ocr`, which needs Tesseract on the machine. Each verb hands every
//! page's picture to Tesseract; `documentOcr` gives the layout, the text, and the invoice from
//! one such reading, where calling the three verbs apart reads the pages three times.

use std::sync::Arc;

use contentdown::PageLayout;
use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;
use tiffdown::{Recognized, Tiff};

use crate::invoice::{InvoiceOptions, InvoiceResult};
use crate::types::{TiffPage, page_count_of, pages_of};
use crate::{TiffDown, open};

// ── What a caller passes ──────────────────────────────────────────

/// How pages are read.
#[napi(object)]
pub struct OcrOptions {
  /// The language Tesseract reads in, as it names its data; `eng` when left out. Several are
  /// joined with `+`. A language the machine has no data for is said in each page's warnings.
  pub lang: Option<String>,
  /// How many pages are read at once; four when left out, and never more than the machine has
  /// cores.
  pub max_threads: Option<u32>,
}

/// What was asked of a reading, kept so that it can cross to another thread.
#[derive(Clone)]
pub struct Reading {
  lang: String,
  max_threads: Option<u32>,
}

impl Reading {
  fn new(options: Option<OcrOptions>) -> Self {
    let (lang, max_threads) = options.map_or((None, None), |o| (o.lang, o.max_threads));
    Reading {
      lang: lang.unwrap_or_else(|| "eng".to_string()),
      max_threads,
    }
  }

  fn of(&self, tiff: &Tiff) -> Recognized {
    tiff.recognize(&self.lang, self.max_threads)
  }
}

// ── What comes back ───────────────────────────────────────────────

/// One recognized word, with its box in the page frame: points, y up, the page's bottom-left
/// corner at the origin.
#[napi(object, object_from_js = false)]
pub struct TiffWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `TiffPageLayout.lines`.
  pub line: u32,
  /// How sure Tesseract is of the word, from 0 to 1.
  pub confidence: f64,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount` address a contiguous
/// slice of `TiffPageLayout.words`.
#[napi(object, object_from_js = false)]
pub struct TiffLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// One page as it was read: its words where they lie on it, and the lines they make.
#[napi(object, object_from_js = false)]
pub struct TiffPageLayout {
  /// From 1.
  pub page: u32,
  /// The page's size in points as it was read: turned the way its orientation says, and
  /// turned again by `contentRotation` when its text lay another way up. A page that states
  /// no resolution is sized at 300 pixels to the inch, and says so in `warnings`.
  pub width: f64,
  pub height: f64,
  /// Clockwise degrees (0, 90, 180, or 270) the page was turned, beyond what its orientation
  /// says, so that its text reads upright. Every word and line is in the turned frame.
  pub content_rotation: u32,
  pub words: Vec<TiffWord>,
  pub lines: Vec<TiffLine>,
  /// What to know about the reading: a turn that was made, a resolution that was assumed, a
  /// picture that could not be decoded (the page then has no words), a language Tesseract has
  /// no data for.
  pub warnings: Vec<String>,
}

impl From<PageLayout> for TiffPageLayout {
  fn from(page: PageLayout) -> Self {
    TiffPageLayout {
      page: page.page,
      width: page.width,
      height: page.height,
      content_rotation: page.content_rotation,
      words: page
        .words
        .into_iter()
        .map(|word| TiffWord {
          text: word.text,
          x0: word.x0,
          y0: word.y0,
          x1: word.x1,
          y1: word.y1,
          line: word.line,
          // Every word here is Tesseract's, which states a confidence for each.
          confidence: word.confidence.unwrap_or(0.0),
        })
        .collect(),
      lines: page
        .lines
        .into_iter()
        .map(|line| TiffLine {
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

/// The text of one page: its lines from the top down, a line of text to each.
#[napi(object, object_from_js = false)]
pub struct PageText {
  /// From 1.
  pub page: u32,
  pub text: String,
}

/// Everything read from a file, from one reading of its pages.
#[napi(object, object_from_js = false)]
pub struct TiffDocumentOcr {
  /// How many bytes the file has.
  pub byte_size: f64,
  pub page_count: u32,
  /// What the file says of each page, as `pages` gives it.
  pub pages: Vec<TiffPage>,
  /// What was wrong with the file, where it could still be read.
  pub warnings: Vec<String>,
  pub layout: Vec<TiffPageLayout>,
  pub text: Vec<PageText>,
  pub invoice: InvoiceResult,
}

fn text_of(read: &Recognized) -> Vec<PageText> {
  let numbers = read.pages().iter().map(|page| page.page);
  numbers
    .zip(read.text_per_page())
    .map(|(page, text)| PageText { page, text })
    .collect()
}

fn layout_of(read: Recognized) -> Vec<TiffPageLayout> {
  read.into_pages().into_iter().map(Into::into).collect()
}

/// What `documentOcr` gathers on the thread that does the reading.
pub struct WholeReading {
  tiff: Arc<Tiff>,
  read: Recognized,
  invoice: contentdown::invoice::InvoiceResult,
}

impl WholeReading {
  fn of(
    tiff: Arc<Tiff>,
    reading: &Reading,
    options: Option<&contentdown::invoice::InvoiceOptions>,
  ) -> Self {
    let read = reading.of(&tiff);
    let invoice = read.invoice(options);
    WholeReading {
      tiff,
      read,
      invoice,
    }
  }
}

impl From<WholeReading> for TiffDocumentOcr {
  fn from(whole: WholeReading) -> Self {
    TiffDocumentOcr {
      // A file's length is far below 2^53, past which a JS number stops counting by ones.
      byte_size: whole.tiff.byte_size() as f64,
      page_count: page_count_of(&whole.tiff),
      pages: pages_of(&whole.tiff),
      warnings: whole.tiff.warnings().to_vec(),
      text: text_of(&whole.read),
      layout: layout_of(whole.read),
      invoice: whole.invoice.into(),
    }
  }
}

// ── Tasks ─────────────────────────────────────────────────────────

/// The file a task reads: bytes still to be parsed, or a file a `TiffDown` already holds.
pub enum Held {
  Bytes(Vec<u8>),
  Parsed(Arc<Tiff>),
}

impl Held {
  fn tiff(&mut self) -> Result<Arc<Tiff>> {
    match self {
      Held::Bytes(bytes) => open(std::mem::take(bytes)).map(Arc::new),
      Held::Parsed(tiff) => Ok(Arc::clone(tiff)),
    }
  }
}

/// Reads every page's words on a worker thread, for `tiffLayoutAsync` and `layoutAsync`.
pub struct LayoutTask {
  held: Held,
  reading: Reading,
}

#[napi]
impl Task for LayoutTask {
  type Output = Recognized;
  type JsValue = Vec<TiffPageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let tiff = self.held.tiff()?;
    Ok(self.reading.of(&tiff))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(layout_of(output))
  }
}

/// Reads every page's text on a worker thread, for `tiffTextPerPageAsync` and
/// `textPerPageAsync`.
pub struct TextPerPageTask {
  held: Held,
  reading: Reading,
}

#[napi]
impl Task for TextPerPageTask {
  type Output = Recognized;
  type JsValue = Vec<PageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let tiff = self.held.tiff()?;
    Ok(self.reading.of(&tiff))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(text_of(&output))
  }
}

/// Reads the pages and the invoice on them on a worker thread, for `tiffInvoiceAsync` and
/// `invoiceAsync`.
pub struct InvoiceTask {
  held: Held,
  reading: Reading,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for InvoiceTask {
  type Output = contentdown::invoice::InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    let tiff = self.held.tiff()?;
    let read = self.reading.of(&tiff);
    Ok(read.invoice(self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Reads the pages once and gathers everything on a worker thread, for `tiffDocumentOcrAsync`
/// and `documentOcrAsync`.
pub struct DocumentOcrTask {
  held: Held,
  reading: Reading,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for DocumentOcrTask {
  type Output = WholeReading;
  type JsValue = TiffDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    let tiff = self.held.tiff()?;
    Ok(WholeReading::of(tiff, &self.reading, self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// ── Functions ─────────────────────────────────────────────────────

/// Every page of a TIFF buffer as it was read: its words where they lie on the page, and the
/// lines they make. A page whose picture cannot be decoded is there with no words and a warning
/// that gives the reason. Throws `unsupported format: …` for bytes that are not a TIFF.
#[napi]
pub fn tiff_layout(buffer: Buffer, ocr: Option<OcrOptions>) -> Result<Vec<TiffPageLayout>> {
  let tiff = open(buffer.to_vec())?;
  Ok(layout_of(Reading::new(ocr).of(&tiff)))
}

/// `tiffLayout` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn tiff_layout_async(buffer: Buffer, ocr: Option<OcrOptions>) -> AsyncTask<LayoutTask> {
  AsyncTask::new(LayoutTask {
    held: Held::Bytes(buffer.to_vec()),
    reading: Reading::new(ocr),
  })
}

/// The text of each page of a TIFF buffer: its lines from the top down.
#[napi]
pub fn tiff_text_per_page(buffer: Buffer, ocr: Option<OcrOptions>) -> Result<Vec<PageText>> {
  let tiff = open(buffer.to_vec())?;
  Ok(text_of(&Reading::new(ocr).of(&tiff)))
}

/// `tiffTextPerPage` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn tiff_text_per_page_async(
  buffer: Buffer,
  ocr: Option<OcrOptions>,
) -> AsyncTask<TextPerPageTask> {
  AsyncTask::new(TextPerPageTask {
    held: Held::Bytes(buffer.to_vec()),
    reading: Reading::new(ocr),
  })
}

/// Invoice fields read from the recognized pages of a TIFF buffer. What the pages warn of
/// comes first among its warnings, so an invoice read from some of a file's pages names the
/// pages that were not read.
#[napi]
pub fn tiff_invoice(
  buffer: Buffer,
  opts: Option<InvoiceOptions>,
  ocr: Option<OcrOptions>,
) -> Result<InvoiceResult> {
  let tiff = open(buffer.to_vec())?;
  let read = Reading::new(ocr).of(&tiff);
  Ok(read.invoice(opts.map(Into::into).as_ref()).into())
}

/// `tiffInvoice` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn tiff_invoice_async(
  buffer: Buffer,
  opts: Option<InvoiceOptions>,
  ocr: Option<OcrOptions>,
) -> AsyncTask<InvoiceTask> {
  AsyncTask::new(InvoiceTask {
    held: Held::Bytes(buffer.to_vec()),
    reading: Reading::new(ocr),
    options: opts.map(Into::into),
  })
}

/// Everything read from a TIFF buffer, from one reading of its pages: what the file says of
/// each page, the layout, the text, and the invoice.
#[napi]
pub fn tiff_document_ocr(
  buffer: Buffer,
  ocr: Option<OcrOptions>,
  invoice: Option<InvoiceOptions>,
) -> Result<TiffDocumentOcr> {
  let tiff = Arc::new(open(buffer.to_vec())?);
  let options = invoice.map(Into::into);
  Ok(WholeReading::of(tiff, &Reading::new(ocr), options.as_ref()).into())
}

/// `tiffDocumentOcr` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn tiff_document_ocr_async(
  buffer: Buffer,
  ocr: Option<OcrOptions>,
  invoice: Option<InvoiceOptions>,
) -> AsyncTask<DocumentOcrTask> {
  AsyncTask::new(DocumentOcrTask {
    held: Held::Bytes(buffer.to_vec()),
    reading: Reading::new(ocr),
    options: invoice.map(Into::into),
  })
}

// ── The class, with words ─────────────────────────────────────────

#[napi]
impl TiffDown {
  /// Every page as it was read: its words where they lie on the page, and the lines they make.
  /// A page whose picture cannot be decoded is there with no words and a warning that gives
  /// the reason. This reads every page, on the JavaScript thread; `layoutAsync` does not hold
  /// that thread.
  #[napi]
  pub fn layout(&self, ocr: Option<OcrOptions>) -> Vec<TiffPageLayout> {
    layout_of(Reading::new(ocr).of(&self.inner))
  }

  /// `layout` on a worker thread.
  #[napi]
  pub fn layout_async(&self, ocr: Option<OcrOptions>) -> AsyncTask<LayoutTask> {
    AsyncTask::new(LayoutTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      reading: Reading::new(ocr),
    })
  }

  /// The text of each page: its lines from the top down.
  #[napi]
  pub fn text_per_page(&self, ocr: Option<OcrOptions>) -> Vec<PageText> {
    text_of(&Reading::new(ocr).of(&self.inner))
  }

  /// `textPerPage` on a worker thread.
  #[napi]
  pub fn text_per_page_async(&self, ocr: Option<OcrOptions>) -> AsyncTask<TextPerPageTask> {
    AsyncTask::new(TextPerPageTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      reading: Reading::new(ocr),
    })
  }

  /// Invoice fields read from the recognized pages. What the pages warn of comes first among
  /// its warnings.
  #[napi]
  pub fn invoice(&self, opts: Option<InvoiceOptions>, ocr: Option<OcrOptions>) -> InvoiceResult {
    let read = Reading::new(ocr).of(&self.inner);
    read.invoice(opts.map(Into::into).as_ref()).into()
  }

  /// `invoice` on a worker thread.
  #[napi]
  pub fn invoice_async(
    &self,
    opts: Option<InvoiceOptions>,
    ocr: Option<OcrOptions>,
  ) -> AsyncTask<InvoiceTask> {
    AsyncTask::new(InvoiceTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      reading: Reading::new(ocr),
      options: opts.map(Into::into),
    })
  }

  /// Everything read from the file, from one reading of its pages: what the file says of each
  /// page, the layout, the text, and the invoice. Calling `layout`, `textPerPage`, and
  /// `invoice` apart reads the pages three times; this reads them once.
  #[napi]
  pub fn document_ocr(
    &self,
    ocr: Option<OcrOptions>,
    invoice: Option<InvoiceOptions>,
  ) -> TiffDocumentOcr {
    let options = invoice.map(Into::into);
    WholeReading::of(
      Arc::clone(&self.inner),
      &Reading::new(ocr),
      options.as_ref(),
    )
    .into()
  }

  /// `documentOcr` on a worker thread.
  #[napi]
  pub fn document_ocr_async(
    &self,
    ocr: Option<OcrOptions>,
    invoice: Option<InvoiceOptions>,
  ) -> AsyncTask<DocumentOcrTask> {
    AsyncTask::new(DocumentOcrTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      reading: Reading::new(ocr),
      options: invoice.map(Into::into),
    })
  }
}
