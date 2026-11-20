#![deny(clippy::all)]

//! `@d0paminedriven/tiffdown`: a TIFF file's pages, for Node.
//!
//! This crate is the Node-API surface: this file holds the functions, their promise-returning
//! twins, and the class, and `types.rs` holds the shapes JavaScript sees. The work itself is
//! done by the `tiffdown` crate, which knows nothing about Node.
//!
//! With the `ocr` feature, `ocr.rs` adds the verbs that read a page's words, and `invoice.rs`
//! the invoice's shapes; that build is the superset package, `@d0paminedriven/tiffdown-ocr`,
//! which needs Tesseract on the machine.
//!
//! Every `*Async` function and method returns an [`AsyncTask`]: the input is copied on the JS
//! thread, the work runs on libuv's pool, and the result is turned into JS values back on the
//! JS thread when the promise resolves. The bytes are copied because a caller may go on to
//! change its buffer while the work runs.

use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;
use tiffdown::{Tiff, TiffError};

#[cfg(feature = "ocr")]
pub mod invoice;
#[cfg(feature = "ocr")]
pub mod ocr;
mod types;

pub use types::{
  PaperSize, StandardPaperSize, TiffCompression, TiffDocument, TiffPage, TiffPhotometric,
  TiffResolution, TiffSize,
};
use types::{document_of, page_count_of, pages_of};

// ── What a caller passes ──────────────────────────────────────────

fn js_error(error: TiffError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: Vec<u8>) -> Result<Tiff> {
  Tiff::open(bytes).map_err(js_error)
}

/// A page's number as JavaScript passes it. A fraction, `NaN`, and `Infinity` are refused and
/// not quietly cut down to a page that was not asked for.
fn page_number(page: f64) -> Result<u32> {
  let fits = page.is_finite() && page.fract() == 0.0 && (1.0..=f64::from(u32::MAX)).contains(&page);
  if !fits {
    return Err(Error::new(
      Status::InvalidArg,
      "page must be a whole number from 1",
    ));
  }
  // Checked above to be a whole number a `u32` holds, so the conversion keeps its value.
  Ok(page as u32)
}

// ── Tasks that parse a buffer ─────────────────────────────────────

/// Parses a buffer on a worker thread, for `TiffDown.openAsync`.
pub struct OpenTask {
  bytes: Vec<u8>,
}

#[napi]
impl Task for OpenTask {
  type Output = Tiff;
  type JsValue = TiffDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(TiffDown {
      inner: Arc::new(output),
    })
  }
}

/// Parses a buffer and lists its pages, for `tiffPagesAsync`.
pub struct PagesTask {
  bytes: Vec<u8>,
}

#[napi]
impl Task for PagesTask {
  type Output = Tiff;
  type JsValue = Vec<TiffPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(pages_of(&output))
  }
}

/// Parses a buffer and writes one of its pages as a PNG, for `tiffPagePngAsync`.
pub struct PagePngTask {
  bytes: Vec<u8>,
  page: u32,
}

#[napi]
impl Task for PagePngTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes))?
      .png(self.page)
      .map_err(js_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Parses a buffer and gathers what is known of it, for `tiffDocumentAsync`.
pub struct DocumentTask {
  bytes: Vec<u8>,
}

#[napi]
impl Task for DocumentTask {
  type Output = Tiff;
  type JsValue = TiffDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(document_of(&output))
  }
}

// ── Tasks that read a file a `TiffDown` holds ─────────────────────

/// Writes one page of the file a `TiffDown` holds as a PNG, for its `pagePngAsync`.
pub struct SharedPagePngTask {
  inner: Arc<Tiff>,
  page: u32,
}

#[napi]
impl Task for SharedPagePngTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    self.inner.png(self.page).map_err(js_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Gathers what is known of the file a `TiffDown` holds, for its `documentAsync`.
pub struct SharedDocumentTask {
  inner: Arc<Tiff>,
}

#[napi]
impl Task for SharedDocumentTask {
  type Output = ();
  type JsValue = TiffDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(())
  }

  fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
    Ok(document_of(&self.inner))
  }
}

// ── Functions ─────────────────────────────────────────────────────

/// The pages of a TIFF buffer, in order, each with what the file says of it: its size, its
/// resolution, how its pixels are kept, and the paper it is nearest to. No pixels are read.
/// Throws `unsupported format: …` for bytes that are not a TIFF.
#[napi]
pub fn tiff_pages(buffer: Buffer) -> Result<Vec<TiffPage>> {
  Ok(pages_of(&open(buffer.to_vec())?))
}

/// `tiffPages` on a worker thread: the buffer is copied, then parsed there.
#[napi]
pub fn tiff_pages_async(buffer: Buffer) -> AsyncTask<PagesTask> {
  AsyncTask::new(PagesTask {
    bytes: buffer.to_vec(),
  })
}

/// One page of a TIFF buffer as a PNG, as the page is to be looked at: turned the way its
/// orientation says, and with square pixels. `page` counts from 1. Throws `limit exceeded: …`
/// for a page the file does not have, `unsupported format: …` for a page kept in a coding
/// that is not read, and `malformed file: …` for a damaged one.
#[napi]
pub fn tiff_page_png(buffer: Buffer, page: f64) -> Result<Buffer> {
  let page = page_number(page)?;
  let png = open(buffer.to_vec())?.png(page).map_err(js_error)?;
  Ok(png.into())
}

/// `tiffPagePng` on a worker thread: the buffer is copied, then parsed and decoded there. A
/// page number that is not a whole number from 1 throws before any work is handed over.
#[napi]
pub fn tiff_page_png_async(buffer: Buffer, page: f64) -> Result<AsyncTask<PagePngTask>> {
  Ok(AsyncTask::new(PagePngTask {
    bytes: buffer.to_vec(),
    page: page_number(page)?,
  }))
}

/// What is known of a TIFF buffer, in one object: its pages, its size in bytes, and what was
/// wrong with it where it could still be read.
#[napi]
pub fn tiff_document(buffer: Buffer) -> Result<TiffDocument> {
  Ok(document_of(&open(buffer.to_vec())?))
}

/// `tiffDocument` on a worker thread: the buffer is copied, then parsed there.
#[napi]
pub fn tiff_document_async(buffer: Buffer) -> AsyncTask<DocumentTask> {
  AsyncTask::new(DocumentTask {
    bytes: buffer.to_vec(),
  })
}

// ── The class ─────────────────────────────────────────────────────

/// A parsed TIFF file. Its directories are read once, in the constructor or in `openAsync`;
/// a page's pixels are decoded when they are asked for, one page at a time.
#[napi]
pub struct TiffDown {
  inner: Arc<Tiff>,
}

#[napi]
impl TiffDown {
  /// Parse a file from its bytes, which are copied. Throws `unsupported format: …` for bytes
  /// that are not a TIFF or are a BigTIFF, and `malformed file: …` for a file whose first
  /// directory cannot be found.
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(TiffDown {
      inner: Arc::new(open(buffer.to_vec())?),
    })
  }

  /// Parse a file off the JavaScript thread; resolves to a `TiffDown`, or rejects with the
  /// errors the constructor throws.
  #[napi]
  pub fn open_async(buffer: Buffer) -> AsyncTask<OpenTask> {
    AsyncTask::new(OpenTask {
      bytes: buffer.to_vec(),
    })
  }

  /// How many pages the file has.
  #[napi]
  pub fn page_count(&self) -> u32 {
    page_count_of(&self.inner)
  }

  /// The file's pages, in order, each with what the file says of it.
  #[napi]
  pub fn pages(&self) -> Vec<TiffPage> {
    pages_of(&self.inner)
  }

  /// What was wrong with the file, where it could still be read.
  #[napi]
  pub fn warnings(&self) -> Vec<String> {
    self.inner.warnings().to_vec()
  }

  /// One page as a PNG, as the page is to be looked at: turned the way its orientation says,
  /// and with square pixels. `page` counts from 1. Throws as `tiffPagePng` does.
  #[napi]
  pub fn page_png(&self, page: f64) -> Result<Buffer> {
    let png = self.inner.png(page_number(page)?).map_err(js_error)?;
    Ok(png.into())
  }

  /// `pagePng` on a worker thread. A page number that is not a whole number from 1 throws
  /// before any work is handed over.
  #[napi]
  pub fn page_png_async(&self, page: f64) -> Result<AsyncTask<SharedPagePngTask>> {
    Ok(AsyncTask::new(SharedPagePngTask {
      inner: Arc::clone(&self.inner),
      page: page_number(page)?,
    }))
  }

  /// What is known of the file, in one object: its pages, its size in bytes, and what was
  /// wrong with it.
  #[napi]
  pub fn document(&self) -> TiffDocument {
    document_of(&self.inner)
  }

  /// `document` as a promise.
  #[napi]
  pub fn document_async(&self) -> AsyncTask<SharedDocumentTask> {
    AsyncTask::new(SharedDocumentTask {
      inner: Arc::clone(&self.inner),
    })
  }
}
