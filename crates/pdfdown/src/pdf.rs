//! An opened PDF: the parsed document and the bytes it came from.

use contentdown::PageLayout;
use contentdown::invoice::{InvoiceOptions, InvoiceResult};
use lopdf::Document;

use crate::error::PdfError;
use crate::types::{
  Attachment, PageAnnotation, PageFormField, PageImage, PageText, PdfDocument, PdfMeta,
  StructuredPageText,
};

/// A parsed PDF. Parsing happens once, in [`Pdf::open`]; every reader works from the parsed
/// document. The original bytes are kept because rendering and OCR re-read them.
pub struct Pdf {
  document: Document,
  bytes: Vec<u8>,
}

impl Pdf {
  pub fn open(bytes: Vec<u8>) -> Result<Pdf, PdfError> {
    let document = Document::load_mem(&bytes)?;
    Ok(Pdf { document, bytes })
  }

  /// The parsed document, for readers outside this crate.
  pub fn document(&self) -> &Document {
    &self.document
  }

  /// The bytes the document was parsed from.
  pub fn bytes(&self) -> &[u8] {
    &self.bytes
  }

  pub fn text(&self) -> Vec<PageText> {
    crate::text::extract_text(&self.document)
  }

  pub fn structured_text(&self) -> Vec<StructuredPageText> {
    crate::text::extract_structured_text(&self.document)
  }

  pub fn images(&self) -> Vec<PageImage> {
    crate::images::extract_images_raw(&self.document)
  }

  /// How many images the pages paint. Nothing is decoded, so this is far cheaper than
  /// counting what [`Pdf::images`] returns.
  pub fn image_count(&self) -> u32 {
    crate::images::count_images(&self.document)
  }

  pub fn annotations(&self) -> Vec<PageAnnotation> {
    crate::document::extract_annotations(&self.document)
  }

  pub fn metadata(&self) -> PdfMeta {
    crate::meta::extract_metadata(&self.document)
  }

  pub fn layout(&self) -> Vec<PageLayout> {
    crate::layout::extract_layout(&self.document)
  }

  pub fn attachments(&self) -> Vec<Attachment> {
    crate::attachments::extract_attachments(&self.document)
  }

  pub fn form_fields(&self) -> Vec<PageFormField> {
    crate::forms::extract_forms(&self.document).fields
  }

  /// Text, images, annotations, metadata and form fields, gathered in one pass.
  pub fn extract_all(&self) -> PdfDocument {
    crate::document::extract_all(&self.document)
  }

  /// Invoice fields: embedded e-invoice XML when the document carries it and it is preferred,
  /// else the page text of `pages`. Pass the document's own layout, or one read by OCR.
  pub fn invoice(&self, options: Option<&InvoiceOptions>, pages: &[PageLayout]) -> InvoiceResult {
    crate::invoice::extract_invoice(&self.document, options, pages)
  }
}
