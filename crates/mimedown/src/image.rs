//! What an image's header says of it: its size, its frames, how its color is kept.
//!
//! The fields every format shares are in [`ImageFacts`], under the names of the columns they
//! fill. What only one format has is in [`ImageDetail`]. Nothing here decodes a pixel.

pub(crate) mod bmp;
pub(crate) mod gif;
pub(crate) mod hdr;
pub(crate) mod heif;
pub(crate) mod ico;
pub(crate) mod jp2;
pub(crate) mod jpeg;
pub(crate) mod jxl;
pub(crate) mod jxr;
pub(crate) mod png;
pub(crate) mod svg;
pub(crate) mod tiff;
pub(crate) mod webp;

pub use bmp::BmpDetail;
pub use gif::GifDetail;
pub use hdr::{HdrDetail, HdrEncoding};
pub use heif::HeifDetail;
pub use ico::{IcoDetail, IcoEntry, IcoKind};
pub use jp2::Jp2Detail;
pub use jpeg::JpegDetail;
pub use jxl::JxlDetail;
pub use jxr::JxrDetail;
pub use png::{PngDetail, PngText, PngTextKind};
pub use svg::SvgDetail;
pub use tiff::TiffDetail;
pub use webp::WebpDetail;

use crate::container::exif::Exif;
use crate::container::icc::IccProfile;
use crate::container::xmp::Xmp;
use crate::source::{ByteRange, Missing};

/// How an image's samples are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorModel {
  Rgb,
  Rgba,
  Grayscale,
  GrayscaleAlpha,
  Indexed,
  Cmyk,
  Ycbcr,
  Ycck,
  Vector,
  Lab,
  Unknown,
}

impl ColorModel {
  pub fn as_str(self) -> &'static str {
    match self {
      ColorModel::Rgb => "rgb",
      ColorModel::Rgba => "rgba",
      ColorModel::Grayscale => "grayscale",
      ColorModel::GrayscaleAlpha => "grayscale_alpha",
      ColorModel::Indexed => "indexed",
      ColorModel::Cmyk => "cmyk",
      ColorModel::Ycbcr => "ycbcr",
      ColorModel::Ycck => "ycck",
      ColorModel::Vector => "vector",
      ColorModel::Lab => "lab",
      ColorModel::Unknown => "unknown",
    }
  }
}

/// Which colors an image's samples stand for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSpace {
  Srgb,
  DisplayP3,
  AdobeRgb,
  ProphotoRgb,
  Rec2020,
  Rec709,
  Cmyk,
  Lab,
  Xyz,
  Gray,
  Unknown,
}

impl ColorSpace {
  pub fn as_str(self) -> &'static str {
    match self {
      ColorSpace::Srgb => "srgb",
      ColorSpace::DisplayP3 => "display_p3",
      ColorSpace::AdobeRgb => "adobe_rgb",
      ColorSpace::ProphotoRgb => "prophoto_rgb",
      ColorSpace::Rec2020 => "rec2020",
      ColorSpace::Rec709 => "rec709",
      ColorSpace::Cmyk => "cmyk",
      ColorSpace::Lab => "lab",
      ColorSpace::Xyz => "xyz",
      ColorSpace::Gray => "gray",
      ColorSpace::Unknown => "unknown",
    }
  }

  /// The space a color profile's name stands for, when the name is one that is known.
  pub(crate) fn named(name: &str) -> Option<ColorSpace> {
    let name = name.to_ascii_lowercase();
    let says = |words: &[&str]| words.iter().any(|word| name.contains(word));
    if says(&["display p3", "displayp3", "p3 d65", "p3-d65"]) {
      Some(ColorSpace::DisplayP3)
    } else if says(&["adobe rgb", "adobergb", "oprgb"]) {
      Some(ColorSpace::AdobeRgb)
    } else if says(&["prophoto", "romm"]) {
      Some(ColorSpace::ProphotoRgb)
    } else if says(&["2020"]) {
      Some(ColorSpace::Rec2020)
    } else if says(&["rec. 709", "rec709", "bt.709", "bt709"]) {
      Some(ColorSpace::Rec709)
    } else if says(&["srgb", "iec61966-2", "iec 61966-2"]) {
      Some(ColorSpace::Srgb)
    } else {
      None
    }
  }
}

/// What only one format says of an image.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageDetail {
  Png(PngDetail),
  Gif(GifDetail),
  Bmp(BmpDetail),
  Ico(IcoDetail),
  Jpeg(JpegDetail),
  Tiff(TiffDetail),
  Webp(WebpDetail),
  Heif(HeifDetail),
  Svg(SvgDetail),
  Jp2(Jp2Detail),
  Jxl(JxlDetail),
  Jxr(JxrDetail),
  Hdr(HdrDetail),
}

/// What an image's header says of it. The fields are the columns of an image's metadata row.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageFacts {
  pub width: u32,
  pub height: u32,
  /// How many pictures the file holds: an animation's frames, an icon's sizes.
  pub frames: u32,
  pub animated: bool,
  /// Whether any of it can be see-through. None when the header does not say.
  pub has_alpha: Option<bool>,
  /// How the picture is to be turned for display, as EXIF numbers it from 1 to 8.
  pub orientation: Option<u8>,
  pub color_model: ColorModel,
  pub color_space: ColorSpace,
  /// The name of the color profile the file carries.
  pub icc_profile: Option<String>,
  /// When the picture was taken, as the file writes it.
  pub exif_date_time_original: Option<String>,
  pub camera_make: Option<String>,
  pub camera_model: Option<String>,
  pub lens_model: Option<String>,
  pub gps_lat: Option<f64>,
  pub gps_lon: Option<f64>,
  /// The color profile the file carries, when it carries one that can be read.
  pub icc: Option<IccProfile>,
  /// The XMP packet the file carries.
  pub xmp: Option<Xmp>,
  pub detail: ImageDetail,
}

impl ImageFacts {
  /// Width over height. None for an image that states no size.
  pub fn aspect_ratio(&self) -> Option<f64> {
    (self.height != 0).then(|| f64::from(self.width) / f64::from(self.height))
  }

  /// The facts of an image of this size, with everything a format may add left unsaid.
  pub(crate) fn sized(width: u32, height: u32, detail: ImageDetail) -> Self {
    ImageFacts {
      width,
      height,
      frames: 1,
      animated: false,
      has_alpha: None,
      orientation: None,
      color_model: ColorModel::Unknown,
      color_space: ColorSpace::Unknown,
      icc_profile: None,
      exif_date_time_original: None,
      camera_make: None,
      camera_model: None,
      lens_model: None,
      gps_lat: None,
      gps_lon: None,
      icc: None,
      xmp: None,
      detail,
    }
  }

  /// Take what EXIF says of the picture.
  pub(crate) fn take_exif(&mut self, exif: Exif) {
    self.orientation = exif.orientation;
    self.exif_date_time_original = exif.date_time_original;
    self.camera_make = exif.make;
    self.camera_model = exif.model;
    self.lens_model = exif.lens_model;
    (self.gps_lat, self.gps_lon) = exif.position.unzip();
  }

  /// Take the color profile the file carries: its name, and the space it stands for when
  /// that can be told.
  pub(crate) fn take_profile(&mut self, profile: IccProfile) {
    self.icc_profile = Some(profile.name());
    let space = profile.color_space();
    if space != ColorSpace::Unknown {
      self.color_space = space;
    }
    self.icc = Some(profile);
  }
}

/// What a reader has to say beside the facts: what it lacked, and what was wrong.
#[derive(Debug, Default)]
pub(crate) struct Reading {
  /// The first range a reader wanted and was not given.
  pub needs: Option<ByteRange>,
  pub warnings: Vec<String>,
}

impl Reading {
  pub(crate) fn warn(&mut self, warning: impl Into<String>) {
    self.warnings.push(warning.into());
  }

  /// What a read returned, or nothing. A read that failed because its bytes were not given is
  /// what the answer then asks for; one that ran past the end of the file is a damaged file,
  /// and `cut` says where.
  pub(crate) fn take<T>(&mut self, read: Result<T, Missing>, cut: &str) -> Option<T> {
    match read {
      Ok(found) => Some(found),
      Err(Missing::NotGiven(range)) => {
        self.needs.get_or_insert(range);
        None
      }
      Err(Missing::PastEnd) => {
        self.warn(format!("the file ends {cut}"));
        None
      }
    }
  }
}
