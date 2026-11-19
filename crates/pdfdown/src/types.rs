//! The plain data the readers return. Bytes are `Vec<u8>`; a binding converts at its boundary.

mod forms;

pub use contentdown::PaperSize;
pub use forms::{ChoiceOption, FormInfo, PageFormField, UnplacedFormField};
pub(crate) use forms::{FieldCommon, FieldKind};

#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
  pub page: u32,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfMeta {
  pub page_count: u32,
  pub version: String,
  pub is_linearized: bool,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  /// What the document says of itself in its Info dictionary, as the file writes it.
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  /// One string, which is usually a list separated by commas or semicolons.
  pub keywords: Option<String>,
  /// The language the catalog names for the document's text: `en-US`.
  pub language: Option<String>,
  /// The file was encrypted on disk, whether or not it opened without a password.
  pub is_encrypted: bool,
  pub page_boxes: Vec<PageBox>,
  /// AcroForm facts; present when `/AcroForm` exists or orphan widgets were found.
  pub form: Option<FormInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxType {
  CropBox,
  MediaBox,
  Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageBox {
  /// Number of pages that share these dimensions.
  pub page_count: u32,
  pub left: f64,
  pub bottom: f64,
  pub right: f64,
  pub top: f64,
  pub width: f64,
  pub height: f64,
  pub box_type: BoxType,
  /// The standard paper size nearest to `width` by `height`, in either orientation.
  pub paper_size: PaperSize,
  /// Present only on non-dominant boxes — lists the specific pages with these
  /// dimensions. `None` on the first (most frequent) entry means "all pages
  /// not listed in any other entry's `pages` array."
  pub pages: Option<Vec<u32>>,
}

/// One image painted on a page, encoded as PNG.
#[derive(Debug, Clone, PartialEq)]
pub struct PageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Vec<u8>,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageAnnotation {
  pub page: u32,
  pub subtype: String,
  pub rect: Vec<f64>,
  pub uri: Option<String>,
  pub dest: Option<String>,
  pub content: Option<String>,
}

/// Everything the readers return for a document, gathered in one pass.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocument {
  pub meta: PdfMeta,
  pub text: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub form_fields: Vec<PageFormField>,
  pub form_field_pages: Vec<u32>,
}

/// A file embedded in the PDF: a `/Names /EmbeddedFiles` entry, a catalog
/// `/AF` associated file, or a page-level `/FileAttachment` annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
  pub name: String,
  /// From the embedded file stream's `/Subtype`, e.g. `text/xml`.
  pub mime_type: Option<String>,
  /// `/AFRelationship`: `Source`, `Data`, `Alternative`, `Supplement`, ...
  pub relationship: Option<String>,
  pub description: Option<String>,
  /// 1-based page for a `/FileAttachment` annotation; absent for document-level files.
  pub page: Option<u32>,
  /// The decoded payload.
  pub data: Vec<u8>,
}

/// One page rasterized to PNG.
#[cfg(feature = "render")]
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPage {
  pub page: u32,
  pub width: u32,
  pub height: u32,
  pub dpi: u32,
  pub data: Vec<u8>,
}
