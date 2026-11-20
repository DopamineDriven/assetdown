//! What a document carries, as JavaScript sees it: the files embedded in it and its pictures.
//! Nothing here is read back from JavaScript.

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use rtfdown::{Embedded, Picture, PictureFormat, Rtf};

use crate::invoice::InvoiceResult;
use crate::layout::RtfPage;

/// A file embedded in the document, as it was before it was embedded. It is not read here:
/// it goes to the reader of what it is.
#[napi(object, object_from_js = false)]
pub struct RtfEmbedded {
  /// The class of the object that holds it, as the program that embedded it names itself:
  /// `Outlook.FileAttach` for a file attached to a mail message, `Package` for a file dropped
  /// into a document, `Word.Document.12`, and so on.
  pub class: Option<String>,
  /// The file's name, where the object keeps one.
  pub name: Option<String>,
  /// The file. For a class this package does not know, the object's own data as it is.
  pub data: Buffer,
}

impl From<&Embedded> for RtfEmbedded {
  fn from(file: &Embedded) -> Self {
    RtfEmbedded {
      class: file.class.clone(),
      name: file.name.clone(),
      data: file.bytes.clone().into(),
    }
  }
}

/// What a picture's bytes are.
#[napi(string_enum = "lowercase")]
pub enum RtfPictureFormat {
  Png,
  Jpeg,
  /// An enhanced metafile: drawing commands, not pixels.
  Emf,
  /// A Windows metafile.
  Wmf,
  /// A bitmap with no file header, as Windows keeps one in memory.
  Dib,
  /// A bitmap of the kind Windows had before that.
  Bitmap,
  /// A QuickDraw picture.
  MacPict,
  /// Neither the group nor the bytes say.
  Unknown,
}

impl From<PictureFormat> for RtfPictureFormat {
  fn from(format: PictureFormat) -> Self {
    match format {
      PictureFormat::Png => RtfPictureFormat::Png,
      PictureFormat::Jpeg => RtfPictureFormat::Jpeg,
      PictureFormat::Emf => RtfPictureFormat::Emf,
      PictureFormat::Wmf => RtfPictureFormat::Wmf,
      PictureFormat::Dib => RtfPictureFormat::Dib,
      PictureFormat::Bitmap => RtfPictureFormat::Bitmap,
      PictureFormat::MacPict => RtfPictureFormat::MacPict,
      PictureFormat::Unknown => RtfPictureFormat::Unknown,
    }
  }
}

/// A picture of the document, as its bytes. A metafile is not drawn.
#[napi(object, object_from_js = false)]
pub struct RtfPicture {
  /// What the bytes open as, where they open as a PNG or a JPEG; otherwise what the file says.
  pub format: RtfPictureFormat,
  pub data: Buffer,
  /// The width and height the picture is to be shown at, in points, where the file states
  /// both.
  pub width: Option<f64>,
  pub height: Option<f64>,
}

impl From<&Picture> for RtfPicture {
  fn from(picture: &Picture) -> Self {
    // The file gives the size in twentieths of a point.
    let points = |twips: i32| f64::from(twips) / 20.0;
    RtfPicture {
      format: picture.format.into(),
      data: picture.bytes.clone().into(),
      width: picture.goal.map(|goal| points(goal.0)),
      height: picture.goal.map(|goal| points(goal.1)),
    }
  }
}

/// Everything read from a document, in one object.
#[napi(object, object_from_js = false)]
pub struct RtfDocument {
  /// A line to a paragraph, and a line to a table's row with a tab between its cells. A
  /// header and a footer are on the pages, and not in this.
  pub text: String,
  pub pages: Vec<RtfPage>,
  pub embedded: Vec<RtfEmbedded>,
  pub pictures: Vec<RtfPicture>,
  /// What to know of the reading: an attachment whose place is marked in the text and that
  /// is not in the file.
  pub warnings: Vec<String>,
  pub invoice: InvoiceResult,
}

pub(crate) fn embedded_of(rtf: &Rtf) -> Vec<RtfEmbedded> {
  rtf.embedded().iter().map(RtfEmbedded::from).collect()
}

pub(crate) fn pictures_of(rtf: &Rtf) -> Vec<RtfPicture> {
  rtf.pictures().iter().map(RtfPicture::from).collect()
}
