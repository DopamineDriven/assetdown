//! A JPEG file as JavaScript sees it: mirrors of `jpegdown`'s facts, copied out at the boundary.
//! Nothing here is read back from JavaScript.

use jpegdown::{Coding, Color, Jpeg, Page};
use napi_derive::napi;

// ── Paper ─────────────────────────────────────────────────────────

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

    /// The standard paper size nearest to a page, as a union keyed on `type`. The
    /// values of `type` are spelled the way a database enum spells them (`A4`, `LETTER`,
    /// `UNKNOWN`). A standard size carries its own dimensions in points, portrait: `width` is
    /// the short side. They are the standard's, not the page's; the page's are its `sizeInPoints` and its `sizeAsRead`.
    ///
    /// `UNKNOWN` carries `closestTo`, the standard sizes the page lies between by area: first the
    /// largest one no larger than the page, then the smallest one larger. A side is `null` when
    /// the page is beyond that end of the list (smaller than A10, or larger than B0), and both
    /// are when the page states no resolution, and so has no size to compare.
    #[napi(discriminant_case = "UPPERCASE")]
    pub enum PaperSize {
      $($size { width: f64, height: f64 },)+
      Unknown {
        closest_to: (Option<StandardPaperSize>, Option<StandardPaperSize>),
      },
    }

    impl StandardPaperSize {
      /// `None` for the unknown size, which has no dimensions of its own.
      fn new(size: contentdown::PaperSize) -> Option<Self> {
        let (width, height) = size.dimensions()?;
        match size {
          $(contentdown::PaperSize::$size => Some(StandardPaperSize::$size { width, height }),)+
          contentdown::PaperSize::Unknown => None,
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
  /// The union for the size of a page that is `width` by `height`, when it has a size.
  fn new(size: contentdown::PaperSize, width: Option<f64>, height: Option<f64>) -> Self {
    match StandardPaperSize::new(size) {
      Some(standard) => standard.into(),
      None => {
        let (smaller, larger) = match (width, height) {
          (Some(width), Some(height)) => contentdown::paper_neighbors(width, height),
          _ => (None, None),
        };
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

// ── The page's facts ──────────────────────────────────────────────

/// A resolution, in pixels to the inch.
#[napi(object, object_from_js = false)]
pub struct JpegResolution {
  pub x: f64,
  pub y: f64,
}

/// A width and a height in points, of which there are 72 to the inch.
#[napi(object, object_from_js = false)]
pub struct JpegSize {
  pub width: f64,
  pub height: f64,
}

/// How the picture's data is coded, as a union keyed on `type`. `other` carries the marker of
/// the frame for a coding this package has no name for.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum JpegCoding {
  /// What nearly every JPEG is: eight bits, one pass.
  Baseline,
  /// One pass, with deeper samples or more tables than the baseline allows.
  Extended,
  /// Several passes, each sharpening the last.
  Progressive,
  /// Kept exactly, with no loss. A coding of its own, which is not read.
  Lossless,
  Other {
    marker: u32,
  },
}

impl From<Coding> for JpegCoding {
  fn from(coding: Coding) -> Self {
    match coding {
      Coding::Baseline => JpegCoding::Baseline,
      Coding::Extended => JpegCoding::Extended,
      Coding::Progressive => JpegCoding::Progressive,
      Coding::Lossless => JpegCoding::Lossless,
      Coding::Other(marker) => JpegCoding::Other {
        marker: u32::from(marker),
      },
    }
  }
}

/// What a pixel's components are, as a union keyed on `type`. `other` carries how many there
/// are.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum JpegColor {
  /// One component: what a scan in black and white or in gray is.
  Gray,
  /// Three components, kept as a brightness and two differences of color: what a JPEG in
  /// color nearly always is.
  YCbCr,
  Rgb,
  /// The four inks of a press.
  Cmyk,
  Ycck,
  Other {
    components: u32,
  },
}

impl From<Color> for JpegColor {
  fn from(color: Color) -> Self {
    match color {
      Color::Gray => JpegColor::Gray,
      Color::YCbCr => JpegColor::YCbCr,
      Color::Rgb => JpegColor::Rgb,
      Color::Cmyk => JpegColor::Cmyk,
      Color::Ycck => JpegColor::Ycck,
      Color::Other(components) => JpegColor::Other {
        components: u32::from(components),
      },
    }
  }
}

/// The size the page is read at, as it is to be looked at.
#[napi(object, object_from_js = false)]
pub struct JpegPageSize {
  /// In points.
  pub width: f64,
  pub height: f64,
  /// Absent when the size is the one the file's resolution makes. Otherwise the file states
  /// no resolution that can be believed (none at all, or one that would make a sheet no
  /// sheet is), and the picture is taken to be a sheet of this paper, the one its shape is
  /// nearest to among letter, A4, and legal.
  pub assumed: Option<StandardPaperSize>,
}

/// The file's page, and what the file says of it. Nothing of the picture is decoded to make
/// this.
#[napi(object, object_from_js = false)]
pub struct JpegPage {
  /// The picture's size in pixels, as it lies in the file: before its orientation turns it.
  pub width: u32,
  pub height: u32,
  /// As the file states it. Absent when it states none, or states one in no unit. It is not
  /// always a measurement: software writes 72 or 96 when it has measured nothing.
  pub resolution: Option<JpegResolution>,
  pub bits_per_sample: u32,
  /// How many components a pixel has.
  pub components: u32,
  pub color: JpegColor,
  pub coding: JpegCoding,
  /// How the picture is to be turned for display, numbered from 1 to 8 as Exif does: 1 is as
  /// it lies. `jpegPng` gives the picture already turned.
  pub orientation: u32,
  /// The program that wrote the file, as it names itself.
  pub software: Option<String>,
  /// When the file was written, as it records it: `2015-03-04T10:22:33`. The format has no
  /// place for a time zone, so there is none.
  pub date_time: Option<String>,
  /// The page's size in points as it lies in the file, by the resolution the file states.
  /// Absent for a file that states none.
  pub size_in_points: Option<JpegSize>,
  /// The standard paper size nearest to `sizeInPoints`, in either orientation; its `type` is
  /// `UNKNOWN` when none is within 3% on both sides, or the file states no resolution.
  pub paper_size: PaperSize,
  /// The size the page is read at, which is the stated one when that can be believed.
  pub size_as_read: JpegPageSize,
}

impl From<&Page> for JpegPage {
  fn from(page: &Page) -> Self {
    let size = page.size_in_points();
    let paper = page.paper_size().unwrap_or(contentdown::PaperSize::Unknown);
    let read = page.size_as_read();
    JpegPage {
      width: page.width,
      height: page.height,
      resolution: page.resolution.map(|resolution| JpegResolution {
        x: resolution.x,
        y: resolution.y,
      }),
      bits_per_sample: u32::from(page.bits_per_sample),
      components: u32::from(page.components),
      color: page.color.into(),
      coding: page.coding.into(),
      orientation: u32::from(page.orientation),
      software: page.software.clone(),
      date_time: page.date_time.clone(),
      size_in_points: size.map(|(width, height)| JpegSize { width, height }),
      paper_size: PaperSize::new(paper, size.map(|size| size.0), size.map(|size| size.1)),
      size_as_read: JpegPageSize {
        width: read.width,
        height: read.height,
        assumed: read.assumed.and_then(StandardPaperSize::new),
      },
    }
  }
}

// ── A file ────────────────────────────────────────────────────────

/// A file's page, with its size in bytes.
#[napi(object, object_from_js = false)]
pub struct JpegDocument {
  /// How many bytes the file has.
  pub byte_size: f64,
  pub page: JpegPage,
}

pub(crate) fn page_of(jpeg: &Jpeg) -> JpegPage {
  JpegPage::from(jpeg.page())
}

pub(crate) fn document_of(jpeg: &Jpeg) -> JpegDocument {
  JpegDocument {
    // A file's length is far below 2^53, past which a JS number stops counting by ones.
    byte_size: jpeg.byte_size() as f64,
    page: page_of(jpeg),
  }
}
