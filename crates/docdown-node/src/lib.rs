#![deny(clippy::all)]

//! `@d0paminedriven/docdown`: a Word document of 1997 to 2003, for Node.
//!
//! This crate is the Node-API surface: this file holds the functions, their promise-returning
//! twins, and the class; `types.rs`, `layout.rs`, and `invoice.rs` hold the shapes JavaScript
//! sees. The work itself is done by the `docdown` crate, which knows nothing about Node.
//!
//! Every `*Async` function and method returns an [`AsyncTask`]: the input is copied on the JS
//! thread, the work runs on libuv's pool, and the result is turned into JS values back on the
//! JS thread when the promise resolves. The bytes are copied because a caller may go on to
//! change its buffer while the work runs.

use std::sync::Arc;

use contentdown::PageLayout;
use docdown::{Doc, DocError};
use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

pub mod invoice;
mod layout;
mod types;

use invoice::{InvoiceOptions, InvoiceResult};
use layout::pages_of;
pub use layout::{DocLine, DocPage, DocWord};
pub use types::{DocDocument, DocEmbedded, DocPicture, DocPictureFormat};
use types::{embedded_of, pictures_of};

// ── What a caller passes ──────────────────────────────────────────

fn js_error(error: DocError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: &[u8]) -> Result<Doc> {
  Doc::open(bytes).map_err(js_error)
}

/// The document a task reads: bytes still to be parsed, or one an `DocDown` already holds.
pub enum Held {
  Bytes(Vec<u8>),
  Parsed(Arc<Doc>),
}

impl Held {
  fn doc(&mut self) -> Result<Arc<Doc>> {
    match self {
      Held::Bytes(bytes) => open(&std::mem::take(bytes)).map(Arc::new),
      Held::Parsed(doc) => Ok(Arc::clone(doc)),
    }
  }
}

/// Everything read from a document, gathered on the thread that does the reading.
pub struct Whole {
  doc: Arc<Doc>,
  pages: Vec<PageLayout>,
  invoice: contentdown::invoice::InvoiceResult,
}

impl Whole {
  fn of(doc: Arc<Doc>, options: Option<&contentdown::invoice::InvoiceOptions>) -> Self {
    let pages = doc.layout();
    let invoice = doc.invoice(options);
    Whole {
      doc,
      pages,
      invoice,
    }
  }
}

impl From<Whole> for DocDocument {
  fn from(whole: Whole) -> Self {
    DocDocument {
      text: whole.doc.text(),
      pages: pages_of(whole.pages),
      embedded: embedded_of(&whole.doc),
      pictures: pictures_of(&whole.doc),
      warnings: whole.doc.warnings().to_vec(),
      is_template: whole.doc.is_template(),
      has_macros: whole.doc.has_macros(),
      invoice: whole.invoice.into(),
    }
  }
}

// ── Tasks ─────────────────────────────────────────────────────────

/// Parses a buffer on a worker thread, for `DocDown.openAsync`.
pub struct OpenTask {
  bytes: Vec<u8>,
}

#[napi]
impl Task for OpenTask {
  type Output = Doc;
  type JsValue = DocDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(&std::mem::take(&mut self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(DocDown {
      inner: Arc::new(output),
    })
  }
}

/// Reads a document's text on a worker thread, for `docTextAsync`.
pub struct TextTask {
  held: Held,
}

#[napi]
impl Task for TextTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.held.doc()?.text())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Sets a document on pages on a worker thread, for `docLayoutAsync` and `layoutAsync`.
pub struct LayoutTask {
  held: Held,
}

#[napi]
impl Task for LayoutTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<DocPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.held.doc()?.layout())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(pages_of(output))
  }
}

/// Reads a document's invoice fields on a worker thread, for `docInvoiceAsync` and
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
    Ok(self.held.doc()?.invoice(self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Parses a buffer on a worker thread and gives what it carries, for `docEmbeddedAsync`.
pub struct EmbeddedTask {
  held: Held,
}

#[napi]
impl Task for EmbeddedTask {
  type Output = Arc<Doc>;
  type JsValue = Vec<DocEmbedded>;

  fn compute(&mut self) -> Result<Self::Output> {
    self.held.doc()
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(embedded_of(&output))
  }
}

/// Parses a buffer on a worker thread and gives its pictures, for `docPicturesAsync`.
pub struct PicturesTask {
  held: Held,
}

#[napi]
impl Task for PicturesTask {
  type Output = Arc<Doc>;
  type JsValue = Vec<DocPicture>;

  fn compute(&mut self) -> Result<Self::Output> {
    self.held.doc()
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(pictures_of(&output))
  }
}

/// Reads everything on a worker thread, for `docDocumentAsync` and `documentAsync`.
pub struct DocumentTask {
  held: Held,
  options: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for DocumentTask {
  type Output = Whole;
  type JsValue = DocDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(Whole::of(self.held.doc()?, self.options.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// ── Functions ─────────────────────────────────────────────────────

/// The text of a `.doc` buffer: a line to a paragraph, and a line to a table's row with a tab
/// between its cells. A header and a footer are not in it: they are on the pages `docLayout`
/// gives, and the text of a text box is where its shape is anchored. Throws
/// `unsupported format: …` for a document of Word 6 or 95 and for one saved with a password,
/// and `malformed document: …` for bytes that are no Word document.
#[napi]
pub fn doc_text(buffer: Buffer) -> Result<String> {
  Ok(open(&buffer)?.text())
}

/// `docText` on a worker thread: the buffer is copied, then parsed there.
#[napi]
pub fn doc_text_async(buffer: Buffer) -> AsyncTask<TextTask> {
  AsyncTask::new(TextTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// A `.doc` buffer set on pages of positioned words, with the file's header and footer on each.
#[napi]
pub fn doc_layout(buffer: Buffer) -> Result<Vec<DocPage>> {
  Ok(pages_of(open(&buffer)?.layout()))
}

/// `docLayout` on a worker thread: the buffer is copied, then parsed and set there.
#[napi]
pub fn doc_layout_async(buffer: Buffer) -> AsyncTask<LayoutTask> {
  AsyncTask::new(LayoutTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// Invoice fields read from a `.doc` buffer's pages as `docLayout` sets them. When the document
/// carries embedded files, the first of the warnings says so: they are not read here, and the
/// invoice may be one of them.
#[napi]
pub fn doc_invoice(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
  let doc = open(&buffer)?;
  Ok(doc.invoice(opts.map(Into::into).as_ref()).into())
}

/// `docInvoice` on a worker thread.
#[napi]
pub fn doc_invoice_async(buffer: Buffer, opts: Option<InvoiceOptions>) -> AsyncTask<InvoiceTask> {
  AsyncTask::new(InvoiceTask {
    held: Held::Bytes(buffer.to_vec()),
    options: opts.map(Into::into),
  })
}

/// The files embedded in a `.doc` buffer, each as it was before it was embedded: a file dropped
/// into the document, a workbook, another document, a PDF.
#[napi]
pub fn doc_embedded(buffer: Buffer) -> Result<Vec<DocEmbedded>> {
  Ok(embedded_of(&open(&buffer)?))
}

/// `docEmbedded` on a worker thread.
#[napi]
pub fn doc_embedded_async(buffer: Buffer) -> AsyncTask<EmbeddedTask> {
  AsyncTask::new(EmbeddedTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// The pictures of a `.doc` buffer, as their bytes. The picture that stands for an embedded
/// object is not among them.
#[napi]
pub fn doc_pictures(buffer: Buffer) -> Result<Vec<DocPicture>> {
  Ok(pictures_of(&open(&buffer)?))
}

/// `docPictures` on a worker thread.
#[napi]
pub fn doc_pictures_async(buffer: Buffer) -> AsyncTask<PicturesTask> {
  AsyncTask::new(PicturesTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// Everything read from a `.doc` buffer, in one object: its text, its pages, what it carries,
/// its pictures, and its invoice fields.
#[napi]
pub fn doc_document(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<DocDocument> {
  let options = opts.map(Into::into);
  Ok(Whole::of(Arc::new(open(&buffer)?), options.as_ref()).into())
}

/// `docDocument` on a worker thread.
#[napi]
pub fn doc_document_async(buffer: Buffer, opts: Option<InvoiceOptions>) -> AsyncTask<DocumentTask> {
  AsyncTask::new(DocumentTask {
    held: Held::Bytes(buffer.to_vec()),
    options: opts.map(Into::into),
  })
}

// ── The class ─────────────────────────────────────────────────────

/// A parsed Word document. Parsing happens once, in the constructor or in `openAsync`; every
/// method reads from the parsed document.
#[napi]
pub struct DocDown {
  inner: Arc<Doc>,
}

#[napi]
impl DocDown {
  /// Parse a document from the bytes of a `.doc` file, as Word wrote them from 1997 to 2003.
  /// Throws `unsupported format: …` for a document of Word 6 or 95 and for one saved with a
  /// password, `limit exceeded: …` for one past the size limits, and `malformed document: …`
  /// for anything else that cannot be read.
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(DocDown {
      inner: Arc::new(open(&buffer)?),
    })
  }

  /// Parse a document off the JavaScript thread; resolves to an `DocDown`, or rejects with
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
  pub fn layout(&self) -> Vec<DocPage> {
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
  pub fn embedded(&self) -> Vec<DocEmbedded> {
    embedded_of(&self.inner)
  }

  /// The document's pictures, as their bytes.
  #[napi]
  pub fn pictures(&self) -> Vec<DocPicture> {
    pictures_of(&self.inner)
  }

  /// What to know of the reading: a part of the text that could not be found in the file.
  #[napi]
  pub fn warnings(&self) -> Vec<String> {
    self.inner.warnings().to_vec()
  }

  /// Everything read from the document, in one object.
  #[napi]
  pub fn document(&self, opts: Option<InvoiceOptions>) -> DocDocument {
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
