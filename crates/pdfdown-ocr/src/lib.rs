//! OCR for PDFs, on top of `pdfdown`: text and layout for pages whose own text is missing or
//! too thin, read from the page's embedded images with Tesseract and, with the `render` feature,
//! from the rasterized page.
//!
//! Tesseract and Leptonica are linked from the system. Nothing here depends on Node.

mod document;
mod ocr;
mod types;

pub use document::extract_all_with_ocr;
pub use ocr::{
  RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER, detect_headers_footers_ocr,
  extract_layout_with_ocr, extract_text_with_ocr,
};
pub use ocrdown::normalize_max_threads;
pub use types::{OcrPageText, OcrStructuredPageText, PdfDocumentOcr};
