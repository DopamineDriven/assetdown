//! PDF reading: per-page text, images, annotations, form fields, attachments, metadata and the
//! positioned layout of each page, plus invoice extraction over that layout.
//!
//! [`Pdf::open`] parses a document once; its methods read from the parsed document. The same
//! readers are free functions over a `lopdf::Document` in the modules below, which is what
//! `pdfdown-ocr` builds on. Pages of positioned words and the invoice extractor come from
//! `contentdown`. Rendering pages to images is behind the `render` feature. Nothing here depends
//! on Node.

pub mod attachments;
pub mod document;
mod error;
pub mod forms;
pub mod images;
pub mod invoice;
pub mod layout;
pub mod meta;
mod pdf;
#[cfg(feature = "render")]
pub mod render;
pub mod text;
mod types;

pub use error::PdfError;
// The readers take a `lopdf::Document`, so the crate that defines it is part of this one's API.
pub use lopdf;
pub use pdf::Pdf;
#[cfg(feature = "render")]
pub use types::RenderedPage;
pub use types::{
  Attachment, BoxType, ChoiceOption, FormInfo, PageAnnotation, PageBox, PageFormField, PageImage,
  PageText, PdfDocument, PdfMeta, StructuredPageText, UnplacedFormField,
};
