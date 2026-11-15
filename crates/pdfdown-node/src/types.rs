//! The shapes JavaScript sees for a PDF's text, images, annotations, metadata and attachments:
//! mirrors of `pdfdown`'s types, converted at the boundary. Bytes become `Buffer`s here, on the
//! JavaScript thread; everything else is plain data.

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

use crate::forms::{FormInfo, PageFormField};
#[cfg(feature = "ocr")]
use crate::layout::TextSource;

#[napi(object)]
pub struct PageText {
  pub page: u32,
  pub text: String,
}

impl From<pdfdown::PageText> for PageText {
  fn from(value: pdfdown::PageText) -> Self {
    PageText {
      page: value.page,
      text: value.text,
    }
  }
}

#[napi(object)]
pub struct StructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
}

impl From<pdfdown::StructuredPageText> for StructuredPageText {
  fn from(value: pdfdown::StructuredPageText) -> Self {
    StructuredPageText {
      page: value.page,
      header: value.header,
      body: value.body,
      footer: value.footer,
    }
  }
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrPageText {
  pub page: u32,
  pub text: String,
  pub source: TextSource,
}

#[cfg(feature = "ocr")]
impl From<pdfdown_ocr::OcrPageText> for OcrPageText {
  fn from(value: pdfdown_ocr::OcrPageText) -> Self {
    OcrPageText {
      page: value.page,
      text: value.text,
      source: value.source.into(),
    }
  }
}

#[cfg(all(feature = "ocr", not(feature = "render")))]
#[napi(object)]
pub struct OcrOptions {
  pub lang: Option<String>,
  pub min_text_length: Option<u32>,
  pub max_threads: Option<u32>,
}

#[cfg(all(feature = "ocr", feature = "render"))]
#[napi(object)]
pub struct OcrOptions {
  pub lang: Option<String>,
  pub min_text_length: Option<u32>,
  pub max_threads: Option<u32>,
  pub render: Option<RenderMode>,
  pub render_dpi: Option<u32>,
  pub pdfium_path: Option<String>,
}

#[cfg(feature = "render")]
#[napi(string_enum)]
pub enum RenderMode {
  Auto,
  Never,
  Always,
}

#[cfg(feature = "render")]
#[napi(object)]
pub struct RenderedPage {
  pub page: u32,
  pub width: u32,
  pub height: u32,
  pub dpi: u32,
  pub data: Buffer,
}

#[cfg(feature = "render")]
impl From<pdfdown::RenderedPage> for RenderedPage {
  fn from(value: pdfdown::RenderedPage) -> Self {
    RenderedPage {
      page: value.page,
      width: value.width,
      height: value.height,
      dpi: value.dpi,
      data: value.data.into(),
    }
  }
}

#[cfg(feature = "render")]
#[napi(object)]
pub struct RenderOptions {
  pub dpi: Option<u32>,
  pub mode: Option<RenderMode>,
}

#[napi(object)]
pub struct Capabilities {
  pub ocr: bool,
  pub render: bool,
}

#[napi(object)]
pub struct PdfMeta {
  pub page_count: u32,
  pub version: String,
  pub is_linearized: bool,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  /// AcroForm facts; present when `/AcroForm` exists or orphan widgets were found.
  pub form: Option<FormInfo>,
}

impl From<pdfdown::PdfMeta> for PdfMeta {
  fn from(value: pdfdown::PdfMeta) -> Self {
    PdfMeta {
      page_count: value.page_count,
      version: value.version,
      is_linearized: value.is_linearized,
      creator: value.creator,
      producer: value.producer,
      creation_date: value.creation_date,
      modification_date: value.modification_date,
      page_boxes: value.page_boxes.into_iter().map(Into::into).collect(),
      form: value.form.map(Into::into),
    }
  }
}

#[napi(string_enum)]
pub enum BoxType {
  CropBox,
  MediaBox,
  Unknown,
}

impl From<pdfdown::BoxType> for BoxType {
  fn from(value: pdfdown::BoxType) -> Self {
    match value {
      pdfdown::BoxType::CropBox => BoxType::CropBox,
      pdfdown::BoxType::MediaBox => BoxType::MediaBox,
      pdfdown::BoxType::Unknown => BoxType::Unknown,
    }
  }
}

/// Declares the two paper-size unions from one list of sizes, so they cannot drift apart: the
/// standard sizes on their own, and the same sizes with `UNKNOWN` beside them.
macro_rules! paper_sizes {
  ($($size:ident),+ $(,)?) => {
    /// A standard paper size, as a union keyed on `type`, with its own dimensions in points,
    /// portrait: `width` is the short side.
    #[napi(discriminant_case = "UPPERCASE")]
    pub enum StandardPaperSize {
      $($size { width: f64, height: f64 },)+
    }

    /// The standard paper size nearest to a page box, as a union keyed on `type`. The values of
    /// `type` are spelled the way a database enum spells them (`A4`, `LETTER`, `UNKNOWN`). A
    /// standard size carries its own dimensions in points, portrait: `width` is the short side.
    /// They are the standard's, not the page's; the page's are on the box.
    ///
    /// `UNKNOWN` carries `closestTo`, the standard sizes the page lies between by area: first the
    /// largest one no larger than the page, then the smallest one larger. A side is `null` when
    /// the page is beyond that end of the list (smaller than A10, or larger than B0).
    #[napi(discriminant_case = "UPPERCASE")]
    pub enum PaperSize {
      $($size { width: f64, height: f64 },)+
      Unknown {
        closest_to: (Option<StandardPaperSize>, Option<StandardPaperSize>),
      },
    }

    impl StandardPaperSize {
      /// `None` for the unknown size, which has no dimensions of its own.
      fn new(size: pdfdown::PaperSize) -> Option<Self> {
        let (width, height) = size.dimensions()?;
        match size {
          $(pdfdown::PaperSize::$size => Some(StandardPaperSize::$size { width, height }),)+
          pdfdown::PaperSize::Unknown => None,
        }
      }
    }

    impl From<StandardPaperSize> for PaperSize {
      fn from(value: StandardPaperSize) -> Self {
        match value {
          $(StandardPaperSize::$size { width, height } => PaperSize::$size { width, height },)+
        }
      }
    }
  };
}

paper_sizes!(
  A0, A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, B0, B1, B2, B3, B4, B5, B6, B7, B8, B9, B10, Letter,
  Legal, Tabloid, Executive, Statement
);

impl PaperSize {
  /// The union for the size of a `width` by `height` page box.
  fn new(size: pdfdown::PaperSize, width: f64, height: f64) -> Self {
    match StandardPaperSize::new(size) {
      Some(standard) => standard.into(),
      None => {
        let (smaller, larger) = contentdown::paper_neighbors(width, height);
        PaperSize::Unknown {
          closest_to: (
            smaller.and_then(StandardPaperSize::new),
            larger.and_then(StandardPaperSize::new),
          ),
        }
      }
    }
  }
}

#[napi(object)]
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
  /// The standard paper size nearest to `width` by `height`, in either orientation; its `type`
  /// is `UNKNOWN` when none is within 3% on both sides.
  pub paper_size: PaperSize,
  /// Present only on non-dominant boxes — lists the specific pages with these
  /// dimensions. `None` on the first (most frequent) entry means "all pages
  /// not listed in any other entry's `pages` array."
  pub pages: Option<Vec<u32>>,
}

impl From<pdfdown::PageBox> for PageBox {
  fn from(value: pdfdown::PageBox) -> Self {
    PageBox {
      page_count: value.page_count,
      left: value.left,
      bottom: value.bottom,
      right: value.right,
      top: value.top,
      width: value.width,
      height: value.height,
      box_type: value.box_type.into(),
      paper_size: PaperSize::new(value.paper_size, value.width, value.height),
      pages: value.pages,
    }
  }
}

#[napi(object)]
pub struct PageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Buffer,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

impl From<pdfdown::PageImage> for PageImage {
  fn from(value: pdfdown::PageImage) -> Self {
    PageImage {
      page: value.page,
      image_index: value.image_index,
      width: value.width,
      height: value.height,
      data: value.data.into(),
      color_space: value.color_space,
      bits_per_component: value.bits_per_component,
      filter: value.filter,
      xobject_name: value.xobject_name,
      object_id: value.object_id,
    }
  }
}

#[napi(object)]
pub struct PageAnnotation {
  pub page: u32,
  pub subtype: String,
  pub rect: Vec<f64>,
  pub uri: Option<String>,
  pub dest: Option<String>,
  pub content: Option<String>,
}

impl From<pdfdown::PageAnnotation> for PageAnnotation {
  fn from(value: pdfdown::PageAnnotation) -> Self {
    PageAnnotation {
      page: value.page,
      subtype: value.subtype,
      rect: value.rect,
      uri: value.uri,
      dest: value.dest,
      content: value.content,
    }
  }
}

#[napi(object, object_from_js = false)]
pub struct PdfDocument {
  pub version: String,
  pub is_linearized: bool,
  pub page_count: u32,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  pub total_images: u32,
  pub total_annotations: u32,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub text: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub form: Option<FormInfo>,
  pub form_fields: Vec<PageFormField>,
  pub total_form_fields: u32,
  pub form_field_pages: Vec<u32>,
}

impl From<pdfdown::PdfDocument> for PdfDocument {
  fn from(r: pdfdown::PdfDocument) -> Self {
    let total_images = r.images.len() as u32;
    let total_annotations = r.annotations.len() as u32;
    let total_form_fields = r.form_fields.len() as u32;
    PdfDocument {
      version: r.meta.version,
      is_linearized: r.meta.is_linearized,
      page_count: r.meta.page_count,
      creator: r.meta.creator,
      producer: r.meta.producer,
      creation_date: r.meta.creation_date,
      modification_date: r.meta.modification_date,
      page_boxes: r.meta.page_boxes.into_iter().map(Into::into).collect(),
      total_images,
      total_annotations,
      image_pages: r.image_pages,
      annotation_pages: r.annotation_pages,
      text: r.text.into_iter().map(Into::into).collect(),
      structured_text: r.structured_text.into_iter().map(Into::into).collect(),
      images: r.images.into_iter().map(Into::into).collect(),
      annotations: r.annotations.into_iter().map(Into::into).collect(),
      form: r.meta.form.map(Into::into),
      form_fields: r.form_fields.into_iter().map(Into::into).collect(),
      total_form_fields,
      form_field_pages: r.form_field_pages,
    }
  }
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrStructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
  pub source: TextSource,
}

#[cfg(feature = "ocr")]
impl From<pdfdown_ocr::OcrStructuredPageText> for OcrStructuredPageText {
  fn from(value: pdfdown_ocr::OcrStructuredPageText) -> Self {
    OcrStructuredPageText {
      page: value.page,
      header: value.header,
      body: value.body,
      footer: value.footer,
      source: value.source.into(),
    }
  }
}

#[cfg(feature = "ocr")]
#[napi(object, object_from_js = false)]
pub struct PdfDocumentOcr {
  pub version: String,
  pub is_linearized: bool,
  pub page_count: u32,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  pub total_images: u32,
  pub total_annotations: u32,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub text: Vec<OcrPageText>,
  pub structured_text: Vec<OcrStructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub form: Option<FormInfo>,
  pub form_fields: Vec<PageFormField>,
  pub total_form_fields: u32,
  pub form_field_pages: Vec<u32>,
}

#[cfg(feature = "ocr")]
impl From<pdfdown_ocr::PdfDocumentOcr> for PdfDocumentOcr {
  fn from(r: pdfdown_ocr::PdfDocumentOcr) -> Self {
    let total_images = r.images.len() as u32;
    let total_annotations = r.annotations.len() as u32;
    let total_form_fields = r.form_fields.len() as u32;
    PdfDocumentOcr {
      version: r.meta.version,
      is_linearized: r.meta.is_linearized,
      page_count: r.meta.page_count,
      creator: r.meta.creator,
      producer: r.meta.producer,
      creation_date: r.meta.creation_date,
      modification_date: r.meta.modification_date,
      page_boxes: r.meta.page_boxes.into_iter().map(Into::into).collect(),
      total_images,
      total_annotations,
      image_pages: r.image_pages,
      annotation_pages: r.annotation_pages,
      text: r.text.into_iter().map(Into::into).collect(),
      structured_text: r.structured_text.into_iter().map(Into::into).collect(),
      images: r.images.into_iter().map(Into::into).collect(),
      annotations: r.annotations.into_iter().map(Into::into).collect(),
      form: r.meta.form.map(Into::into),
      form_fields: r.form_fields.into_iter().map(Into::into).collect(),
      total_form_fields,
      form_field_pages: r.form_field_pages,
    }
  }
}

/// A file embedded in the PDF: a `/Names /EmbeddedFiles` entry, a catalog
/// `/AF` associated file, or a page-level `/FileAttachment` annotation.
#[napi(object)]
pub struct Attachment {
  pub name: String,
  /// From the embedded file stream's `/Subtype`, e.g. `text/xml`.
  pub mime_type: Option<String>,
  /// Decoded payload size in bytes.
  pub size: u32,
  /// `/AFRelationship`: `Source`, `Data`, `Alternative`, `Supplement`, ...
  pub relationship: Option<String>,
  pub description: Option<String>,
  /// 1-based page for a `/FileAttachment` annotation; absent for document-level files.
  pub page: Option<u32>,
  pub data: Buffer,
}

impl From<pdfdown::Attachment> for Attachment {
  fn from(r: pdfdown::Attachment) -> Self {
    Attachment {
      name: r.name,
      mime_type: r.mime_type,
      size: r.data.len() as u32,
      relationship: r.relationship,
      description: r.description,
      page: r.page,
      data: r.data.into(),
    }
  }
}
