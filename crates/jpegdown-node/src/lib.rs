#![deny(clippy::all)]

//! `@d0paminedriven/jpegdown`: a JPEG that is a page, for Node.
//!
//! This crate is the Node-API surface: this file holds the functions, their promise-returning
//! twins, and the class, and `types.rs` holds the shapes JavaScript sees. The work itself is
//! done by the `jpegdown` crate, which knows nothing about Node.
//!
//! With the `ocr` feature, `ocr.rs` adds the verbs that read the page's words, and
//! `invoice.rs` the invoice's shapes; that build is the superset package,
//! `@d0paminedriven/jpegdown-ocr`, which needs Tesseract on the machine.
//!
//! Every `*Async` function and method returns an [`AsyncTask`]: the input is copied on the JS
//! thread, the work runs on libuv's pool, and the result is turned into JS values back on the
//! JS thread when the promise resolves. The bytes are copied because a caller may go on to
//! change its buffer while the work runs.

use std::sync::Arc;

use jpegdown::{Jpeg, JpegError};
use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

#[cfg(feature = "ocr")]
pub mod invoice;
#[cfg(feature = "ocr")]
pub mod ocr;
mod types;

pub use types::{
  JpegCoding, JpegColor, JpegDocument, JpegPage, JpegPageSize, JpegResolution, JpegSize, PaperSize,
  StandardPaperSize,
};
use types::{document_of, page_of};

// ── What a caller passes ──────────────────────────────────────────

fn js_error(error: JpegError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: Vec<u8>) -> Result<Jpeg> {
  Jpeg::open(bytes).map_err(js_error)
}

/// The file a task reads: bytes still to be parsed, or a file a `JpegDown` already holds.
pub enum Held {
  Bytes(Vec<u8>),
  Parsed(Arc<Jpeg>),
}

impl Held {
  fn jpeg(&mut self) -> Result<Arc<Jpeg>> {
    match self {
      Held::Bytes(bytes) => open(std::mem::take(bytes)).map(Arc::new),
      Held::Parsed(jpeg) => Ok(Arc::clone(jpeg)),
    }
  }
}

// ── Tasks ─────────────────────────────────────────────────────────

/// Parses a buffer on a worker thread, for `JpegDown.openAsync`.
pub struct OpenTask {
  bytes: Vec<u8>,
}

#[napi]
impl Task for OpenTask {
  type Output = Jpeg;
  type JsValue = JpegDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(JpegDown {
      inner: Arc::new(output),
    })
  }
}

/// Reads a file's header on a worker thread, for `jpegPageAsync` and `jpegDocumentAsync`.
pub struct FactsTask {
  held: Held,
}

/// What `FactsTask` gives when it is asked for the page alone.
pub struct PageTask(FactsTask);

#[napi]
impl Task for PageTask {
  type Output = Arc<Jpeg>;
  type JsValue = JpegPage;

  fn compute(&mut self) -> Result<Self::Output> {
    self.0.held.jpeg()
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(page_of(&output))
  }
}

#[napi]
impl Task for FactsTask {
  type Output = Arc<Jpeg>;
  type JsValue = JpegDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    self.held.jpeg()
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(document_of(&output))
  }
}

/// Decodes the picture and writes it as a PNG on a worker thread, for `jpegPngAsync` and
/// `pngAsync`.
pub struct PngTask {
  held: Held,
}

#[napi]
impl Task for PngTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    self.held.jpeg()?.png().map_err(js_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// ── Functions ─────────────────────────────────────────────────────

/// What a JPEG buffer says of its page: its size, its resolution, how it is kept, the paper it
/// is nearest to, and the size it is read at. Nothing of the picture is decoded. Throws
/// `unsupported format: …` for bytes that are not a JPEG and `malformed file: …` for a header
/// that is damaged.
#[napi]
pub fn jpeg_page(buffer: Buffer) -> Result<JpegPage> {
  Ok(page_of(&open(buffer.to_vec())?))
}

/// `jpegPage` on a worker thread: the buffer is copied, then read there.
#[napi]
pub fn jpeg_page_async(buffer: Buffer) -> AsyncTask<PageTask> {
  AsyncTask::new(PageTask(FactsTask {
    held: Held::Bytes(buffer.to_vec()),
  }))
}

/// The picture of a JPEG buffer as a PNG, as it is to be looked at: turned the way its
/// orientation says. Throws `unsupported format: …` for a picture kept in a coding that is not
/// read (without loss, or with samples of more than 8 bits), `limit exceeded: …` for one
/// larger than any page, and `malformed file: …` for one that cannot be decoded.
#[napi]
pub fn jpeg_png(buffer: Buffer) -> Result<Buffer> {
  let png = open(buffer.to_vec())?.png().map_err(js_error)?;
  Ok(png.into())
}

/// `jpegPng` on a worker thread: the buffer is copied, then decoded there.
#[napi]
pub fn jpeg_png_async(buffer: Buffer) -> AsyncTask<PngTask> {
  AsyncTask::new(PngTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

/// What is known of a JPEG buffer, in one object: its page and its size in bytes.
#[napi]
pub fn jpeg_document(buffer: Buffer) -> Result<JpegDocument> {
  Ok(document_of(&open(buffer.to_vec())?))
}

/// `jpegDocument` on a worker thread: the buffer is copied, then read there.
#[napi]
pub fn jpeg_document_async(buffer: Buffer) -> AsyncTask<FactsTask> {
  AsyncTask::new(FactsTask {
    held: Held::Bytes(buffer.to_vec()),
  })
}

// ── The class ─────────────────────────────────────────────────────

/// A JPEG file. Its header is read once, in the constructor or in `openAsync`; its picture is
/// decoded when it is asked for.
#[napi]
pub struct JpegDown {
  inner: Arc<Jpeg>,
}

#[napi]
impl JpegDown {
  /// Read a file's header from its bytes, which are copied. Throws `unsupported format: …`
  /// for bytes that are not a JPEG and `malformed file: …` for a header that is damaged.
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(JpegDown {
      inner: Arc::new(open(buffer.to_vec())?),
    })
  }

  /// Read a file's header off the JavaScript thread; resolves to a `JpegDown`, or rejects with
  /// the errors the constructor throws.
  #[napi]
  pub fn open_async(buffer: Buffer) -> AsyncTask<OpenTask> {
    AsyncTask::new(OpenTask {
      bytes: buffer.to_vec(),
    })
  }

  /// What the file says of its page.
  #[napi]
  pub fn page(&self) -> JpegPage {
    page_of(&self.inner)
  }

  /// The picture as a PNG, as it is to be looked at: turned the way its orientation says.
  /// Throws as `jpegPng` does.
  #[napi]
  pub fn png(&self) -> Result<Buffer> {
    let png = self.inner.png().map_err(js_error)?;
    Ok(png.into())
  }

  /// `png` on a worker thread.
  #[napi]
  pub fn png_async(&self) -> AsyncTask<PngTask> {
    AsyncTask::new(PngTask {
      held: Held::Parsed(Arc::clone(&self.inner)),
    })
  }

  /// What is known of the file, in one object: its page and its size in bytes.
  #[napi]
  pub fn document(&self) -> JpegDocument {
    document_of(&self.inner)
  }
}
