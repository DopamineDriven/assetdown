#![deny(clippy::all)]

//! `@d0paminedriven/rtfdown`: an RTF document, for Node.
//!
//! This crate is the Node-API surface: this file holds the functions, their promise-returning
//! twins, and the class; `types.rs`, `layout.rs`, and `invoice.rs` hold the shapes JavaScript
//! sees. The work itself is done by the `rtfdown` crate, which knows nothing about Node.
//!
//! Every `*Async` function and method returns an [`AsyncTask`]: the input is copied on the JS
//! thread, the work runs on libuv's pool, and the result is turned into JS values back on the
//! JS thread when the promise resolves. The bytes are copied because a caller may go on to
//! change its buffer while the work runs.

use std::sync::Arc;

use contentdown::PageLayout;
use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;
use rtfdown::{Rtf, RtfError};

pub mod invoice;
mod layout;
mod types;

use invoice::{InvoiceOptions, InvoiceResult};
use layout::pages_of;
pub use layout::{RtfLine, RtfPage, RtfWord};
pub use types::{RtfDocument, RtfEmbedded, RtfPicture, RtfPictureFormat};
use types::{embedded_of, pictures_of};

// ── What a caller passes ──────────────────────────────────────────

fn js_error(error: RtfError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: &[u8]) -> Result<Rtf> {
  Rtf::open(bytes).map_err(js_error)
}

/// The document a task reads: bytes still to be parsed, or one an `RtfDown` already holds.
pub enum Held {
  Bytes(Vec<u8>),
  Parsed(Arc<Rtf>),
}

impl Held {
  fn rtf(&mut self) -> Result<Arc<Rtf>> {
    match self {
      Held::Bytes(bytes) => open(&std::mem::take(bytes)).map(Arc::new),
      Held::Parsed(rtf) => Ok(Arc::clone(rtf)),
    }
  }
}

/// Everything read from a document, gathered on the thread that does the reading.
pub struct Whole {
  rtf: Arc<Rtf>,
  pages: Vec<PageLayout>,
  invoice: contentdown::invoice::InvoiceResult,
}

impl Whole {
  fn of(rtf: Arc<Rtf>, options: Option<&contentdown::invoice::InvoiceOptions>) -> Self {
    let pages = rtf.layout();
    let invoice = rtf.invoice(options);
    Whole {
      rtf,
      pages,
      invoice,
    }
  }
}

impl From<Whole> for RtfDocument {
  fn from(whole: Whole) -> Self {
    RtfDocument {
      text: whole.rtf.text(),
      pages: pages_of(whole.pages),
      embedded: embedded_of(&whole.rtf),
      pictures: pictures_of(&whole.rtf),
      warnings: whole.rtf.warnings().to_vec(),
      invoice: whole.invoice.into(),
    }
  }
}

// ── Tasks ─────────────────────────────────────────────────────────

/// Parses a buffer on a worker thread, for `RtfDown.openAsync`.
pub struct OpenTask {
  bytes: Vec<u8>,
}

#[napi]
impl Task for OpenTask {
  type Output = Rtf;
  type JsValue = RtfDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(&std::mem::take(&mut self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(RtfDown {
      inner: Arc::new(output),
    })
  }
}

/// Reads a document's text on a worker thread, for `rtfTextAsync`.
pub struct TextTask {
  held: Held,
}

#[napi]
impl Task for TextTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.held.rtf()?.text())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Sets a document on pages on a worker thread, for `rtfLayoutAsync` and `layoutAsync`.
pub struct LayoutTask {
  held: Held,
}

#[napi]
impl Task for LayoutTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<RtfPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.held.rtf()?.layout())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(pages_of(output))
  }
}

/// Reads a document's invoice fields on a worker thread, for `rtfInvoiceAsync` and
/// `invoiceAsync`.
pub struct InvoiceTask {
  held: Held,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for InvoiceTask {
  type Output = contentdown::invoice::InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.held.rtf()?.invoice(self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Parses a buffer on a worker thread and gives what it carries, for `rtfEmbeddedAsync`.
pub struct EmbeddedTask {
  held: Held,
}

#[napi]
impl Task for EmbeddedTask {
  type Output = Arc<Rtf>;
  type JsValue = Vec<RtfEmbedded>;

  fn compute(&mut self) -> Result<Self::Output> {
    self.held.rtf()
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(embedded_of(&output))
  }
}

/// Parses a buffer on a worker thread and gives its pictures, for `rtfPicturesAsync`.
pub struct PicturesTask {
  held: Held,
}

#[napi]
impl Task for PicturesTask {
  type Output = Arc<Rtf>;
  type JsValue = Vec<RtfPicture>;

  fn compute(&mut self) -> Result<Self::Output> {
    self.held.rtf()
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(pictures_of(&output))
  }
}

/// Reads everything on a worker thread, for `rtfDocumentAsync` and `documentAsync`.
pub struct DocumentTask {
  held: Held,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for DocumentTask {
  type Output = Whole;
  type JsValue = RtfDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(Whole::of(self.held.rtf()?, self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// ── Functions ─────────────────────────────────────────────────────

/// The text of an RTF buffer: a line to a paragraph, and a line to a table's row with a tab
/// between its cells. A header and a footer are not in it: they are on the pages `rtfLayout`
/// gives. Throws `unsupported format: …` for bytes that are not RTF.
#[napi]
pub fn rtf_text(buffer: Buffer) -> Result<String> {
  Ok(open(&buffer)?.text())
}

/// `rtfText` on a worker thread: the buffer is copied, then parsed there.
#[napi]
pub fn rtf_text_async(buffer: Buffer) -> AsyncTask<TextTask> {
  AsyncTask::new(TextTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// An RTF buffer set on pages of positioned words, with the file's header and footer on each.
#[napi]
pub fn rtf_layout(buffer: Buffer) -> Result<Vec<RtfPage>> {
  Ok(pages_of(open(&buffer)?.layout()))
}

/// `rtfLayout` on a worker thread: the buffer is copied, then parsed and set there.
#[napi]
pub fn rtf_layout_async(buffer: Buffer) -> AsyncTask<LayoutTask> {
  AsyncTask::new(LayoutTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// Invoice fields read from an RTF buffer's pages as `rtfLayout` sets them. When the document
/// carries embedded files, the first of the warnings says so: they are not read here, and the
/// invoice may be one of them.
#[napi]
pub fn rtf_invoice(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
  let rtf = open(&buffer)?;
  Ok(rtf.invoice(opts.map(Into::into).as_ref()).into())
}

/// `rtfInvoice` on a worker thread.
#[napi]
pub fn rtf_invoice_async(buffer: Buffer, opts: Option<InvoiceOptions>) -> AsyncTask<InvoiceTask> {
  AsyncTask::new(InvoiceTask {
    held: Held::Bytes(buffer.to_vec()),
    options: opts.map(Into::into),
  })
}

/// The files embedded in an RTF buffer, each as it was before it was embedded: a file attached
/// to a saved mail message, a file dropped into a document, an Office document, a PDF.
#[napi]
pub fn rtf_embedded(buffer: Buffer) -> Result<Vec<RtfEmbedded>> {
  Ok(embedded_of(&open(&buffer)?))
}

/// `rtfEmbedded` on a worker thread.
#[napi]
pub fn rtf_embedded_async(buffer: Buffer) -> AsyncTask<EmbeddedTask> {
  AsyncTask::new(EmbeddedTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// The pictures of an RTF buffer, as their bytes. The picture that stands for an embedded
/// object is not among them.
#[napi]
pub fn rtf_pictures(buffer: Buffer) -> Result<Vec<RtfPicture>> {
  Ok(pictures_of(&open(&buffer)?))
}

/// `rtfPictures` on a worker thread.
#[napi]
pub fn rtf_pictures_async(buffer: Buffer) -> AsyncTask<PicturesTask> {
  AsyncTask::new(PicturesTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// Everything read from an RTF buffer, in one object: its text, its pages, what it carries,
/// its pictures, and its invoice fields.
#[napi]
pub fn rtf_document(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<RtfDocument> {
  let options = opts.map(Into::into);
  Ok(Whole::of(Arc::new(open(&buffer)?), options.as_ref()).into())
}

/// `rtfDocument` on a worker thread.
#[napi]
pub fn rtf_document_async(buffer: Buffer, opts: Option<InvoiceOptions>) -> AsyncTask<DocumentTask> {
  AsyncTask::new(DocumentTask {
    held: Held::Bytes(buffer.to_vec()),
    options: opts.map(Into::into),
  })
}

// ── The class ─────────────────────────────────────────────────────

/// A parsed RTF document. Parsing happens once, in the constructor or in `openAsync`; every
/// method reads from the parsed document.
#[napi]
pub struct RtfDown {
  inner: Arc<Rtf>,
}

#[napi]
impl RtfDown {
  /// Parse a document from its bytes. Throws `unsupported format: …` for bytes that are not
  /// RTF, and `limit exceeded: …` for groups nested deeper than any document nests them.
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(RtfDown {
      inner: Arc::new(open(&buffer)?),
    })
  }

  /// Parse a document off the JavaScript thread; resolves to an `RtfDown`, or rejects with
  /// the errors the constructor throws.
  #[napi]
  pub fn open_async(buffer: Buffer) -> AsyncTask<OpenTask> {
    AsyncTask::new(OpenTask {
      bytes: buffer.to_vec(),
    })
  }

  /// The document's text: a line to a paragraph, and a line to a table's row with a tab
  /// between its cells.
  #[napi]
  pub fn text(&self) -> String {
    self.inner.text()
  }

  /// The document set on pages of positioned words.
  #[napi]
  pub fn layout(&self) -> Vec<RtfPage> {
    pages_of(self.inner.layout())
  }

  /// `layout` on a worker thread.
  #[napi]
  pub fn layout_async(&self) -> AsyncTask<LayoutTask> {
    AsyncTask::new(LayoutTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
    })
  }

  /// Invoice fields read from the document's pages as `layout` sets them.
  #[napi]
  pub fn invoice(&self, opts: Option<InvoiceOptions>) -> InvoiceResult {
    self.inner.invoice(opts.map(Into::into).as_ref()).into()
  }

  /// `invoice` on a worker thread.
  #[napi]
  pub fn invoice_async(&self, opts: Option<InvoiceOptions>) -> AsyncTask<InvoiceTask> {
    AsyncTask::new(InvoiceTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      options: opts.map(Into::into),
    })
  }

  /// The files embedded in the document, each as it was before it was embedded.
  #[napi]
  pub fn embedded(&self) -> Vec<RtfEmbedded> {
    embedded_of(&self.inner)
  }

  /// The document's pictures, as their bytes.
  #[napi]
  pub fn pictures(&self) -> Vec<RtfPicture> {
    pictures_of(&self.inner)
  }

  /// What to know of the reading: an attachment whose place is marked in the text and that
  /// is not in the file.
  #[napi]
  pub fn warnings(&self) -> Vec<String> {
    self.inner.warnings().to_vec()
  }

  /// Everything read from the document, in one object.
  #[napi]
  pub fn document(&self, opts: Option<InvoiceOptions>) -> RtfDocument {
    let options = opts.map(Into::into);
    Whole::of(Arc::clone(&self.inner), options.as_ref()).into()
  }

  /// `document` on a worker thread.
  #[napi]
  pub fn document_async(&self, opts: Option<InvoiceOptions>) -> AsyncTask<DocumentTask> {
    AsyncTask::new(DocumentTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
      options: opts.map(Into::into),
    })
  }
}
