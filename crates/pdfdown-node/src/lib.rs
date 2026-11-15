#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

use pdfdown::lopdf::{self, Document};
use std::sync::Arc;

mod forms;
// Public so its enums keep the spellings JavaScript sees (`UBL`, `DMY`) without a lint exception.
pub mod invoice;
mod layout;
mod readers;
mod types;

// Public API types (appear in generated .d.ts)
pub use forms::{ChoiceOption, FormInfo, PageFormField, UnplacedFormField};
pub use invoice::{InvoiceOptions, InvoiceResult};
pub use layout::{PageLayout, PlacedImage, TextLine, TextSource, TextWord};
pub use types::{
  Attachment, BoxType, Capabilities, PageAnnotation, PageBox, PageImage, PageText, PdfDocument,
  PdfMeta, StructuredPageText,
};

#[cfg(feature = "ocr")]
pub use types::{OcrOptions, OcrPageText, OcrStructuredPageText, PdfDocumentOcr};

#[cfg(feature = "render")]
pub use types::{RenderMode, RenderOptions, RenderedPage};

// ── Step 1: Compile-time assertion that lopdf::Document is Send + Sync ──
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  // Referenced rather than called: monomorphizing the fn checks the bound at compile time
  // without leaving an uncalled function behind for dead-code analysis.
  let _ = assert_send_sync::<lopdf::Document>;
};

// ── Shared helpers ──────────────────────────────────────────────

use crate::readers::{
  extract_all, extract_annotations, extract_attachments, extract_forms, extract_from_pages,
  extract_images_raw, extract_invoice_impl, extract_layout, extract_metadata,
  extract_structured_text, extract_text,
};

#[cfg(feature = "ocr")]
use crate::readers::{extract_all_with_ocr, extract_layout_with_ocr, extract_text_with_ocr};
#[cfg(feature = "ocr")]
use pdfdown_ocr::normalize_max_threads;

fn load_doc(buf: &[u8]) -> Result<Document> {
  Document::load_mem(buf).map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))
}

/// Extract render mode (u8) from OcrOptions when render feature is enabled.
#[cfg(all(feature = "ocr", feature = "render"))]
fn extract_render_mode(opts: &Option<OcrOptions>) -> u8 {
  use pdfdown::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};
  opts
    .as_ref()
    .and_then(|o| o.render.as_ref())
    .map(|m| match m {
      RenderMode::Auto => RENDER_MODE_AUTO,
      RenderMode::Never => RENDER_MODE_NEVER,
      RenderMode::Always => RENDER_MODE_ALWAYS,
    })
    .unwrap_or(RENDER_MODE_AUTO)
}

/// Extract render DPI from OcrOptions when render feature is enabled.
#[cfg(all(feature = "ocr", feature = "render"))]
fn extract_render_dpi(opts: &Option<OcrOptions>) -> u32 {
  pdfdown::render::normalize_dpi(opts.as_ref().and_then(|o| o.render_dpi))
}

/// Extract render mode as u8 — when render feature is off, always return Auto (0).
#[cfg(all(feature = "ocr", not(feature = "render")))]
fn extract_render_mode(_opts: &Option<OcrOptions>) -> u8 {
  pdfdown_ocr::RENDER_MODE_AUTO
}

/// Extract render DPI — when render feature is off, return default 300.
#[cfg(all(feature = "ocr", not(feature = "render")))]
fn extract_render_dpi(_opts: &Option<OcrOptions>) -> u32 {
  300
}

/// Initialize PDFium from pdfium_path in OcrOptions (if present).
#[cfg(all(feature = "ocr", feature = "render"))]
fn maybe_init_pdfium(opts: &Option<OcrOptions>) {
  let path = opts.as_ref().and_then(|o| o.pdfium_path.as_deref());
  let _ = pdfdown::render::ensure_pdfium_with_path(path);
}

#[cfg(all(feature = "ocr", not(feature = "render")))]
fn maybe_init_pdfium(_opts: &Option<OcrOptions>) {}

/// Resolved OCR settings shared by the positioned-text entry points.
#[cfg(feature = "ocr")]
struct OcrParams {
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
fn ocr_params(opts: &Option<OcrOptions>) -> OcrParams {
  maybe_init_pdfium(opts);
  OcrParams {
    lang: opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string()),
    min_len: opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1),
    max_threads: normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads)),
    render_dpi: extract_render_dpi(opts),
    render_mode: extract_render_mode(opts),
  }
}

// ── Standalone sync functions ───────────────────────────────────

#[napi]
pub fn extract_text_per_page(buffer: Buffer) -> Result<Vec<PageText>> {
  let doc = load_doc(buffer.as_ref())?;
  extract_text(&doc)
}

#[napi]
pub fn pdf_metadata(buffer: Buffer) -> Result<PdfMeta> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_metadata(&doc))
}

#[napi]
pub fn extract_annotations_per_page(buffer: Buffer) -> Result<Vec<PageAnnotation>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_annotations(&doc))
}

/// Positioned words and lines per page from the content stream (native text only).
/// Coordinates are PDF user space in points, y up.
#[napi]
pub fn layout_per_page(buffer: Buffer) -> Result<Vec<PageLayout>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_layout(&doc))
}

/// Every embedded file: `/Names /EmbeddedFiles`, catalog `/AF`, and page `/FileAttachment` annotations.
#[napi]
pub fn attachments(buffer: Buffer) -> Result<Vec<Attachment>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(
    extract_attachments(&doc)
      .into_iter()
      .map(Attachment::from)
      .collect(),
  )
}

/// Structured invoice data. This version reads embedded Factur-X / ZUGFeRD /
/// XRechnung / UBL XML; documents without it return `source: "None"`.
#[napi]
pub fn extract_invoice(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
  let doc = load_doc(buffer.as_ref())?;
  let pages = pdf_invoice_pages(&doc, buffer.as_ref());
  Ok(extract_invoice_impl(&doc, opts.as_ref(), &pages))
}

/// The pages `invoice()` reads for a PDF: every page's native layout, and in the OCR build the
/// cascade with default options on the first and the last page only, the two pages the header
/// track reads, so a long scan does not cost a whole-document OCR here.
#[cfg(feature = "ocr")]
fn pdf_invoice_pages(doc: &Document, raw: &[u8]) -> Vec<contentdown::PageLayout> {
  let last = doc.get_pages().len() as u32;
  let p = ocr_params(&None);
  pdfdown_ocr::extract_layout_with_ocr(
    doc,
    raw,
    &p.lang,
    p.min_len,
    p.max_threads,
    p.render_dpi,
    p.render_mode,
    Some(&[1, last]),
  )
}

#[cfg(not(feature = "ocr"))]
fn pdf_invoice_pages(doc: &Document, _raw: &[u8]) -> Vec<contentdown::PageLayout> {
  pdfdown::layout::extract_layout(doc)
}

/// The layout track over pages from anywhere: a PDF's own layout, `layoutWithOcr`, or pages
/// the caller built.
#[napi]
pub fn invoice_from_layout(pages: Vec<PageLayout>, opts: Option<InvoiceOptions>) -> InvoiceResult {
  extract_from_pages(&pages, opts.as_ref())
}

pub struct InvoiceFromLayoutTask {
  pages: Vec<PageLayout>,
  opts: Option<InvoiceOptions>,
}

#[napi]
impl Task for InvoiceFromLayoutTask {
  type Output = InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_from_pages(&self.pages, self.opts.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn invoice_from_layout_async(
  pages: Vec<PageLayout>,
  opts: Option<InvoiceOptions>,
) -> AsyncTask<InvoiceFromLayoutTask> {
  AsyncTask::new(InvoiceFromLayoutTask { pages, opts })
}

#[napi]
pub fn extract_images_per_page(buffer: Buffer) -> Result<Vec<PageImage>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(
    extract_images_raw(&doc)
      .into_iter()
      .map(PageImage::from)
      .collect(),
  )
}

#[napi]
pub fn pdf_document(buffer: Buffer) -> Result<PdfDocument> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(PdfDocument::from(extract_all(&doc)?))
}

#[napi]
pub fn extract_structured_text_per_page(buffer: Buffer) -> Result<Vec<StructuredPageText>> {
  let doc = load_doc(buffer.as_ref())?;
  extract_structured_text(&doc)
}

#[napi]
pub fn extract_form_fields_per_page(buffer: Buffer) -> Result<Vec<PageFormField>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_forms(&doc).fields)
}

#[cfg(feature = "ocr")]
#[napi]
pub fn extract_text_with_ocr_per_page(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> Result<Vec<OcrPageText>> {
  maybe_init_pdfium(&opts);
  let doc = load_doc(buffer.as_ref())?;
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.as_deref())
    .unwrap_or("eng");
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  extract_text_with_ocr(
    &doc,
    buffer.as_ref(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  )
}

/// Positioned words and lines per page with the OCR cascade: native glyph
/// placement, then OCR of embedded images mapped through their placements,
/// then (with PDFium) OCR of the rasterized page.
#[cfg(feature = "ocr")]
#[napi]
pub fn extract_layout_with_ocr_per_page(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> Result<Vec<PageLayout>> {
  let p = ocr_params(&opts);
  let doc = load_doc(buffer.as_ref())?;
  extract_layout_with_ocr(
    &doc,
    buffer.as_ref(),
    &p.lang,
    p.min_len,
    p.max_threads,
    p.render_dpi,
    p.render_mode,
    None,
  )
}

#[cfg(feature = "ocr")]
#[napi]
pub fn pdf_document_ocr(buffer: Buffer, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
  maybe_init_pdfium(&opts);
  let doc = load_doc(buffer.as_ref())?;
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.as_deref())
    .unwrap_or("eng");
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  Ok(PdfDocumentOcr::from(extract_all_with_ocr(
    &doc,
    buffer.as_ref(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  )?))
}

// ── Standalone render functions ─────────────────────────────────

#[cfg(feature = "render")]
pub struct RenderPagesTask {
  data: Vec<u8>,
  dpi: u32,
  mode: u8,
}

#[cfg(feature = "render")]
#[napi]
impl Task for RenderPagesTask {
  type Output = Vec<pdfdown::RenderedPage>;
  type JsValue = Vec<RenderedPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    use pdfdown::render::{RENDER_MODE_AUTO, RENDER_MODE_NEVER};

    let dpi = self.dpi;
    let mode = self.mode;
    let pdf_bytes = &self.data;

    if mode == RENDER_MODE_NEVER {
      return Ok(Vec::new());
    }

    pdfdown::render::ensure_pdfium_with_path(None)
      .map_err(|e| Error::from_reason(e.to_string()))?;

    let doc = load_doc(pdf_bytes)?;
    let pages = doc.get_pages();
    let page_count = pages.len() as u16;

    let indices: Vec<u16> = if mode == RENDER_MODE_AUTO {
      pages
        .iter()
        .filter_map(|(&page_num, &page_id)| {
          let raw = doc.extract_text(&[page_num]).unwrap_or_default();
          let non_ws: usize = raw.chars().filter(|c| !c.is_whitespace()).count();
          if non_ws > 0 {
            return None;
          }
          let has_images = !pdfdown::images::collect_page_decoded_images(&doc, page_id).is_empty();
          if has_images {
            return None;
          }
          Some((page_num - 1) as u16)
        })
        .collect()
    } else {
      (0..page_count).collect()
    };

    let rendered = pdfdown::render::render_pages_to_png(pdf_bytes, &indices, dpi);
    Ok(
      rendered
        .into_iter()
        .map(|(idx, w, h, data)| pdfdown::RenderedPage {
          page: (idx as u32) + 1,
          width: w,
          height: h,
          dpi,
          data,
        })
        .collect(),
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(RenderedPage::from).collect())
  }
}

#[cfg(feature = "render")]
#[napi]
pub fn render_pages_async(
  buffer: Buffer,
  opts: Option<RenderOptions>,
) -> AsyncTask<RenderPagesTask> {
  use pdfdown::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};
  let dpi = pdfdown::render::normalize_dpi(opts.as_ref().and_then(|o| o.dpi));
  let mode = opts
    .as_ref()
    .and_then(|o| o.mode.as_ref())
    .map(|m| match m {
      RenderMode::Auto => RENDER_MODE_AUTO,
      RenderMode::Never => RENDER_MODE_NEVER,
      RenderMode::Always => RENDER_MODE_ALWAYS,
    })
    .unwrap_or(RENDER_MODE_ALWAYS);
  AsyncTask::new(RenderPagesTask {
    data: buffer.to_vec(),
    dpi,
    mode,
  })
}

// ── Capabilities check ──────────────────────────────────────────

#[napi]
pub fn capabilities() -> Capabilities {
  Capabilities {
    ocr: cfg!(feature = "ocr"),
    #[cfg(feature = "render")]
    render: pdfdown::render::is_pdfium_available(),
    #[cfg(not(feature = "render"))]
    render: false,
  }
}

// ── Standalone async functions (libuv thread pool via AsyncTask) ─

pub struct ExtractTextTask(Vec<u8>);

#[napi]
impl Task for ExtractTextTask {
  type Output = Vec<PageText>;
  type JsValue = Vec<PageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    extract_text(&doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_text_per_page_async(buffer: Buffer) -> AsyncTask<ExtractTextTask> {
  AsyncTask::new(ExtractTextTask(buffer.to_vec()))
}

pub struct ExtractImagesTask(Vec<u8>);

#[napi]
impl Task for ExtractImagesTask {
  type Output = Vec<pdfdown::PageImage>;
  type JsValue = Vec<PageImage>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_images_raw(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(PageImage::from).collect())
  }
}

#[napi]
pub fn extract_images_per_page_async(buffer: Buffer) -> AsyncTask<ExtractImagesTask> {
  AsyncTask::new(ExtractImagesTask(buffer.to_vec()))
}

pub struct PdfMetaTask(Vec<u8>);

#[napi]
impl Task for PdfMetaTask {
  type Output = PdfMeta;
  type JsValue = PdfMeta;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_metadata(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct ExtractAnnotationsTask(Vec<u8>);

#[napi]
impl Task for ExtractAnnotationsTask {
  type Output = Vec<PageAnnotation>;
  type JsValue = Vec<PageAnnotation>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_annotations(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_annotations_per_page_async(buffer: Buffer) -> AsyncTask<ExtractAnnotationsTask> {
  AsyncTask::new(ExtractAnnotationsTask(buffer.to_vec()))
}

pub struct ExtractLayoutTask(Vec<u8>);

#[napi]
impl Task for ExtractLayoutTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_layout(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn layout_per_page_async(buffer: Buffer) -> AsyncTask<ExtractLayoutTask> {
  AsyncTask::new(ExtractLayoutTask(buffer.to_vec()))
}

pub struct AttachmentsTask(Vec<u8>);

#[napi]
impl Task for AttachmentsTask {
  type Output = Vec<pdfdown::Attachment>;
  type JsValue = Vec<Attachment>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_attachments(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(Attachment::from).collect())
  }
}

#[napi]
pub fn attachments_async(buffer: Buffer) -> AsyncTask<AttachmentsTask> {
  AsyncTask::new(AttachmentsTask(buffer.to_vec()))
}

pub struct ExtractInvoiceTask {
  data: Vec<u8>,
  opts: Option<InvoiceOptions>,
}

#[napi]
impl Task for ExtractInvoiceTask {
  type Output = InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    let pages = pdf_invoice_pages(&doc, &self.data);
    Ok(extract_invoice_impl(&doc, self.opts.as_ref(), &pages))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_invoice_async(
  buffer: Buffer,
  opts: Option<InvoiceOptions>,
) -> AsyncTask<ExtractInvoiceTask> {
  AsyncTask::new(ExtractInvoiceTask {
    data: buffer.to_vec(),
    opts,
  })
}

#[napi]
pub fn pdf_metadata_async(buffer: Buffer) -> AsyncTask<PdfMetaTask> {
  AsyncTask::new(PdfMetaTask(buffer.to_vec()))
}

pub struct PdfDocumentTask(Vec<u8>);

#[napi]
impl Task for PdfDocumentTask {
  type Output = pdfdown::PdfDocument;
  type JsValue = PdfDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    extract_all(&doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocument::from(output))
  }
}

#[napi]
pub fn pdf_document_async(buffer: Buffer) -> AsyncTask<PdfDocumentTask> {
  AsyncTask::new(PdfDocumentTask(buffer.to_vec()))
}

pub struct ExtractStructuredTextTask(Vec<u8>);

#[napi]
impl Task for ExtractStructuredTextTask {
  type Output = Vec<StructuredPageText>;
  type JsValue = Vec<StructuredPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    extract_structured_text(&doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_structured_text_per_page_async(
  buffer: Buffer,
) -> AsyncTask<ExtractStructuredTextTask> {
  AsyncTask::new(ExtractStructuredTextTask(buffer.to_vec()))
}

pub struct ExtractFormFieldsTask(Vec<u8>);

#[napi]
impl Task for ExtractFormFieldsTask {
  type Output = Vec<PageFormField>;
  type JsValue = Vec<PageFormField>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_forms(&doc).fields)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_form_fields_per_page_async(buffer: Buffer) -> AsyncTask<ExtractFormFieldsTask> {
  AsyncTask::new(ExtractFormFieldsTask(buffer.to_vec()))
}

#[cfg(feature = "ocr")]
pub struct ExtractTextOcrTask {
  data: Vec<u8>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for ExtractTextOcrTask {
  type Output = Vec<OcrPageText>;
  type JsValue = Vec<OcrPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    extract_text_with_ocr(
      &doc,
      &self.data,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
#[napi]
pub fn extract_text_with_ocr_per_page_async(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> AsyncTask<ExtractTextOcrTask> {
  maybe_init_pdfium(&opts);
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.clone())
    .unwrap_or_else(|| "eng".to_string());
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  AsyncTask::new(ExtractTextOcrTask {
    data: buffer.to_vec(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  })
}

#[cfg(feature = "ocr")]
pub struct ExtractLayoutOcrTask {
  data: Vec<u8>,
  params: OcrParams,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for ExtractLayoutOcrTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    let p = &self.params;
    extract_layout_with_ocr(
      &doc,
      &self.data,
      &p.lang,
      p.min_len,
      p.max_threads,
      p.render_dpi,
      p.render_mode,
      None,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
#[napi]
pub fn extract_layout_with_ocr_per_page_async(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> AsyncTask<ExtractLayoutOcrTask> {
  AsyncTask::new(ExtractLayoutOcrTask {
    data: buffer.to_vec(),
    params: ocr_params(&opts),
  })
}

#[cfg(feature = "ocr")]
pub struct PdfDocumentOcrTask {
  data: Vec<u8>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for PdfDocumentOcrTask {
  type Output = pdfdown_ocr::PdfDocumentOcr;
  type JsValue = PdfDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    extract_all_with_ocr(
      &doc,
      &self.data,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocumentOcr::from(output))
  }
}

#[cfg(feature = "ocr")]
#[napi]
pub fn pdf_document_ocr_async(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> AsyncTask<PdfDocumentOcrTask> {
  maybe_init_pdfium(&opts);
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.clone())
    .unwrap_or_else(|| "eng".to_string());
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  AsyncTask::new(PdfDocumentOcrTask {
    data: buffer.to_vec(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  })
}

// ── Step 3: Class-based API with Arc<Document> ──────────────────

/// Shared-document task types for class async methods. They hold the parsed document, never raw
/// bytes, and check the document kind inside `compute`, so a workbook handed a PDF-only method
/// rejects the promise rather than throwing before it exists.
pub struct SharedExtractTextTask(Arc<PdfState>);

#[napi]
impl Task for SharedExtractTextTask {
  type Output = Vec<PageText>;
  type JsValue = Vec<PageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_text(&self.0.doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedExtractImagesTask(Arc<PdfState>);

#[napi]
impl Task for SharedExtractImagesTask {
  type Output = Vec<pdfdown::PageImage>;
  type JsValue = Vec<PageImage>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_images_raw(&self.0.doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(PageImage::from).collect())
  }
}

pub struct SharedExtractAnnotationsTask(Arc<PdfState>);

#[napi]
impl Task for SharedExtractAnnotationsTask {
  type Output = Vec<PageAnnotation>;
  type JsValue = Vec<PageAnnotation>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_annotations(&self.0.doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedLayoutTask(Arc<PdfState>);

#[napi]
impl Task for SharedLayoutTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_layout(&self.0.doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedAttachmentsTask(Arc<PdfState>);

#[napi]
impl Task for SharedAttachmentsTask {
  type Output = Vec<pdfdown::Attachment>;
  type JsValue = Vec<Attachment>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_attachments(&self.0.doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(Attachment::from).collect())
  }
}

pub struct SharedInvoiceTask {
  inner: Arc<PdfState>,
  opts: Option<InvoiceOptions>,
}

#[napi]
impl Task for SharedInvoiceTask {
  type Output = InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(invoice_of(&self.inner, self.opts.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedPdfMetaTask(Arc<PdfState>);

#[napi]
impl Task for SharedPdfMetaTask {
  type Output = PdfMeta;
  type JsValue = PdfMeta;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_metadata(&self.0.doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedPdfDocumentTask(Arc<PdfState>);

#[napi]
impl Task for SharedPdfDocumentTask {
  type Output = pdfdown::PdfDocument;
  type JsValue = PdfDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_all(&self.0.doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocument::from(output))
  }
}

pub struct SharedStructuredTextTask(Arc<PdfState>);

#[napi]
impl Task for SharedStructuredTextTask {
  type Output = Vec<StructuredPageText>;
  type JsValue = Vec<StructuredPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_structured_text(&self.0.doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedFormFieldsTask(Arc<PdfState>);

#[napi]
impl Task for SharedFormFieldsTask {
  type Output = Vec<PageFormField>;
  type JsValue = Vec<PageFormField>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_forms(&self.0.doc).fields)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
pub struct SharedExtractTextOcrTask {
  inner: Arc<PdfState>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedExtractTextOcrTask {
  type Output = Vec<OcrPageText>;
  type JsValue = Vec<OcrPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let state = &self.inner;
    extract_text_with_ocr(
      &state.doc,
      &state.raw,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
pub struct SharedLayoutOcrTask {
  inner: Arc<PdfState>,
  params: OcrParams,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedLayoutOcrTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let p = &self.params;
    let state = &self.inner;
    extract_layout_with_ocr(
      &state.doc,
      &state.raw,
      &p.lang,
      p.min_len,
      p.max_threads,
      p.render_dpi,
      p.render_mode,
      None,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
pub struct SharedPdfDocumentOcrTask {
  inner: Arc<PdfState>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedPdfDocumentOcrTask {
  type Output = pdfdown_ocr::PdfDocumentOcr;
  type JsValue = PdfDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    let state = &self.inner;
    extract_all_with_ocr(
      &state.doc,
      &state.raw,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocumentOcr::from(output))
  }
}

#[cfg(feature = "render")]
pub struct SharedRenderPagesTask {
  inner: Arc<PdfState>,
  dpi: u32,
  mode: u8,
}

#[cfg(feature = "render")]
#[napi]
impl Task for SharedRenderPagesTask {
  type Output = Vec<pdfdown::RenderedPage>;
  type JsValue = Vec<RenderedPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    use pdfdown::render::{RENDER_MODE_AUTO, RENDER_MODE_NEVER};

    let dpi = self.dpi;
    let mode = self.mode;
    let pdf_bytes = &self.inner.raw;

    if mode == RENDER_MODE_NEVER {
      return Ok(Vec::new());
    }

    pdfdown::render::ensure_pdfium_with_path(None)
      .map_err(|e| Error::from_reason(e.to_string()))?;

    let doc = load_doc(pdf_bytes)?;
    let pages = doc.get_pages();
    let page_count = pages.len() as u16;

    let indices: Vec<u16> = if mode == RENDER_MODE_AUTO {
      pages
        .iter()
        .filter_map(|(&page_num, &page_id)| {
          let raw = doc.extract_text(&[page_num]).unwrap_or_default();
          let non_ws: usize = raw.chars().filter(|c| !c.is_whitespace()).count();
          if non_ws > 0 {
            return None;
          }
          let has_images = !pdfdown::images::collect_page_decoded_images(&doc, page_id).is_empty();
          if has_images {
            return None;
          }
          Some((page_num - 1) as u16)
        })
        .collect()
    } else {
      (0..page_count).collect()
    };

    let rendered = pdfdown::render::render_pages_to_png(pdf_bytes, &indices, dpi);
    Ok(
      rendered
        .into_iter()
        .map(|(idx, w, h, data)| pdfdown::RenderedPage {
          page: (idx as u32) + 1,
          width: w,
          height: h,
          dpi,
          data,
        })
        .collect(),
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(RenderedPage::from).collect())
  }
}

/// A parsed PDF: the document and the bytes it came from (the OCR and render tiers re-read them).
pub struct PdfState {
  doc: Arc<Document>,
  #[allow(dead_code)] // used when ocr or render features are enabled
  raw: Arc<Vec<u8>>,
}

const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  let _ = assert_send_sync::<PdfState>;
};

fn parse_document(bytes: Vec<u8>) -> Result<PdfState> {
  let doc = load_doc(&bytes)?;
  Ok(PdfState {
    doc: Arc::new(doc),
    raw: Arc::new(bytes),
  })
}

pub struct OpenDocumentTask(Vec<u8>);

#[napi]
impl Task for OpenDocumentTask {
  type Output = PdfState;
  type JsValue = PdfDown;

  fn compute(&mut self) -> Result<Self::Output> {
    parse_document(std::mem::take(&mut self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDown {
      inner: Arc::new(output),
    })
  }
}

fn invoice_of(inner: &PdfState, opts: Option<&InvoiceOptions>) -> InvoiceResult {
  let pages = pdf_invoice_pages(&inner.doc, &inner.raw);
  extract_invoice_impl(&inner.doc, opts, &pages)
}

#[napi]
pub struct PdfDown {
  inner: Arc<PdfState>,
}

#[napi]
impl PdfDown {
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(PdfDown {
      inner: Arc::new(parse_document(buffer.to_vec())?),
    })
  }

  /// Open a PDF off the JavaScript thread; resolves to a `PdfDown`.
  #[napi]
  pub fn open_async(buffer: Buffer) -> AsyncTask<OpenDocumentTask> {
    AsyncTask::new(OpenDocumentTask(buffer.to_vec()))
  }

  #[napi]
  pub fn text_per_page(&self) -> Result<Vec<PageText>> {
    extract_text(&self.inner.doc)
  }

  #[napi]
  pub fn images_per_page(&self) -> Result<Vec<PageImage>> {
    Ok(
      extract_images_raw(&self.inner.doc)
        .into_iter()
        .map(PageImage::from)
        .collect(),
    )
  }

  #[napi]
  pub fn annotations_per_page(&self) -> Result<Vec<PageAnnotation>> {
    Ok(extract_annotations(&self.inner.doc))
  }

  #[napi]
  pub fn metadata(&self) -> Result<PdfMeta> {
    Ok(extract_metadata(&self.inner.doc))
  }

  /// Positioned words and lines per page, from the page's content stream (native text only).
  #[napi]
  pub fn layout(&self) -> Result<Vec<PageLayout>> {
    Ok(extract_layout(&self.inner.doc))
  }

  #[napi]
  pub fn layout_async(&self) -> Result<AsyncTask<SharedLayoutTask>> {
    Ok(AsyncTask::new(SharedLayoutTask(Arc::clone(&self.inner))))
  }

  #[napi]
  pub fn attachments(&self) -> Result<Vec<Attachment>> {
    Ok(
      extract_attachments(&self.inner.doc)
        .into_iter()
        .map(Attachment::from)
        .collect(),
    )
  }

  #[napi]
  pub fn attachments_async(&self) -> Result<AsyncTask<SharedAttachmentsTask>> {
    Ok(AsyncTask::new(SharedAttachmentsTask(Arc::clone(
      &self.inner,
    ))))
  }

  /// Structured invoice data: the embedded Factur-X / ZUGFeRD / XRechnung / UBL XML when the
  /// document carries it, else the fields read from the pages.
  #[napi]
  pub fn invoice(&self, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
    Ok(invoice_of(&self.inner, opts.as_ref()))
  }

  #[napi]
  pub fn invoice_async(
    &self,
    opts: Option<InvoiceOptions>,
  ) -> Result<AsyncTask<SharedInvoiceTask>> {
    Ok(AsyncTask::new(SharedInvoiceTask {
      inner: Arc::clone(&self.inner),
      opts,
    }))
  }

  #[napi]
  pub fn text_per_page_async(&self) -> Result<AsyncTask<SharedExtractTextTask>> {
    Ok(AsyncTask::new(SharedExtractTextTask(Arc::clone(
      &self.inner,
    ))))
  }

  #[napi]
  pub fn images_per_page_async(&self) -> Result<AsyncTask<SharedExtractImagesTask>> {
    Ok(AsyncTask::new(SharedExtractImagesTask(Arc::clone(
      &self.inner,
    ))))
  }

  #[napi]
  pub fn annotations_per_page_async(&self) -> Result<AsyncTask<SharedExtractAnnotationsTask>> {
    Ok(AsyncTask::new(SharedExtractAnnotationsTask(Arc::clone(
      &self.inner,
    ))))
  }

  #[napi]
  pub fn metadata_async(&self) -> Result<AsyncTask<SharedPdfMetaTask>> {
    Ok(AsyncTask::new(SharedPdfMetaTask(Arc::clone(&self.inner))))
  }

  #[napi]
  pub fn document(&self) -> Result<PdfDocument> {
    Ok(PdfDocument::from(extract_all(&self.inner.doc)?))
  }

  #[napi]
  pub fn document_async(&self) -> Result<AsyncTask<SharedPdfDocumentTask>> {
    Ok(AsyncTask::new(SharedPdfDocumentTask(Arc::clone(
      &self.inner,
    ))))
  }

  #[napi]
  pub fn structured_text(&self) -> Result<Vec<StructuredPageText>> {
    extract_structured_text(&self.inner.doc)
  }

  #[napi]
  pub fn structured_text_async(&self) -> Result<AsyncTask<SharedStructuredTextTask>> {
    Ok(AsyncTask::new(SharedStructuredTextTask(Arc::clone(
      &self.inner,
    ))))
  }

  #[napi]
  pub fn form_fields_per_page(&self) -> Result<Vec<PageFormField>> {
    Ok(extract_forms(&self.inner.doc).fields)
  }

  #[napi]
  pub fn form_fields_per_page_async(&self) -> Result<AsyncTask<SharedFormFieldsTask>> {
    Ok(AsyncTask::new(SharedFormFieldsTask(Arc::clone(
      &self.inner,
    ))))
  }
}

#[cfg(feature = "ocr")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn text_with_ocr_per_page(&self, opts: Option<OcrOptions>) -> Result<Vec<OcrPageText>> {
    let state = &self.inner;
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.as_deref())
      .unwrap_or("eng");
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    extract_text_with_ocr(
      &state.doc,
      &state.raw,
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    )
  }

  #[napi]
  pub fn text_with_ocr_per_page_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> Result<AsyncTask<SharedExtractTextOcrTask>> {
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    Ok(AsyncTask::new(SharedExtractTextOcrTask {
      inner: Arc::clone(&self.inner),
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    }))
  }

  /// Positioned words and lines per page with the OCR cascade.
  #[napi]
  pub fn layout_with_ocr(&self, opts: Option<OcrOptions>) -> Result<Vec<PageLayout>> {
    let state = &self.inner;
    let p = ocr_params(&opts);
    extract_layout_with_ocr(
      &state.doc,
      &state.raw,
      &p.lang,
      p.min_len,
      p.max_threads,
      p.render_dpi,
      p.render_mode,
      None,
    )
  }

  #[napi]
  pub fn layout_with_ocr_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> Result<AsyncTask<SharedLayoutOcrTask>> {
    Ok(AsyncTask::new(SharedLayoutOcrTask {
      inner: Arc::clone(&self.inner),
      params: ocr_params(&opts),
    }))
  }

  #[napi]
  pub fn document_ocr(&self, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
    let state = &self.inner;
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.as_deref())
      .unwrap_or("eng");
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    Ok(PdfDocumentOcr::from(extract_all_with_ocr(
      &state.doc,
      &state.raw,
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    )?))
  }

  #[napi]
  pub fn document_ocr_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> Result<AsyncTask<SharedPdfDocumentOcrTask>> {
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    Ok(AsyncTask::new(SharedPdfDocumentOcrTask {
      inner: Arc::clone(&self.inner),
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    }))
  }
}

#[cfg(feature = "render")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn render_pages_async(
    &self,
    opts: Option<RenderOptions>,
  ) -> Result<AsyncTask<SharedRenderPagesTask>> {
    use pdfdown::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};
    let dpi = pdfdown::render::normalize_dpi(opts.as_ref().and_then(|o| o.dpi));
    let mode = opts
      .as_ref()
      .and_then(|o| o.mode.as_ref())
      .map(|m| match m {
        RenderMode::Auto => RENDER_MODE_AUTO,
        RenderMode::Never => RENDER_MODE_NEVER,
        RenderMode::Always => RENDER_MODE_ALWAYS,
      })
      .unwrap_or(RENDER_MODE_ALWAYS);
    Ok(AsyncTask::new(SharedRenderPagesTask {
      inner: Arc::clone(&self.inner),
      dpi,
      mode,
    }))
  }
}
