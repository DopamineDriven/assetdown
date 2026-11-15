//! `pdfdown`'s readers under the names `lib.rs` calls, returning the shapes JavaScript sees.
//! Types that carry bytes are returned as the library's own, because a `Buffer` can only be made
//! on the JavaScript thread; the callers convert those when they resolve.

use napi::bindgen_prelude::Result;
use pdfdown::lopdf::Document;

use crate::forms::PageFormField;
use crate::invoice::{InvoiceOptions, InvoiceResult};
use crate::layout::PageLayout;
#[cfg(feature = "ocr")]
use crate::types::OcrPageText;
use crate::types::{PageAnnotation, PageText, PdfMeta, StructuredPageText};

pub(crate) use pdfdown::attachments::extract_attachments;
pub(crate) use pdfdown::images::extract_images_raw;

pub(crate) fn extract_text(doc: &Document) -> Result<Vec<PageText>> {
  Ok(
    pdfdown::text::extract_text(doc)
      .into_iter()
      .map(Into::into)
      .collect(),
  )
}

pub(crate) fn extract_structured_text(doc: &Document) -> Result<Vec<StructuredPageText>> {
  Ok(
    pdfdown::text::extract_structured_text(doc)
      .into_iter()
      .map(Into::into)
      .collect(),
  )
}

pub(crate) fn extract_annotations(doc: &Document) -> Vec<PageAnnotation> {
  pdfdown::document::extract_annotations(doc)
    .into_iter()
    .map(Into::into)
    .collect()
}

pub(crate) fn extract_layout(doc: &Document) -> Vec<PageLayout> {
  pdfdown::layout::extract_layout(doc)
    .into_iter()
    .map(Into::into)
    .collect()
}

pub(crate) fn extract_metadata(doc: &Document) -> PdfMeta {
  pdfdown::meta::extract_metadata(doc).into()
}

pub(crate) fn extract_all(doc: &Document) -> Result<pdfdown::PdfDocument> {
  Ok(pdfdown::document::extract_all(doc))
}

/// The form fields of a document, as `extract_forms(doc).fields`.
pub(crate) struct Forms {
  pub fields: Vec<PageFormField>,
}

pub(crate) fn extract_forms(doc: &Document) -> Forms {
  Forms {
    fields: pdfdown::forms::extract_forms(doc)
      .fields
      .into_iter()
      .map(Into::into)
      .collect(),
  }
}

/// Invoice fields for a PDF: its embedded XML when preferred, else `pages`.
pub(crate) fn extract_invoice_impl(
  doc: &Document,
  opts: Option<&InvoiceOptions>,
  pages: &[contentdown::PageLayout],
) -> InvoiceResult {
  let options = opts.cloned().map(Into::into);
  pdfdown::invoice::extract_invoice(doc, options.as_ref(), pages).into()
}

/// The layout track over pages a caller supplied.
pub(crate) fn extract_from_pages(
  pages: &[PageLayout],
  opts: Option<&InvoiceOptions>,
) -> InvoiceResult {
  let options = opts.cloned().map(Into::into);
  let pages: Vec<contentdown::PageLayout> = pages.iter().cloned().map(Into::into).collect();
  contentdown::invoice::extract_from_pages(&pages, options.as_ref()).into()
}

#[cfg(feature = "ocr")]
pub(crate) fn extract_text_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
) -> Result<Vec<OcrPageText>> {
  Ok(
    pdfdown_ocr::extract_text_with_ocr(
      doc,
      pdf_bytes,
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    )
    .into_iter()
    .map(Into::into)
    .collect(),
  )
}

#[cfg(feature = "ocr")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn extract_layout_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
  only_pages: Option<&[u32]>,
) -> Result<Vec<PageLayout>> {
  Ok(
    pdfdown_ocr::extract_layout_with_ocr(
      doc,
      pdf_bytes,
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
      only_pages,
    )
    .into_iter()
    .map(Into::into)
    .collect(),
  )
}

#[cfg(feature = "ocr")]
pub(crate) fn extract_all_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
) -> Result<pdfdown_ocr::PdfDocumentOcr> {
  Ok(pdfdown_ocr::extract_all_with_ocr(
    doc,
    pdf_bytes,
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  ))
}
