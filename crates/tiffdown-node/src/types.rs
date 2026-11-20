//! A TIFF file as JavaScript sees it: mirrors of `tiffdown`'s facts, copied out at the boundary.
//! Nothing here is read back from JavaScript.

use napi_derive::napi;
use tiffdown::{Compression, Page, Photometric, Tiff};

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
    /// the short side. They are the standard's, not the page's; the page's are its `sizeInPoints`.
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

// ── A page's facts ────────────────────────────────────────────────

/// A resolution, in pixels to the inch. A fax can have fewer down than across: its pixels are
/// then not square.
#[napi(object, object_from_js = false)]
pub struct TiffResolution {
  pub x: f64,
  pub y: f64,
}

/// A width and a height in points, of which there are 72 to the inch.
#[napi(object, object_from_js = false)]
pub struct TiffSize {
  pub width: f64,
  pub height: f64,
}

/// How a page's pixels are kept, as a union keyed on `type`. `other` carries the number the
/// format gives a coding this package has no name for.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum TiffCompression {
  /// As they are.
  None,
  /// A fax machine's first coding, a line at a time, with no marks between the lines.
  ModifiedHuffman,
  /// CCITT Group 3 (T.4).
  Group3,
  /// CCITT Group 4 (T.6): what a scanned or faxed page usually is.
  Group4,
  Lzw,
  /// JPEG as TIFF first wrote it, before the form that replaced it.
  OldJpeg,
  Jpeg,
  Deflate,
  PackBits,
  Other {
    code: u32,
  },
}

impl From<Compression> for TiffCompression {
  fn from(compression: Compression) -> Self {
    match compression {
      Compression::None => TiffCompression::None,
      Compression::ModifiedHuffman => TiffCompression::ModifiedHuffman,
      Compression::Group3 => TiffCompression::Group3,
      Compression::Group4 => TiffCompression::Group4,
      Compression::Lzw => TiffCompression::Lzw,
      Compression::OldJpeg => TiffCompression::OldJpeg,
      Compression::Jpeg => TiffCompression::Jpeg,
      Compression::Deflate => TiffCompression::Deflate,
      Compression::PackBits => TiffCompression::PackBits,
      Compression::Other(code) => TiffCompression::Other {
        code: u32::from(code),
      },
    }
  }
}

/// What a sample's value means, as a union keyed on `type`. `other` carries the number the
/// format gives a meaning this package has no name for.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum TiffPhotometric {
  /// One sample to a pixel, and 0 is white: what a fax is.
  WhiteIsZero,
  /// One sample to a pixel, and 0 is black.
  BlackIsZero,
  Rgb,
  /// Each sample picks a color from the page's own table.
  Palette,
  /// A mask that says which pixels of another picture show.
  Mask,
  Cmyk,
  YCbCr,
  CieLab,
  Other {
    code: u32,
  },
}

impl From<Photometric> for TiffPhotometric {
  fn from(photometric: Photometric) -> Self {
    match photometric {
      Photometric::WhiteIsZero => TiffPhotometric::WhiteIsZero,
      Photometric::BlackIsZero => TiffPhotometric::BlackIsZero,
      Photometric::Rgb => TiffPhotometric::Rgb,
      Photometric::Palette => TiffPhotometric::Palette,
      Photometric::Mask => TiffPhotometric::Mask,
      Photometric::Cmyk => TiffPhotometric::Cmyk,
      Photometric::YCbCr => TiffPhotometric::YCbCr,
      Photometric::CieLab => TiffPhotometric::CieLab,
      Photometric::Other(code) => TiffPhotometric::Other {
        code: u32::from(code),
      },
    }
  }
}

/// One page of a file, and what the file says of it. Nothing of the page's pixels is read to
/// make this.
#[napi(object, object_from_js = false)]
pub struct TiffPage {
  /// From 1.
  pub number: u32,
  /// The page's size in pixels, as it lies in the file: before its orientation turns it.
  pub width: u32,
  pub height: u32,
  /// Absent when the file states no resolution, or states one in no unit.
  pub resolution: Option<TiffResolution>,
  /// How many bits each sample of a pixel has, and how many samples a pixel has.
  pub bits_per_sample: Vec<u32>,
  pub samples_per_pixel: u32,
  pub compression: TiffCompression,
  /// Absent when the file does not say.
  pub photometric: Option<TiffPhotometric>,
  /// How the page is to be turned for display, as the format numbers it from 1 to 8: 1 is as
  /// it lies. `pagePng` gives the page already turned.
  pub orientation: u32,
  /// The program that wrote the page, as it names itself.
  pub software: Option<String>,
  /// When the page was written, as the file records it: `2015-03-04T10:22:33`. The format has
  /// no place for a time zone, so there is none.
  pub date_time: Option<String>,
  /// The page's size in points as it lies in the file. Absent for a page that states no
  /// resolution.
  pub size_in_points: Option<TiffSize>,
  /// The standard paper size nearest to the page, in either orientation; its `type` is
  /// `UNKNOWN` when none is within 3% on both sides, or the page states no resolution.
  pub paper_size: PaperSize,
}

impl From<&Page> for TiffPage {
  fn from(page: &Page) -> Self {
    let size = page.size_in_points();
    let paper = page.paper_size().unwrap_or(contentdown::PaperSize::Unknown);
    TiffPage {
      number: page.number,
      width: page.width,
      height: page.height,
      resolution: page.resolution.map(|resolution| TiffResolution {
        x: resolution.x,
        y: resolution.y,
      }),
      bits_per_sample: page
        .bits_per_sample
        .iter()
        .copied()
        .map(u32::from)
        .collect(),
      samples_per_pixel: u32::from(page.samples_per_pixel),
      compression: page.compression.into(),
      photometric: page.photometric.map(Into::into),
      orientation: u32::from(page.orientation),
      software: page.software.clone(),
      date_time: page.date_time.clone(),
      size_in_points: size.map(|(width, height)| TiffSize { width, height }),
      paper_size: PaperSize::new(paper, size.map(|size| size.0), size.map(|size| size.1)),
    }
  }
}

// ── A file ────────────────────────────────────────────────────────

/// A file's pages, with its size and what was wrong with it.
#[napi(object, object_from_js = false)]
pub struct TiffDocument {
  /// How many bytes the file has.
  pub byte_size: f64,
  pub page_count: u32,
  pub pages: Vec<TiffPage>,
  /// What was wrong with the file, where it could still be read: a chain of directories that
  /// leads back on itself, a directory that lies past the end of the file.
  pub warnings: Vec<String>,
}

/// How many pages a file has. The reader follows no more directories than a `u32` counts.
pub(crate) fn page_count_of(tiff: &Tiff) -> u32 {
  u32::try_from(tiff.pages().len()).unwrap_or(u32::MAX)
}

pub(crate) fn pages_of(tiff: &Tiff) -> Vec<TiffPage> {
  tiff.pages().iter().map(TiffPage::from).collect()
}

pub(crate) fn document_of(tiff: &Tiff) -> TiffDocument {
  TiffDocument {
    // A file's length is far below 2^53, past which a JS number stops counting by ones.
    byte_size: tiff.byte_size() as f64,
    page_count: page_count_of(tiff),
    pages: pages_of(tiff),
    warnings: tiff.warnings().to_vec(),
  }
}
