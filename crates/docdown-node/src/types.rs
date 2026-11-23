//! What a document carries, as JavaScript sees it: the files embedded in it and its pictures.
//! Nothing here is read back from JavaScript.

use docdown::{Doc, Embedded, Picture, PictureFormat};
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

use crate::invoice::InvoiceResult;
use crate::layout::DocPage;

/// A file embedded in the document, as it was before it was embedded. It is not read here:
/// it goes to the reader of what it is.
#[napi(object, object_from_js = false)]
pub struct DocEmbedded {
  /// The class of the object that holds it, as the program that embedded it names itself:
  /// `Package` for a file dropped into the document, `Excel.Sheet.8`, `AcroExch.Document.7`,
  /// and so on.
  pub class: Option<String>,
  /// The file's name, where the object keeps one.
  pub name: Option<String>,
  /// The file. A workbook or a document of before 2007 is given as the compound file it is.
  pub data: Buffer,
}

impl From<&Embedded> for DocEmbedded {
  fn from(file: &Embedded) -> Self {
    DocEmbedded {
      class: file.class.clone(),
      name: file.name.clone(),
      data: file.bytes.clone().into(),
    }
  }
}

/// What a picture's bytes are.
#[napi(string_enum = "lowercase")]
pub enum DocPictureFormat {
  Png,
  Jpeg,
  Tiff,
  /// An enhanced metafile: drawing commands, not pixels.
  Emf,
  /// A Windows metafile.
  Wmf,
  /// A bitmap with no file header, as Windows keeps one in memory.
  Dib,
  /// A QuickDraw picture.
  MacPict,
}

impl From<PictureFormat> for DocPictureFormat {
  fn from(format: PictureFormat) -> Self {
    match format {
      PictureFormat::Png => DocPictureFormat::Png,
      PictureFormat::Jpeg => DocPictureFormat::Jpeg,
      PictureFormat::Tiff => DocPictureFormat::Tiff,
      PictureFormat::Emf => DocPictureFormat::Emf,
      PictureFormat::Wmf => DocPictureFormat::Wmf,
      PictureFormat::Dib => DocPictureFormat::Dib,
      PictureFormat::MacPict => DocPictureFormat::MacPict,
    }
  }
}

/// A picture of the document, as its bytes. A metafile is not drawn.
#[napi(object, object_from_js = false)]
pub struct DocPicture {
  /// What the file says the bytes are.
  pub format: DocPictureFormat,
  pub data: Buffer,
  /// The width and height the picture is to be shown at, in points, where the file states
  /// both.
  pub width: Option<f64>,
  pub height: Option<f64>,
}

impl From<&Picture> for DocPicture {
  fn from(picture: &Picture) -> Self {
    // The file gives the size in twentieths of a point.
    let points = |twips: i32| f64::from(twips) / 20.0;
    DocPicture {
      format: picture.format.into(),
      data: picture.bytes.clone().into(),
      width: picture.goal.map(|goal| points(goal.0)),
      height: picture.goal.map(|goal| points(goal.1)),
    }
  }
}

/// Everything read from a document, in one object.
#[napi(object, object_from_js = false)]
pub struct DocDocument {
  /// A line to a paragraph, and a line to a table's row with a tab between its cells. A
  /// header and a footer are on the pages, and not in this.
  pub text: String,
  pub pages: Vec<DocPage>,
  pub embedded: Vec<DocEmbedded>,
  pub pictures: Vec<DocPicture>,
  /// What to know of the reading: a part of the text that could not be found in the file.
  pub warnings: Vec<String>,
  /// The file is a template.
  pub is_template: bool,
  /// The file holds macros. They are never read or run.
  pub has_macros: bool,
  pub invoice: InvoiceResult,
}

pub(crate) fn embedded_of(doc: &Doc) -> Vec<DocEmbedded> {
  doc.embedded().iter().map(DocEmbedded::from).collect()
}

pub(crate) fn pictures_of(doc: &Doc) -> Vec<DocPicture> {
  doc.pictures().iter().map(DocPicture::from).collect()
}
