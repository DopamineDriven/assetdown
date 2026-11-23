//! The JS boundary types: what `mimedown` answers, as plain objects and unions keyed on `type`.
//!
//! Each mirrors a type of the library and is built from it by `From`. A fact that was not read
//! is `null`. A union keyed on `type` holds what it carries under the key of the same name:
//! `{ type: 'image', image: … }`.

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::mime::Mime;

/// A count as a JS number. What is counted here (bytes, words, milliseconds) stays far below
/// the 2^53 at which a number stops being exact.
fn number(count: u64) -> f64 {
  count as f64
}

fn size(count: usize) -> u32 {
  u32::try_from(count).unwrap_or(u32::MAX)
}

// ── Naming ──────────────────────────────────────────────────────────────────────────────────

/// The word in front of the slash in a type's string, as the registry of media types has it.
#[napi(string_enum = "lowercase")]
pub enum TopLevel {
  Application,
  Audio,
  Font,
  Haptics,
  Image,
  Model,
  Text,
  Video,
}

impl From<mimedown::TopLevel> for TopLevel {
  fn from(top_level: mimedown::TopLevel) -> Self {
    match top_level {
      mimedown::TopLevel::Application => TopLevel::Application,
      mimedown::TopLevel::Audio => TopLevel::Audio,
      mimedown::TopLevel::Font => TopLevel::Font,
      mimedown::TopLevel::Haptics => TopLevel::Haptics,
      mimedown::TopLevel::Image => TopLevel::Image,
      mimedown::TopLevel::Model => TopLevel::Model,
      mimedown::TopLevel::Text => TopLevel::Text,
      mimedown::TopLevel::Video => TopLevel::Video,
    }
  }
}

/// What in the bytes named the type.
#[napi(string_enum = "camelCase")]
pub enum Evidence {
  /// Bytes at a fixed place in the file.
  Signature,
  /// What the file's container says it holds.
  Container,
  /// What the text says of itself.
  Content,
  /// The file's name, which is consulted for text and for nothing else.
  FileName,
}

impl From<mimedown::Evidence> for Evidence {
  fn from(evidence: mimedown::Evidence) -> Self {
    match evidence {
      mimedown::Evidence::Signature => Evidence::Signature,
      mimedown::Evidence::Container => Evidence::Container,
      mimedown::Evidence::Content => Evidence::Content,
      mimedown::Evidence::FileName => Evidence::FileName,
    }
  }
}

/// The package of this family that reads a type in full.
#[napi(string_enum = "lowercase")]
pub enum Parser {
  Pdfdown,
  Xlsxdown,
  Docxdown,
  Pptxdown,
  Audiodown,
  Videodown,
  Tiffdown,
  Jpegdown,
  Rtfdown,
  Xlsdown,
  Docdown,
}

impl From<mimedown::Parser> for Parser {
  fn from(parser: mimedown::Parser) -> Self {
    match parser {
      mimedown::Parser::Pdfdown => Parser::Pdfdown,
      mimedown::Parser::Xlsxdown => Parser::Xlsxdown,
      mimedown::Parser::Docxdown => Parser::Docxdown,
      mimedown::Parser::Pptxdown => Parser::Pptxdown,
      mimedown::Parser::Audiodown => Parser::Audiodown,
      mimedown::Parser::Videodown => Parser::Videodown,
      mimedown::Parser::Tiffdown => Parser::Tiffdown,
      mimedown::Parser::Jpegdown => Parser::Jpegdown,
      mimedown::Parser::Rtfdown => Parser::Rtfdown,
      mimedown::Parser::Xlsdown => Parser::Xlsdown,
      mimedown::Parser::Docdown => Parser::Docdown,
    }
  }
}

/// A range of a file.
#[napi(object, object_from_js = false)]
pub struct ByteRange {
  /// Where the range starts: from the start of the file, or back from its end.
  pub offset: f64,
  pub length: f64,
  /// `offset` counts back from the end of the file, which is how the end of a file is asked
  /// for when its size is not known.
  pub from_end: bool,
}

impl From<mimedown::ByteRange> for ByteRange {
  fn from(range: mimedown::ByteRange) -> Self {
    ByteRange {
      offset: number(range.offset),
      length: number(range.length),
      from_end: range.from_end,
    }
  }
}

/// The second look that named a type the first could not. It is taken only when asked for.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum Retry {
  /// The file's own start stands behind this many bytes that are not part of it.
  LeadingBytes { skipped: f64 },
  /// The end of the file says what its start does not.
  EndOfFile,
  /// Text read with more allowance.
  LenientText,
}

impl From<mimedown::Retry> for Retry {
  fn from(retry: mimedown::Retry) -> Self {
    match retry {
      mimedown::Retry::LeadingBytes { skipped } => Retry::LeadingBytes {
        skipped: number(skipped),
      },
      mimedown::Retry::EndOfFile => Retry::EndOfFile,
      mimedown::Retry::LenientText => Retry::LenientText,
    }
  }
}

/// What a caller can say beside the bytes. It is never the type.
#[napi(object, object_to_js = false)]
#[derive(Clone, Default)]
pub struct Options {
  /// The file's name. It is consulted for text and for nothing else.
  pub file_name: Option<String>,
  /// Look again when the first pass names nothing. Default `false`.
  pub retry_on_unknown: Option<bool>,
}

/// One piece of a file: its bytes, and where in the file they start.
#[napi(object, object_to_js = false)]
pub struct Piece {
  pub offset: f64,
  pub bytes: Uint8Array,
}

/// What a file is.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct Sniffed {
  /// `application/octet-stream` for bytes that name no type.
  pub mime: Mime,
  pub top_level: TopLevel,
  /// The usual extension for the type, without the dot.
  pub ext: String,
  pub parser: Option<Parser>,
  /// `null` for bytes that name no type.
  pub evidence: Option<Evidence>,
  pub retry: Option<Retry>,
  /// A range that would settle or complete the answer, when the bytes given stop short of it.
  pub needs: Option<ByteRange>,
}

impl From<mimedown::Sniffed> for Sniffed {
  fn from(sniffed: mimedown::Sniffed) -> Self {
    let (media_type, evidence, needs, retry) = match sniffed {
      mimedown::Sniffed::Named {
        media_type,
        evidence,
        needs,
        retry,
      } => (media_type, Some(evidence), needs, retry),
      mimedown::Sniffed::Unknown { needs } => (mimedown::MediaType::OctetStream, None, needs, None),
    };
    Sniffed {
      mime: media_type.into(),
      top_level: media_type.top_level().into(),
      ext: media_type.ext().to_string(),
      parser: media_type.parser().map(Into::into),
      evidence: evidence.map(Into::into),
      retry: retry.map(Into::into),
      needs: needs.map(Into::into),
    }
  }
}

/// What a file is, and what it says of itself.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct Specs {
  /// `application/octet-stream` for bytes that name no type.
  pub mime: Mime,
  pub top_level: TopLevel,
  /// The usual extension for the type, without the dot.
  pub ext: String,
  pub parser: Option<Parser>,
  /// `null` for bytes that name no type.
  pub evidence: Option<Evidence>,
  pub retry: Option<Retry>,
  /// A range that would complete the answer, when the bytes given stop short of it.
  pub needs: Option<ByteRange>,
  /// What was wrong with the file, or was not read, when it could still be answered for.
  pub warnings: Vec<String>,
  pub facts: Facts,
}

impl From<mimedown::Specs> for Specs {
  fn from(specs: mimedown::Specs) -> Self {
    let media_type = specs.media_type;
    Specs {
      mime: media_type.into(),
      top_level: media_type.top_level().into(),
      ext: media_type.ext().to_string(),
      parser: media_type.parser().map(Into::into),
      evidence: specs.evidence.map(Into::into),
      retry: specs.retry.map(Into::into),
      needs: specs.needs.map(Into::into),
      warnings: specs.warnings,
      facts: specs.facts.into(),
    }
  }
}

/// The facts of a file, by the kind of facts its type has.
// The variants differ a good deal in size and are not boxed: a field has to turn itself into a
// JS value, which a box does not. One of these is built for each answer and handed to JS.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
#[allow(clippy::large_enum_variant)]
pub enum Facts {
  /// The type is named and none of its facts are read.
  None,
  Image {
    image: ImageFacts,
  },
  Document {
    document: DocumentFacts,
  },
  Audio {
    audio: AudioFacts,
  },
  Video {
    video: VideoFacts,
  },
}

impl From<mimedown::Facts> for Facts {
  fn from(facts: mimedown::Facts) -> Self {
    match facts {
      mimedown::Facts::None => Facts::None,
      mimedown::Facts::Image(image) => Facts::Image {
        image: (*image).into(),
      },
      mimedown::Facts::Document(document) => Facts::Document {
        document: (*document).into(),
      },
      mimedown::Facts::Audio(audio) => Facts::Audio {
        audio: (*audio).into(),
      },
      mimedown::Facts::Video(video) => Facts::Video {
        video: (*video).into(),
      },
    }
  }
}

// ── Images ──────────────────────────────────────────────────────────────────────────────────

/// How an image's samples are laid out.
#[napi(string_enum)]
pub enum ColorModel {
  #[napi(value = "rgb")]
  Rgb,
  #[napi(value = "rgba")]
  Rgba,
  #[napi(value = "grayscale")]
  Grayscale,
  #[napi(value = "grayscale_alpha")]
  GrayscaleAlpha,
  #[napi(value = "indexed")]
  Indexed,
  #[napi(value = "cmyk")]
  Cmyk,
  #[napi(value = "ycbcr")]
  Ycbcr,
  #[napi(value = "ycck")]
  Ycck,
  #[napi(value = "vector")]
  Vector,
  #[napi(value = "lab")]
  Lab,
  #[napi(value = "unknown")]
  Unknown,
}

impl From<mimedown::ColorModel> for ColorModel {
  fn from(color_model: mimedown::ColorModel) -> Self {
    match color_model {
      mimedown::ColorModel::Rgb => ColorModel::Rgb,
      mimedown::ColorModel::Rgba => ColorModel::Rgba,
      mimedown::ColorModel::Grayscale => ColorModel::Grayscale,
      mimedown::ColorModel::GrayscaleAlpha => ColorModel::GrayscaleAlpha,
      mimedown::ColorModel::Indexed => ColorModel::Indexed,
      mimedown::ColorModel::Cmyk => ColorModel::Cmyk,
      mimedown::ColorModel::Ycbcr => ColorModel::Ycbcr,
      mimedown::ColorModel::Ycck => ColorModel::Ycck,
      mimedown::ColorModel::Vector => ColorModel::Vector,
      mimedown::ColorModel::Lab => ColorModel::Lab,
      mimedown::ColorModel::Unknown => ColorModel::Unknown,
    }
  }
}

/// Which colors an image's samples stand for.
#[napi(string_enum)]
pub enum ColorSpace {
  #[napi(value = "srgb")]
  Srgb,
  #[napi(value = "display_p3")]
  DisplayP3,
  #[napi(value = "adobe_rgb")]
  AdobeRgb,
  #[napi(value = "prophoto_rgb")]
  ProphotoRgb,
  #[napi(value = "rec2020")]
  Rec2020,
  #[napi(value = "rec709")]
  Rec709,
  #[napi(value = "cmyk")]
  Cmyk,
  #[napi(value = "lab")]
  Lab,
  #[napi(value = "xyz")]
  Xyz,
  #[napi(value = "gray")]
  Gray,
  #[napi(value = "unknown")]
  Unknown,
}

impl From<mimedown::ColorSpace> for ColorSpace {
  fn from(color_space: mimedown::ColorSpace) -> Self {
    match color_space {
      mimedown::ColorSpace::Srgb => ColorSpace::Srgb,
      mimedown::ColorSpace::DisplayP3 => ColorSpace::DisplayP3,
      mimedown::ColorSpace::AdobeRgb => ColorSpace::AdobeRgb,
      mimedown::ColorSpace::ProphotoRgb => ColorSpace::ProphotoRgb,
      mimedown::ColorSpace::Rec2020 => ColorSpace::Rec2020,
      mimedown::ColorSpace::Rec709 => ColorSpace::Rec709,
      mimedown::ColorSpace::Cmyk => ColorSpace::Cmyk,
      mimedown::ColorSpace::Lab => ColorSpace::Lab,
      mimedown::ColorSpace::Xyz => ColorSpace::Xyz,
      mimedown::ColorSpace::Gray => ColorSpace::Gray,
      mimedown::ColorSpace::Unknown => ColorSpace::Unknown,
    }
  }
}

/// Pixels to the inch, across and down.
#[napi(object, object_from_js = false)]
pub struct Density {
  pub x: f64,
  pub y: f64,
}

fn density(dpi: Option<(f64, f64)>) -> Option<Density> {
  dpi.map(|(x, y)| Density { x, y })
}

/// The color profile a file carries.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct IccProfile {
  pub description: Option<String>,
  pub version: String,
  /// What the profile is for: a display, a printer, a camera.
  pub class: String,
  pub data_color_space: String,
  pub connection_space: String,
  pub cmm: Option<String>,
  pub manufacturer: Option<String>,
  pub created: Option<String>,
}

impl From<mimedown::IccProfile> for IccProfile {
  fn from(profile: mimedown::IccProfile) -> Self {
    IccProfile {
      description: profile.description,
      version: profile.version,
      class: profile.class,
      data_color_space: profile.data_color_space,
      connection_space: profile.connection_space,
      cmm: profile.cmm,
      manufacturer: profile.manufacturer,
      created: profile.created,
    }
  }
}

/// The XMP packet a file carries: the fields of it that are read.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct Xmp {
  pub toolkit: Option<String>,
  pub creator_tool: Option<String>,
  pub create_date: Option<String>,
  pub modify_date: Option<String>,
  pub metadata_date: Option<String>,
  pub document_id: Option<String>,
  pub instance_id: Option<String>,
  pub original_document_id: Option<String>,
  pub creator: Option<String>,
  pub title: Option<String>,
  pub description: Option<String>,
  pub format: Option<String>,
  pub rights: Option<String>,
  pub color_mode: Option<String>,
  pub icc_profile: Option<String>,
  pub date_time_original: Option<String>,
  pub orientation: Option<String>,
  pub producer: Option<String>,
}

impl From<mimedown::Xmp> for Xmp {
  fn from(xmp: mimedown::Xmp) -> Self {
    Xmp {
      toolkit: xmp.toolkit,
      creator_tool: xmp.creator_tool,
      create_date: xmp.create_date,
      modify_date: xmp.modify_date,
      metadata_date: xmp.metadata_date,
      document_id: xmp.document_id,
      instance_id: xmp.instance_id,
      original_document_id: xmp.original_document_id,
      creator: xmp.creator,
      title: xmp.title,
      description: xmp.description,
      format: xmp.format,
      rights: xmp.rights,
      color_mode: xmp.color_mode,
      icc_profile: xmp.icc_profile,
      date_time_original: xmp.date_time_original,
      orientation: xmp.orientation,
      producer: xmp.producer,
    }
  }
}

/// What an image's header says of it. The fields are the columns of an image's metadata row.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct ImageFacts {
  pub width: u32,
  pub height: u32,
  /// How many pictures the file holds: an animation's frames, an icon's sizes.
  pub frames: u32,
  pub animated: bool,
  pub has_alpha: Option<bool>,
  /// How the picture is to be turned for display, as EXIF numbers it from 1 to 8.
  pub orientation: Option<u32>,
  /// Width over height. `null` for an image that states no size.
  pub aspect_ratio: Option<f64>,
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
  pub icc: Option<IccProfile>,
  pub xmp: Option<Xmp>,
  /// What only the image's own format says.
  pub detail: ImageDetail,
}

impl From<mimedown::ImageFacts> for ImageFacts {
  fn from(facts: mimedown::ImageFacts) -> Self {
    ImageFacts {
      width: facts.width,
      height: facts.height,
      frames: facts.frames,
      animated: facts.animated,
      has_alpha: facts.has_alpha,
      orientation: facts.orientation.map(u32::from),
      aspect_ratio: facts.aspect_ratio(),
      color_model: facts.color_model.into(),
      color_space: facts.color_space.into(),
      icc_profile: facts.icc_profile,
      exif_date_time_original: facts.exif_date_time_original,
      camera_make: facts.camera_make,
      camera_model: facts.camera_model,
      lens_model: facts.lens_model,
      gps_lat: facts.gps_lat,
      gps_lon: facts.gps_lon,
      icc: facts.icc.map(Into::into),
      xmp: facts.xmp.map(Into::into),
      detail: facts.detail.into(),
    }
  }
}

/// How a PNG's text is kept.
#[napi(string_enum = "lowercase")]
pub enum PngTextKind {
  Plain,
  Compressed,
  International,
}

#[napi(object, object_from_js = false)]
pub struct PngText {
  pub kind: PngTextKind,
  pub keyword: String,
  pub text: String,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct PngDetail {
  pub bit_depth: u32,
  pub color_type: u32,
  pub interlaced: bool,
  pub dpi: Option<Density>,
  pub pixel_aspect_ratio: Option<f64>,
  pub gamma: Option<f64>,
  /// How many times an animation plays, where 0 is without end.
  pub loop_count: Option<u32>,
  pub text: Vec<PngText>,
  pub modification_time: Option<String>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct GifDetail {
  pub version: String,
  pub loop_count: Option<u32>,
  pub duration_ms: f64,
  pub global_colors: Option<u32>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct BmpDetail {
  pub header_bytes: u32,
  pub bits_per_pixel: u32,
  pub compression: String,
  pub top_down: bool,
  pub dpi: Option<Density>,
}

#[napi(string_enum = "lowercase")]
pub enum IcoKind {
  Icon,
  Cursor,
}

/// Where a cursor's point is, from its top left corner.
#[napi(object, object_from_js = false)]
pub struct Hotspot {
  pub x: u32,
  pub y: u32,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct IcoEntry {
  pub width: u32,
  pub height: u32,
  pub bits_per_pixel: Option<u32>,
  /// The entry is a PNG and not a bitmap.
  pub png: bool,
  pub hotspot: Option<Hotspot>,
}

#[napi(object, object_from_js = false)]
pub struct IcoDetail {
  pub kind: IcoKind,
  pub entries: Vec<IcoEntry>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct JpegDetail {
  pub precision: u32,
  pub components: u32,
  pub progressive: bool,
  /// Every sample is exactly what was recorded.
  pub lossless: bool,
  pub dpi: Option<Density>,
  pub jfif: bool,
  pub adobe_transform: Option<u32>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct TiffDetail {
  pub big: bool,
  pub little_endian: bool,
  /// How many directories the file chains together.
  pub pages: u32,
  pub compression: Option<u32>,
  pub bits_per_sample: Vec<u32>,
  pub samples_per_pixel: u32,
  pub photometric: Option<u32>,
  pub dpi: Option<Density>,
  pub software: Option<String>,
  pub date_time: Option<String>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct WebpDetail {
  pub extended: bool,
  pub lossless: Option<bool>,
  pub loop_count: Option<u32>,
  pub duration_ms: f64,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct HeifDetail {
  pub codec: Option<String>,
  pub bit_depth: Option<u32>,
  /// The picture is kept as a grid of smaller ones.
  pub tiled: bool,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct SvgDetail {
  /// The width and height as the file writes them, units and all.
  pub width: Option<String>,
  pub height: Option<String>,
  /// Four numbers: where the drawing's own coordinates start, and how far they reach.
  pub view_box: Option<Vec<f64>>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct Jp2Detail {
  pub components: u32,
  pub bits_per_sample: Option<u32>,
  /// The number the format gives the picture's color space: 16 sRGB, 17 gray, 18 sYCC.
  pub enumerated_color: Option<u32>,
  /// The file is the picture's codestream alone, with no boxes around it.
  pub codestream: bool,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct JxlDetail {
  pub container: bool,
  pub bits_per_sample: u32,
  pub float_samples: bool,
  /// The picture is kept in the format's own color space, as nearly every file that lost
  /// something is.
  pub xyb: bool,
  /// How many times an animation plays, where 0 is without end.
  pub loops: Option<u32>,
  pub jpeg_reconstruction: bool,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct JxrDetail {
  pub version: u32,
  /// The last byte of the identifier of its pixel format.
  pub pixel_format: Option<u32>,
  pub bits_per_pixel: Option<u32>,
}

/// What the four bytes of a Radiance pixel are.
#[napi(string_enum = "lowercase")]
pub enum HdrEncoding {
  Rgbe,
  Xyze,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct HdrDetail {
  pub encoding: HdrEncoding,
  pub exposure: Option<f64>,
  pub software: Option<String>,
}

/// What only one image format says, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum ImageDetail {
  Png { png: PngDetail },
  Gif { gif: GifDetail },
  Bmp { bmp: BmpDetail },
  Ico { ico: IcoDetail },
  Jpeg { jpeg: JpegDetail },
  Tiff { tiff: TiffDetail },
  Webp { webp: WebpDetail },
  Heif { heif: HeifDetail },
  Svg { svg: SvgDetail },
  Jp2 { jp2: Jp2Detail },
  Jxl { jxl: JxlDetail },
  Jxr { jxr: JxrDetail },
  Hdr { hdr: HdrDetail },
}

impl From<mimedown::ImageDetail> for ImageDetail {
  fn from(detail: mimedown::ImageDetail) -> Self {
    match detail {
      mimedown::ImageDetail::Png(png) => ImageDetail::Png {
        png: PngDetail {
          bit_depth: u32::from(png.bit_depth),
          color_type: u32::from(png.color_type),
          interlaced: png.interlaced,
          dpi: density(png.dpi),
          pixel_aspect_ratio: png.pixel_aspect_ratio,
          gamma: png.gamma,
          loop_count: png.loop_count,
          text: png
            .text
            .into_iter()
            .map(|text| PngText {
              kind: match text.kind {
                mimedown::PngTextKind::Plain => PngTextKind::Plain,
                mimedown::PngTextKind::Compressed => PngTextKind::Compressed,
                mimedown::PngTextKind::International => PngTextKind::International,
              },
              keyword: text.keyword,
              text: text.text,
            })
            .collect(),
          modification_time: png.modification_time,
        },
      },
      mimedown::ImageDetail::Gif(gif) => ImageDetail::Gif {
        gif: GifDetail {
          version: gif.version,
          loop_count: gif.loop_count.map(u32::from),
          duration_ms: number(gif.duration_ms),
          global_colors: gif.global_colors.map(u32::from),
        },
      },
      mimedown::ImageDetail::Bmp(bmp) => ImageDetail::Bmp {
        bmp: BmpDetail {
          header_bytes: bmp.header_bytes,
          bits_per_pixel: u32::from(bmp.bits_per_pixel),
          compression: bmp.compression,
          top_down: bmp.top_down,
          dpi: density(bmp.dpi),
        },
      },
      mimedown::ImageDetail::Ico(ico) => ImageDetail::Ico {
        ico: IcoDetail {
          kind: match ico.kind {
            mimedown::IcoKind::Icon => IcoKind::Icon,
            mimedown::IcoKind::Cursor => IcoKind::Cursor,
          },
          entries: ico
            .entries
            .into_iter()
            .map(|entry| IcoEntry {
              width: entry.width,
              height: entry.height,
              bits_per_pixel: entry.bits_per_pixel.map(u32::from),
              png: entry.png,
              hotspot: entry.hotspot.map(|(x, y)| Hotspot {
                x: u32::from(x),
                y: u32::from(y),
              }),
            })
            .collect(),
        },
      },
      mimedown::ImageDetail::Jpeg(jpeg) => ImageDetail::Jpeg {
        jpeg: JpegDetail {
          precision: u32::from(jpeg.precision),
          components: u32::from(jpeg.components),
          progressive: jpeg.progressive,
          lossless: jpeg.lossless,
          dpi: density(jpeg.dpi),
          jfif: jpeg.jfif,
          adobe_transform: jpeg.adobe_transform.map(u32::from),
        },
      },
      mimedown::ImageDetail::Tiff(tiff) => ImageDetail::Tiff {
        tiff: TiffDetail {
          big: tiff.big,
          little_endian: tiff.little_endian,
          pages: tiff.pages,
          compression: tiff.compression.map(u32::from),
          bits_per_sample: tiff.bits_per_sample.into_iter().map(u32::from).collect(),
          samples_per_pixel: u32::from(tiff.samples_per_pixel),
          photometric: tiff.photometric.map(u32::from),
          dpi: density(tiff.dpi),
          software: tiff.software,
          date_time: tiff.date_time,
        },
      },
      mimedown::ImageDetail::Webp(webp) => ImageDetail::Webp {
        webp: WebpDetail {
          extended: webp.extended,
          lossless: webp.lossless,
          loop_count: webp.loop_count.map(u32::from),
          duration_ms: number(webp.duration_ms),
        },
      },
      mimedown::ImageDetail::Heif(heif) => ImageDetail::Heif {
        heif: HeifDetail {
          codec: heif.codec,
          bit_depth: heif.bit_depth.map(u32::from),
          tiled: heif.tiled,
        },
      },
      mimedown::ImageDetail::Svg(svg) => ImageDetail::Svg {
        svg: SvgDetail {
          width: svg.width,
          height: svg.height,
          view_box: svg.view_box.map(Vec::from),
        },
      },
      mimedown::ImageDetail::Jp2(jp2) => ImageDetail::Jp2 {
        jp2: Jp2Detail {
          components: u32::from(jp2.components),
          bits_per_sample: jp2.bits_per_sample.map(u32::from),
          enumerated_color: jp2.enumerated_color,
          codestream: jp2.codestream,
        },
      },
      mimedown::ImageDetail::Jxl(jxl) => ImageDetail::Jxl {
        jxl: JxlDetail {
          container: jxl.container,
          bits_per_sample: u32::from(jxl.bits_per_sample),
          float_samples: jxl.float_samples,
          xyb: jxl.xyb,
          loops: jxl.loops,
          jpeg_reconstruction: jxl.jpeg_reconstruction,
        },
      },
      mimedown::ImageDetail::Jxr(jxr) => ImageDetail::Jxr {
        jxr: JxrDetail {
          version: u32::from(jxr.version),
          pixel_format: jxr.pixel_format.map(u32::from),
          bits_per_pixel: jxr.bits_per_pixel.map(u32::from),
        },
      },
      mimedown::ImageDetail::Hdr(hdr) => ImageDetail::Hdr {
        hdr: HdrDetail {
          encoding: match hdr.encoding {
            mimedown::HdrEncoding::Rgbe => HdrEncoding::Rgbe,
            mimedown::HdrEncoding::Xyze => HdrEncoding::Xyze,
          },
          exposure: hdr.exposure,
          software: hdr.software,
        },
      },
    }
  }
}

// ── Documents ───────────────────────────────────────────────────────────────────────────────

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct PdfDetail {
  pub version: Option<String>,
  pub is_linearized: Option<bool>,
  pub is_encrypted: Option<bool>,
  /// At least one page has text of its own, which a scan has not.
  pub is_searchable: Option<bool>,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub has_form: Option<bool>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct OfficeDetail {
  /// The file can carry macros. They are never read or run.
  pub macro_enabled: bool,
  pub template: bool,
  pub application: Option<String>,
}

#[napi(object, object_from_js = false)]
pub struct TextDetail {
  pub encoding: String,
  pub line_count: f64,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct RtfDetail {
  pub code_page: Option<u32>,
  pub generator: Option<String>,
}

/// What only one kind of document says, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum DocumentDetail {
  Pdf { pdf: PdfDetail },
  Office { office: OfficeDetail },
  Text { text: TextDetail },
  Rtf { rtf: RtfDetail },
}

impl From<mimedown::DocumentDetail> for DocumentDetail {
  fn from(detail: mimedown::DocumentDetail) -> Self {
    match detail {
      mimedown::DocumentDetail::Pdf(pdf) => DocumentDetail::Pdf {
        pdf: PdfDetail {
          version: pdf.version,
          is_linearized: pdf.is_linearized,
          is_encrypted: pdf.is_encrypted,
          is_searchable: pdf.is_searchable,
          creator: pdf.creator,
          producer: pdf.producer,
          has_form: pdf.has_form,
        },
      },
      mimedown::DocumentDetail::Office(office) => DocumentDetail::Office {
        office: OfficeDetail {
          macro_enabled: office.macro_enabled,
          template: office.template,
          application: office.application,
        },
      },
      mimedown::DocumentDetail::Text(text) => DocumentDetail::Text {
        text: TextDetail {
          encoding: text.encoding,
          line_count: number(text.line_count),
        },
      },
      mimedown::DocumentDetail::Rtf(rtf) => DocumentDetail::Rtf {
        rtf: RtfDetail {
          code_page: rtf.code_page,
          generator: rtf.generator,
        },
      },
    }
  }
}

/// What a document says of itself. The fields are the columns of a document's metadata row.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct DocumentFacts {
  pub page_count: Option<u32>,
  pub word_count: Option<f64>,
  pub language: Option<String>,
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  pub keywords: Vec<String>,
  /// The start of the document's text.
  pub text_preview: Option<String>,
  /// The size of the page most pages share, in points.
  pub page_width: Option<f64>,
  pub page_height: Option<f64>,
  /// The standard paper size that page is nearest to: `A4`, `LETTER`, `UNKNOWN`.
  pub paper_size: Option<String>,
  pub page_size_uniform: Option<bool>,
  pub image_count: Option<u32>,
  pub annot_count: Option<u32>,
  /// When the document was made and last changed, as the file writes them.
  pub created_date: Option<String>,
  pub modified_date: Option<String>,
  /// What only the document's own kind says.
  pub detail: DocumentDetail,
}

impl From<mimedown::DocumentFacts> for DocumentFacts {
  fn from(facts: mimedown::DocumentFacts) -> Self {
    DocumentFacts {
      page_count: facts.page_count,
      word_count: facts.word_count.map(number),
      language: facts.language,
      title: facts.title,
      author: facts.author,
      subject: facts.subject,
      keywords: facts.keywords,
      text_preview: facts.text_preview,
      page_width: facts.page_width,
      page_height: facts.page_height,
      paper_size: facts.paper_size,
      page_size_uniform: facts.page_size_uniform,
      image_count: facts.image_count,
      annot_count: facts.annot_count,
      created_date: facts.created_date,
      modified_date: facts.modified_date,
      detail: facts.detail.into(),
    }
  }
}

// ── Sound and film ──────────────────────────────────────────────────────────────────────────

/// What an audio file says of itself.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct AudioFacts {
  pub duration_ms: Option<f64>,
  pub bitrate_kbps: Option<u32>,
  pub sample_rate: Option<u32>,
  pub channels: Option<u32>,
  pub codec: Option<String>,
  pub title: Option<String>,
  pub artist: Option<String>,
  pub album: Option<String>,
  pub year: Option<u32>,
  pub genre: Option<String>,
}

impl From<mimedown::AudioFacts> for AudioFacts {
  fn from(facts: mimedown::AudioFacts) -> Self {
    AudioFacts {
      duration_ms: facts.duration_ms.map(number),
      bitrate_kbps: facts.bitrate_kbps,
      sample_rate: facts.sample_rate,
      channels: facts.channels,
      codec: facts.codec,
      title: facts.title,
      artist: facts.artist,
      album: facts.album,
      year: facts.year.map(u32::from),
      genre: facts.genre,
    }
  }
}

/// What a video file says of itself.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct VideoFacts {
  /// The size the picture is shown at, once it is turned and its pixels are made square.
  pub width: u32,
  pub height: u32,
  pub duration_ms: Option<f64>,
  pub frame_rate: Option<f64>,
  pub bitrate_kbps: Option<u32>,
  pub codec: Option<String>,
  pub has_audio: bool,
  /// `1080p`, `720p`, `4K`: the name of the shorter side.
  pub resolution: Option<String>,
  /// `landscape`, `portrait` or `square`.
  pub orientation: Option<String>,
  /// How far the picture is turned for display, in degrees.
  pub rotation: Option<u32>,
  pub aspect_ratio: Option<f64>,
  pub keyframe_times_ms: Vec<f64>,
}

impl From<mimedown::VideoFacts> for VideoFacts {
  fn from(facts: mimedown::VideoFacts) -> Self {
    VideoFacts {
      width: facts.width,
      height: facts.height,
      duration_ms: facts.duration_ms.map(number),
      frame_rate: facts.frame_rate,
      bitrate_kbps: facts.bitrate_kbps,
      aspect_ratio: facts.aspect_ratio(),
      codec: facts.codec,
      has_audio: facts.has_audio,
      resolution: facts.resolution,
      orientation: facts.orientation,
      rotation: facts.rotation,
      keyframe_times_ms: facts.keyframe_times_ms,
    }
  }
}

// ── What takes more than a header ───────────────────────────────────────────────────────────

/// Why a verb gave nothing, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum WhyNot {
  /// No reader here gives this for a file of this type: a picture has no waveform.
  NoReader { mime: Mime },
  /// A file of this type has it, and the addon was built without the feature that reads it.
  FeatureOff { feature: String },
  /// The file holds none: a film with no sound has no waveform.
  Absent,
  /// It is there, in a coding that is not decoded here.
  Unsupported { coding: String },
  /// Only the start of the file was given.
  Truncated,
  /// The file could not be read.
  Failed { reason: String },
}

impl From<mimedown::WhyNot> for WhyNot {
  fn from(why: mimedown::WhyNot) -> Self {
    match why {
      mimedown::WhyNot::NoReader(media_type) => WhyNot::NoReader {
        mime: media_type.into(),
      },
      mimedown::WhyNot::FeatureOff(feature) => WhyNot::FeatureOff {
        feature: feature.to_string(),
      },
      mimedown::WhyNot::Absent => WhyNot::Absent,
      mimedown::WhyNot::Unsupported(coding) => WhyNot::Unsupported { coding },
      mimedown::WhyNot::Truncated => WhyNot::Truncated,
      mimedown::WhyNot::Failed(reason) => WhyNot::Failed { reason },
    }
  }
}

/// How a waveform is made.
#[napi(object, object_to_js = false)]
#[derive(Clone, Default)]
pub struct WaveformOptions {
  /// How many peaks to cut the sound into. Default `1024`. Not with `samplesPerPeak`.
  pub peak_count: Option<f64>,
  /// How many frames go to a peak. Not with `peakCount`.
  pub samples_per_peak: Option<f64>,
  /// Leave off the silence an encoder pads the sound with. Default `true`.
  pub gapless: Option<bool>,
  /// Stop at the first packet that does not decode. Default `true`.
  pub strict: Option<bool>,
  /// The most samples, over all channels, held in memory. Default `64000000`.
  pub max_decoded_samples: Option<f64>,
  /// Spread the work over the machine's cores. Default `true`.
  pub parallel: Option<bool>,
}

/// One channel of a waveform, peak by peak.
#[napi(object, object_from_js = false)]
pub struct WaveformChannel {
  pub min: Float32Array,
  pub max: Float32Array,
  /// The larger of `|min|` and `|max|`.
  pub peak: Float32Array,
  pub rms: Float64Array,
}

/// The shape of a file's sound.
#[napi(object, object_from_js = false)]
pub struct Waveform {
  pub sample_rate: u32,
  pub channel_count: u32,
  /// Samples in each channel.
  pub frame_count: f64,
  pub duration_ms: f64,
  pub peak_count: u32,
  /// One more than `peakCount` frame offsets: peak `i` covers the frames from
  /// `boundaries[i]` up to `boundaries[i + 1]`.
  pub boundaries: Uint32Array,
  pub channels: Vec<WaveformChannel>,
  pub gapless: bool,
  pub skipped_packets: u32,
}

impl From<mimedown::Waveform> for Waveform {
  fn from(shape: mimedown::Waveform) -> Self {
    Waveform {
      sample_rate: shape.sample_rate,
      channel_count: size(shape.channel_count),
      frame_count: number(u64::try_from(shape.frame_count).unwrap_or(u64::MAX)),
      duration_ms: shape.duration_ms,
      peak_count: size(shape.boundaries.len().saturating_sub(1)),
      boundaries: Uint32Array::new(shape.boundaries),
      channels: shape
        .channels
        .into_iter()
        .map(|channel| WaveformChannel {
          min: Float32Array::new(channel.min),
          max: Float32Array::new(channel.max),
          peak: Float32Array::new(channel.peak),
          rms: Float64Array::new(channel.rms),
        })
        .collect(),
      gapless: shape.gapless,
      skipped_packets: shape.skipped_packets,
    }
  }
}

/// What asking for a waveform gave, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum WaveformOutcome {
  Made { waveform: Waveform },
  Nothing { why: WhyNot },
}

impl From<mimedown::Outcome<mimedown::Waveform>> for WaveformOutcome {
  fn from(outcome: mimedown::Outcome<mimedown::Waveform>) -> Self {
    match outcome {
      mimedown::Outcome::Made(waveform) => WaveformOutcome::Made {
        waveform: waveform.into(),
      },
      mimedown::Outcome::Nothing(why) => WaveformOutcome::Nothing { why: why.into() },
    }
  }
}

/// The image a poster is written as.
#[napi(string_enum = "lowercase")]
#[derive(Clone, Copy)]
pub enum PosterFormat {
  Jpeg,
  Png,
  /// WebP that loses nothing.
  Webp,
}

/// How a poster is made.
#[napi(object, object_to_js = false)]
#[derive(Clone, Default)]
pub struct PosterOptions {
  /// The keyframe at or before this many milliseconds is taken. Default: the first one.
  pub at_ms: Option<f64>,
  /// Default `jpeg`.
  pub format: Option<PosterFormat>,
  /// From 1 to 100, for a JPEG. Default `90`.
  pub quality: Option<f64>,
}

/// A still from a film, at the size the film is shown at.
#[napi(object, object_from_js = false)]
pub struct Poster {
  /// The type of `data`.
  pub mime: Mime,
  pub width: u32,
  pub height: u32,
  /// When the keyframe it was made from is shown, in milliseconds from the start.
  pub time_ms: f64,
  pub data: Uint8Array,
}

/// What asking for a poster gave, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum PosterOutcome {
  Made { poster: Poster },
  Nothing { why: WhyNot },
}

impl From<mimedown::Outcome<mimedown::Poster>> for PosterOutcome {
  fn from(outcome: mimedown::Outcome<mimedown::Poster>) -> Self {
    match outcome {
      mimedown::Outcome::Made(poster) => PosterOutcome::Made {
        poster: Poster {
          mime: poster.media_type.into(),
          width: poster.width,
          height: poster.height,
          time_ms: poster.time_ms,
          data: Uint8Array::new(poster.data),
        },
      },
      mimedown::Outcome::Nothing(why) => PosterOutcome::Nothing { why: why.into() },
    }
  }
}

/// Where a film's keyframes are.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct Keyframes {
  /// When each is shown, in milliseconds from the start, in order.
  pub times_ms: Vec<f64>,
  /// What the list was read from: `stss`, `all-samples`, `cues`, `blocks`.
  pub source: String,
  pub sample_count: Option<f64>,
  /// The list covers the whole film.
  pub complete: bool,
}

/// What asking for a film's keyframes gave, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase", object_from_js = false)]
pub enum KeyframesOutcome {
  Made { keyframes: Keyframes },
  Nothing { why: WhyNot },
}

impl From<mimedown::Outcome<mimedown::Keyframes>> for KeyframesOutcome {
  fn from(outcome: mimedown::Outcome<mimedown::Keyframes>) -> Self {
    match outcome {
      mimedown::Outcome::Made(keyframes) => KeyframesOutcome::Made {
        keyframes: Keyframes {
          times_ms: keyframes.times_ms,
          source: keyframes.source,
          sample_count: keyframes.sample_count.map(number),
          complete: keyframes.complete,
        },
      },
      mimedown::Outcome::Nothing(why) => KeyframesOutcome::Nothing { why: why.into() },
    }
  }
}

/// Which of the parsers the addon was built with.
#[napi(object, object_from_js = false)]
pub struct BuiltWith {
  pub pdf: bool,
  pub office: bool,
  pub audio: bool,
  pub video: bool,
}
