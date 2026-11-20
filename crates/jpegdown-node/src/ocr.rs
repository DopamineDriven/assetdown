//! The verbs that read the page's words, built only with the `ocr` feature: the superset
//! package, `@d0paminedriven/jpegdown-ocr`, which needs Tesseract on the machine. Each verb
//! hands the picture to Tesseract; `documentOcr` gives the layout, the text, and the invoice
//! from one such reading, where calling the three verbs apart reads the page three times.

use std::sync::Arc;

use contentdown::PageLayout;
use jpegdown::{Jpeg, Recognized};
use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

use crate::invoice::{InvoiceOptions, InvoiceResult};
use crate::types::{JpegPage, page_of};
use crate::{Held, JpegDown, js_error, open};

// ── What a caller passes ──────────────────────────────────────────

/// How the page is read.
#[napi(object)]
pub struct OcrOptions {
  /// The language Tesseract reads in, as it names its data; `eng` when left out. Several are
  /// joined with `+`. A language the machine has no data for is said in the page's warnings.
  pub lang: Option<String>,
}

fn lang_of(options: Option<OcrOptions>) -> String {
  options
    .and_then(|options| options.lang)
    .unwrap_or_else(|| "eng".to_string())
}

fn read(jpeg: &Jpeg, lang: &str) -> Result<Recognized> {
  jpeg.recognize(lang).map_err(js_error)
}

// ── What comes back ───────────────────────────────────────────────

/// One recognized word, with its box in the page frame: points, y up, the page's bottom-left
/// corner at the origin.
#[napi(object, object_from_js = false)]
pub struct JpegWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `JpegPageLayout.lines`.
  pub line: u32,
  /// How sure Tesseract is of the word, from 0 to 1.
  pub confidence: f64,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount` address a contiguous
/// slice of `JpegPageLayout.words`.
#[napi(object, object_from_js = false)]
pub struct JpegLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// The page as it was read: its words where they lie on it, and the lines they make.
#[napi(object, object_from_js = false)]
pub struct JpegPageLayout {
  /// The page's size in points as it was read: the page's `sizeAsRead`, turned again by
  /// `contentRotation` when its text lay another way up.
  pub width: f64,
  pub height: f64,
  /// Clockwise degrees (0, 90, 180, or 270) the page was turned, beyond what its orientation
  /// says, so that its text reads upright. Every word and line is in the turned frame.
  pub content_rotation: u32,
  pub words: Vec<JpegWord>,
  pub lines: Vec<JpegLine>,
  /// What to know about the reading: a size that was assumed, a picture with few pixels to
  /// the inch that was enlarged before it was read, a turn that was made, a language
  /// Tesseract has no data for.
  pub warnings: Vec<String>,
}

impl From<PageLayout> for JpegPageLayout {
  fn from(page: PageLayout) -> Self {
    JpegPageLayout {
      width: page.width,
      height: page.height,
      content_rotation: page.content_rotation,
      words: page
        .words
        .into_iter()
        .map(|word| JpegWord {
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
        .map(|line| JpegLine {
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

/// Everything read from a file, from one reading of its page.
#[napi(object, object_from_js = false)]
pub struct JpegDocumentOcr {
  /// How many bytes the file has.
  pub byte_size: f64,
  /// What the file says of its page, as `page` gives it.
  pub page: JpegPage,
  pub layout: JpegPageLayout,
  /// The page's lines from the top down, a line of text to each.
  pub text: String,
  pub invoice: InvoiceResult,
}

/// What `documentOcr` gathers on the thread that does the reading.
pub struct WholeReading {
  jpeg: Arc<Jpeg>,
  read: Recognized,
  invoice: contentdown::invoice::InvoiceResult,
}

impl WholeReading {
  fn of(
    jpeg: Arc<Jpeg>,
    lang: &str,
    options: Option<&contentdown::invoice::InvoiceOptions>,
  ) -> Result<Self> {
    let read = read(&jpeg, lang)?;
    let invoice = read.invoice(options);
    Ok(WholeReading {
      jpeg,
      read,
      invoice,
    })
  }
}

impl From<WholeReading> for JpegDocumentOcr {
  fn from(whole: WholeReading) -> Self {
    JpegDocumentOcr {
      // A file's length is far below 2^53, past which a JS number stops counting by ones.
      byte_size: whole.jpeg.byte_size() as f64,
      page: page_of(&whole.jpeg),
      text: whole.read.text(),
      layout: whole.read.into_page().into(),
      invoice: whole.invoice.into(),
    }
  }
}

// ── Tasks ─────────────────────────────────────────────────────────

/// Reads the page's words on a worker thread, for `jpegLayoutAsync` and `layoutAsync`.
pub struct LayoutTask {
  held: Held,
  lang: String,
}

#[napi]
impl Task for LayoutTask {
  type Output = Recognized;
  type JsValue = JpegPageLayout;

  fn compute(&mut self) -> Result<Self::Output> {
    let jpeg = self.held.jpeg()?;
    read(&jpeg, &self.lang)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_page().into())
  }
}

/// Reads the page's text on a worker thread, for `jpegTextAsync` and `textAsync`.
pub struct TextTask {
  held: Held,
  lang: String,
}

#[napi]
impl Task for TextTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
    let jpeg = self.held.jpeg()?;
    Ok(read(&jpeg, &self.lang)?.text())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Reads the page and the invoice on it on a worker thread, for `jpegInvoiceAsync` and
/// `invoiceAsync`.
pub struct InvoiceTask {
  held: Held,
  lang: String,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for InvoiceTask {
  type Output = contentdown::invoice::InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    let jpeg = self.held.jpeg()?;
    Ok(read(&jpeg, &self.lang)?.invoice(self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Reads the page once and gathers everything on a worker thread, for `jpegDocumentOcrAsync`
/// and `documentOcrAsync`.
pub struct DocumentOcrTask {
  held: Held,
  lang: String,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for DocumentOcrTask {
  type Output = WholeReading;
  type JsValue = JpegDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    let jpeg = self.held.jpeg()?;
    WholeReading::of(jpeg, &self.lang, self.options.as_ref())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// ── Functions ─────────────────────────────────────────────────────

/// The page of a JPEG buffer as it was read: its words where they lie on the page, and the
/// lines they make. Throws as `jpegPng` does when the picture cannot be decoded.
#[napi]
pub fn jpeg_layout(buffer: Buffer, ocr: Option<OcrOptions>) -> Result<JpegPageLayout> {
  let jpeg = open(buffer.to_vec())?;
  Ok(read(&jpeg, &lang_of(ocr))?.into_page().into())
}

/// `jpegLayout` on a worker thread: the buffer is copied, then decoded and read there.
#[napi]
pub fn jpeg_layout_async(buffer: Buffer, ocr: Option<OcrOptions>) -> AsyncTask<LayoutTask> {
  AsyncTask::new(LayoutTask {
    held: Held::Bytes(buffer.to_vec()),
    lang: lang_of(ocr),
  })
}

/// The text of a JPEG buffer's page: its lines from the top down.
#[napi]
pub fn jpeg_text(buffer: Buffer, ocr: Option<OcrOptions>) -> Result<String> {
  let jpeg = open(buffer.to_vec())?;
  Ok(read(&jpeg, &lang_of(ocr))?.text())
}

/// `jpegText` on a worker thread: the buffer is copied, then decoded and read there.
#[napi]
pub fn jpeg_text_async(buffer: Buffer, ocr: Option<OcrOptions>) -> AsyncTask<TextTask> {
  AsyncTask::new(TextTask {
    held: Held::Bytes(buffer.to_vec()),
    lang: lang_of(ocr),
  })
}

/// Invoice fields read from the recognized page of a JPEG buffer. What the page warns of comes
/// first among its warnings.
#[napi]
pub fn jpeg_invoice(
  buffer: Buffer,
  opts: Option<InvoiceOptions>,
  ocr: Option<OcrOptions>,
) -> Result<InvoiceResult> {
  let jpeg = open(buffer.to_vec())?;
  let read = read(&jpeg, &lang_of(ocr))?;
  Ok(read.invoice(opts.map(Into::into).as_ref()).into())
}

/// `jpegInvoice` on a worker thread: the buffer is copied, then decoded and read there.
#[napi]
pub fn jpeg_invoice_async(
  buffer: Buffer,
  opts: Option<InvoiceOptions>,
  ocr: Option<OcrOptions>,
) -> AsyncTask<InvoiceTask> {
  AsyncTask::new(InvoiceTask {
    held: Held::Bytes(buffer.to_vec()),
    lang: lang_of(ocr),
    options: opts.map(Into::into),
  })
}

/// Everything read from a JPEG buffer, from one reading of its page: what the file says of
/// the page, the layout, the text, and the invoice.
#[napi]
pub fn jpeg_document_ocr(
  buffer: Buffer,
  ocr: Option<OcrOptions>,
  invoice: Option<InvoiceOptions>,
) -> Result<JpegDocumentOcr> {
  let jpeg = Arc::new(open(buffer.to_vec())?);
  let options = invoice.map(Into::into);
  Ok(WholeReading::of(jpeg, &lang_of(ocr), options.as_ref())?.into())
}

/// `jpegDocumentOcr` on a worker thread: the buffer is copied, then decoded and read there.
#[napi]
pub fn jpeg_document_ocr_async(
  buffer: Buffer,
  ocr: Option<OcrOptions>,
  invoice: Option<InvoiceOptions>,
) -> AsyncTask<DocumentOcrTask> {
  AsyncTask::new(DocumentOcrTask {
    held: Held::Bytes(buffer.to_vec()),
    lang: lang_of(ocr),
    options: invoice.map(Into::into),
  })
}

// ── The class, with words ─────────────────────────────────────────

#[napi]
impl JpegDown {
  /// The page as it was read: its words where they lie on the page, and the lines they make.
  /// This reads the page on the JavaScript thread; `layoutAsync` does not hold that thread.
  /// Throws as `png` does when the picture cannot be decoded.
  #[napi]
  pub fn layout(&self, ocr: Option<OcrOptions>) -> Result<JpegPageLayout> {
    Ok(read(&self.inner, &lang_of(ocr))?.into_page().into())
  }

  /// `layout` on a worker thread.
  #[napi]
  pub fn layout_async(&self, ocr: Option<OcrOptions>) -> AsyncTask<LayoutTask> {
    AsyncTask::new(LayoutTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      lang: lang_of(ocr),
    })
  }

  /// The page's text: its lines from the top down.
  #[napi]
  pub fn text(&self, ocr: Option<OcrOptions>) -> Result<String> {
    Ok(read(&self.inner, &lang_of(ocr))?.text())
  }

  /// `text` on a worker thread.
  #[napi]
  pub fn text_async(&self, ocr: Option<OcrOptions>) -> AsyncTask<TextTask> {
    AsyncTask::new(TextTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      lang: lang_of(ocr),
    })
  }

  /// Invoice fields read from the recognized page. What the page warns of comes first among
  /// its warnings.
  #[napi]
  pub fn invoice(
    &self,
    opts: Option<InvoiceOptions>,
    ocr: Option<OcrOptions>,
  ) -> Result<InvoiceResult> {
    let read = read(&self.inner, &lang_of(ocr))?;
    Ok(read.invoice(opts.map(Into::into).as_ref()).into())
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
      lang: lang_of(ocr),
      options: opts.map(Into::into),
    })
  }

  /// Everything read from the file, from one reading of its page: what the file says of the
  /// page, the layout, the text, and the invoice. Calling `layout`, `text`, and `invoice`
  /// apart reads the page three times; this reads it once.
  #[napi]
  pub fn document_ocr(
    &self,
    ocr: Option<OcrOptions>,
    invoice: Option<InvoiceOptions>,
  ) -> Result<JpegDocumentOcr> {
    let options = invoice.map(Into::into);
    Ok(WholeReading::of(Arc::clone(&self.inner), &lang_of(ocr), options.as_ref())?.into())
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
      lang: lang_of(ocr),
      options: invoice.map(Into::into),
    })
  }
}
