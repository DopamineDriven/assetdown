//! What the OCR readers return: the shapes `pdfdown` returns, with the tier each page's text
//! came from.

use contentdown::TextSource;
use pdfdown::{PageAnnotation, PageFormField, PageImage, PdfMeta};

#[derive(Debug, Clone, PartialEq)]
pub struct OcrPageText {
  pub page: u32,
  pub text: String,
  pub source: TextSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrStructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
  pub source: TextSource,
}

/// Everything the readers return for a document, with its text read through the OCR cascade.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocumentOcr {
  pub meta: PdfMeta,
  pub text: Vec<OcrPageText>,
  pub structured_text: Vec<OcrStructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub form_fields: Vec<PageFormField>,
  pub form_field_pages: Vec<u32>,
}
