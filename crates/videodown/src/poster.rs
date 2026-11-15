//! A still picture of the video: one keyframe, decoded, turned upright and encoded as an image.
//!
//! A keyframe is the one kind of frame that decodes on its own, so a poster needs no more of the
//! stream than that frame and the track's decoder configuration. `mp4` and `mkv` find those bytes;
//! `rust_h264` and `rust_h265` decode them to YCbCr planes; this module turns the planes into RGB
//! at the size the video is shown at and hands them to the `image` crate to encode.

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbImage, imageops};

use super::sniff::sniff;
use super::{mkv, mp4};

/// Said when the buffer stops before the bytes a poster needs.
pub(crate) const PREFIX: &str =
  "buffer ends before the keyframe; fetch more of the file before making its poster";

/// The image format of a poster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PosterFormat {
  #[default]
  Jpeg,
  Png,
  /// Lossless WebP: the `image` crate writes no other kind.
  Webp,
}

impl PosterFormat {
  pub fn mime(self) -> &'static str {
    match self {
      PosterFormat::Jpeg => "image/jpeg",
      PosterFormat::Png => "image/png",
      PosterFormat::Webp => "image/webp",
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub struct PosterConfig {
  /// Take the keyframe at or before this time; the first keyframe when `None`.
  pub time_sec: Option<f64>,
  pub format: PosterFormat,
  /// JPEG quality, 1 to 100. PNG and WebP are lossless and ignore it.
  pub quality: u8,
}

pub const DEFAULT_QUALITY: u8 = 90;

impl Default for PosterConfig {
  fn default() -> Self {
    PosterConfig {
      time_sec: None,
      format: PosterFormat::default(),
      quality: DEFAULT_QUALITY,
    }
  }
}

/// An encoded poster, at the size the video is shown at: rotation applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Poster {
  pub format: PosterFormat,
  pub width: u32,
  pub height: u32,
  /// The presentation time of the keyframe it was made from.
  pub time_sec: f64,
  pub data: Vec<u8>,
}

/// What asking for a poster produced, so a missing poster is never confused with a failure.
#[derive(Debug)]
pub enum PosterOutcome {
  Poster(Poster),
  /// The container has no video track.
  NoVideo,
  /// There is a video track, but its codec has no decoder here. Carries the codec's name.
  Unsupported(String),
}

/// The codecs a keyframe can be decoded from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Codec {
  H264,
  Hevc,
}

/// The compressed bytes of one keyframe with what is needed to decode and place it.
pub(crate) struct Keyframe<'a> {
  pub codec: Codec,
  /// The track's `avcC` or `hvcC` record: parameter sets and the size of each length prefix.
  pub config: &'a [u8],
  /// The frame itself, as length-prefixed NAL units.
  pub sample: &'a [u8],
  pub time_sec: f64,
  /// Clockwise quarter turns to show the frame upright, in degrees.
  pub rotation: u32,
  /// The color description the container gives, when it gives one.
  pub matrix: Option<u16>,
  pub full_range: Option<bool>,
}

/// What a container yields when asked for a keyframe.
pub(crate) enum Located<'a> {
  Keyframe(Keyframe<'a>),
  NoVideo,
  Unsupported(String),
}

/// The entry to show for `time_sec`: the last one at or before it, or the first when none is
/// that early or no time was asked for. `entries` are in presentation order.
pub(crate) fn at_or_before<T: Copy>(
  entries: &[(T, f64)],
  time_sec: Option<f64>,
) -> Option<(T, f64)> {
  let first = entries.first().copied()?;
  let Some(time) = time_sec else {
    return Some(first);
  };
  Some(
    entries
      .iter()
      .copied()
      .take_while(|(_, at)| *at <= time)
      .last()
      .unwrap_or(first),
  )
}

/// A decoded picture: 8-bit planes, the chroma ones at whatever subsampling the stream uses.
struct Picture {
  width: u32,
  height: u32,
  y: Vec<u8>,
  u: Vec<u8>,
  v: Vec<u8>,
  chroma_width: u32,
  chroma_height: u32,
  /// Color description from the bitstream, when the decoder reports it.
  matrix: Option<u16>,
  full_range: Option<bool>,
}

/// Brings samples of any depth down to eight bits.
fn to_eight_bits(samples: impl Iterator<Item = u16>, bit_depth: u8) -> Vec<u8> {
  let shift = u32::from(bit_depth.saturating_sub(8));
  samples
    .map(|sample| u8::try_from(sample >> shift).unwrap_or(u8::MAX))
    .collect()
}

impl Picture {
  /// Works out the chroma subsampling from the size of the planes: 4:2:0, 4:2:2 or 4:4:4. A
  /// picture with no chroma at all is gray.
  fn new(width: u32, height: u32, y: Vec<u8>, u: Vec<u8>, v: Vec<u8>) -> Result<Self, String> {
    let luma = usize::try_from(u64::from(width) * u64::from(height))
      .map_err(|_| "picture too large".to_owned())?;
    if width == 0 || height == 0 || y.len() != luma {
      return Err("decoder returned a picture whose luma plane does not match its size".into());
    }
    let half = |side: u32| side.div_ceil(2);
    let layouts = [
      (half(width), half(height)),
      (half(width), height),
      (width, height),
    ];
    let chroma = if u.is_empty() && v.is_empty() {
      Some((0, 0))
    } else {
      layouts.into_iter().find(|&(w, h)| {
        let len = u64::from(w) * u64::from(h);
        u64::try_from(u.len()).is_ok_and(|n| n == len) && u.len() == v.len()
      })
    };
    let (chroma_width, chroma_height) =
      chroma.ok_or("decoder returned chroma planes of an unknown layout")?;
    Ok(Picture {
      width,
      height,
      y,
      u,
      v,
      chroma_width,
      chroma_height,
      matrix: None,
      full_range: None,
    })
  }
}

fn decode_h264(config: &[u8], sample: &[u8]) -> Result<Picture, String> {
  use rust_h264::decoder::OrderedDecoder;
  use rust_h264::nal::{parse_avcc, parse_avcc_config, parse_sps_info};

  let record =
    parse_avcc_config(config).map_err(|e| format!("unreadable H.264 configuration: {e}"))?;
  let color = record
    .sps_nals
    .first()
    .and_then(|sps| parse_sps_info(sps).ok())
    .map(|info| (u16::from(info.matrix), info.full_range));
  let mut decoder = OrderedDecoder::new();
  let mut frames = Vec::new();
  for nal in record.sps_nals.iter().chain(&record.pps_nals) {
    decoder
      .decode_nal(nal)
      .map_err(|e| format!("H.264 parameter set rejected: {e:?}"))?;
  }
  for nal in &parse_avcc(sample, record.length_size) {
    let ready = decoder
      .decode_nal(nal)
      .map_err(|e| format!("H.264 keyframe failed to decode: {e:?}"))?;
    frames.extend(ready);
  }
  frames.extend(decoder.flush());
  let frame = frames
    .into_iter()
    .next()
    .ok_or("H.264 keyframe produced no picture")?;
  // Above eight bits every sample is two bytes, little-endian.
  let depth = frame.bit_depth;
  let plane = |bytes: Vec<u8>| {
    if depth > 8 {
      to_eight_bits(
        bytes
          .as_chunks::<2>()
          .0
          .iter()
          .map(|pair| u16::from_le_bytes(*pair)),
        depth,
      )
    } else {
      bytes
    }
  };
  let mut picture = Picture::new(
    frame.width,
    frame.height,
    plane(frame.y),
    plane(frame.u),
    plane(frame.v),
  )?;
  picture.matrix = color.map(|(matrix, _)| matrix);
  picture.full_range = color.map(|(_, full_range)| full_range);
  Ok(picture)
}

/// The parameter sets of an `hvcC` record and the size of the length prefix its samples use.
///
/// The record is 22 bytes of profile and level fields, whose last byte carries the prefix size,
/// then a count of arrays; each array is a type byte, a count, and that many length-prefixed NAL
/// units.
fn hevc_parameter_sets(config: &[u8]) -> Option<(u8, Vec<&[u8]>)> {
  let length_size = (config.get(21)? & 0x03) + 1;
  let arrays = *config.get(22)?;
  let mut at = 23usize;
  let mut sets = Vec::new();
  for _ in 0..arrays {
    let count = u16::from_be_bytes([*config.get(at + 1)?, *config.get(at + 2)?]);
    at += 3;
    for _ in 0..count {
      let len = usize::from(u16::from_be_bytes([*config.get(at)?, *config.get(at + 1)?]));
      at += 2;
      sets.push(config.get(at..at + len)?);
      at += len;
    }
  }
  Some((length_size, sets))
}

fn decode_hevc(config: &[u8], sample: &[u8]) -> Result<Picture, String> {
  use rust_h265::{Decoder, PixelData, parse_hvcc, parse_nal};

  let (length_size, sets) = hevc_parameter_sets(config).ok_or("unreadable HEVC configuration")?;
  let mut decoder = Decoder::new().with_display_order();
  let mut frames = Vec::new();
  for set in sets {
    let nal = parse_nal(set).ok_or("unreadable HEVC parameter set")?;
    decoder
      .decode_nal(&nal)
      .map_err(|e| format!("HEVC parameter set rejected: {e:?}"))?;
  }
  for nal in parse_hvcc(sample, length_size) {
    let ready = decoder
      .decode_nal(&nal)
      .map_err(|e| format!("HEVC keyframe failed to decode: {e:?}"))?;
    frames.extend(ready);
    while let Some(frame) = decoder.next_output() {
      frames.push(frame);
    }
  }
  while let Some(frame) = decoder.flush() {
    frames.push(frame);
  }
  let frame = frames
    .into_iter()
    .next()
    .ok_or("HEVC keyframe produced no picture")?;
  let depth = frame.bit_depth;
  let plane = |data: PixelData| match data {
    PixelData::U8(bytes) => bytes,
    PixelData::U16(samples) => to_eight_bits(samples.into_iter(), depth),
  };
  Picture::new(
    frame.width,
    frame.height,
    plane(frame.y),
    plane(frame.u),
    plane(frame.v),
  )
}

/// The weights of red and blue in luma for a matrix code point of ITU-T H.273. An unspecified
/// matrix follows what players assume: BT.709 for high definition, BT.601 below it.
fn luma_weights(matrix: Option<u16>, width: u32, height: u32) -> (f32, f32) {
  const BT601: (f32, f32) = (0.299, 0.114);
  const BT709: (f32, f32) = (0.2126, 0.0722);
  const BT2020: (f32, f32) = (0.2627, 0.0593);
  match matrix {
    Some(1) => BT709,
    Some(5 | 6) => BT601,
    Some(9 | 10) => BT2020,
    _ if width >= 1280 || height > 576 => BT709,
    _ => BT601,
  }
}

fn to_rgb(picture: &Picture, matrix: Option<u16>, full_range: bool) -> Result<RgbImage, String> {
  let (kr, kb) = luma_weights(matrix, picture.width, picture.height);
  let kg = 1.0 - kr - kb;
  // Limited range puts black at 16 and white at 235, and chroma within 16 to 240.
  let (luma_offset, luma_scale, chroma_scale) = if full_range {
    (0.0, 1.0, 1.0)
  } else {
    (16.0, 255.0 / 219.0, 255.0 / 224.0)
  };
  let side = |value: u32| usize::try_from(value).map_err(|_| "picture too large".to_owned());
  let (width, height) = (side(picture.width)?, side(picture.height)?);
  let (chroma_width, chroma_height) = (side(picture.chroma_width)?, side(picture.chroma_height)?);
  let mut rgb = vec![0u8; width * height * 3];
  for (row, out) in rgb.chunks_exact_mut(width * 3).enumerate() {
    let luma = &picture.y[row * width..(row + 1) * width];
    // The chroma row and column that cover this pixel; each chroma sample is repeated over the
    // pixels it covers.
    let chroma_row = (chroma_height > 0).then(|| row * chroma_height / height);
    for (column, pixel) in out.as_chunks_mut::<3>().0.iter_mut().enumerate() {
      let (cb, cr) = match chroma_row {
        Some(chroma_row) => {
          let at = chroma_row * chroma_width + column * chroma_width / width;
          (
            (f32::from(picture.u[at]) - 128.0) * chroma_scale,
            (f32::from(picture.v[at]) - 128.0) * chroma_scale,
          )
        }
        None => (0.0, 0.0),
      };
      let y = (f32::from(luma[column]) - luma_offset) * luma_scale;
      let red = y + 2.0 * (1.0 - kr) * cr;
      let blue = y + 2.0 * (1.0 - kb) * cb;
      let green = (y - kr * red - kb * blue) / kg;
      // `as` saturates a float into the range of the integer, which is the clamp wanted here.
      *pixel = [red.round() as u8, green.round() as u8, blue.round() as u8];
    }
  }
  RgbImage::from_raw(picture.width, picture.height, rgb)
    .ok_or_else(|| "picture does not fit an image buffer".to_owned())
}

fn encode(image: &RgbImage, format: PosterFormat, quality: u8) -> Result<Vec<u8>, String> {
  let mut out = Vec::new();
  let (width, height) = image.dimensions();
  let written = match format {
    PosterFormat::Jpeg => JpegEncoder::new_with_quality(&mut out, quality).encode(
      image.as_raw(),
      width,
      height,
      ExtendedColorType::Rgb8,
    ),
    PosterFormat::Png => {
      PngEncoder::new(&mut out).write_image(image.as_raw(), width, height, ExtendedColorType::Rgb8)
    }
    PosterFormat::Webp => WebPEncoder::new_lossless(&mut out).write_image(
      image.as_raw(),
      width,
      height,
      ExtendedColorType::Rgb8,
    ),
  };
  written.map_err(|e| format!("poster failed to encode: {e}"))?;
  Ok(out)
}

/// One keyframe of the video as an image at the size the video is shown at.
pub fn poster(data: &[u8], config: PosterConfig) -> Result<PosterOutcome, String> {
  if config.time_sec.is_some_and(|t| !t.is_finite() || t < 0.0) {
    return Err("timeSec must be a finite number of seconds, zero or more".into());
  }
  if !(1..=100).contains(&config.quality) {
    return Err("quality must be an integer in 1..=100".into());
  }
  let kind = sniff(data).ok_or("unrecognized video magic")?;
  let located = if kind.is_isobmff() {
    mp4::keyframe(data, config.time_sec)?
  } else {
    mkv::keyframe(data, config.time_sec)?
  };
  let keyframe = match located {
    Located::Keyframe(keyframe) => keyframe,
    Located::NoVideo => return Ok(PosterOutcome::NoVideo),
    Located::Unsupported(codec) => return Ok(PosterOutcome::Unsupported(codec)),
  };
  let picture = match keyframe.codec {
    Codec::H264 => decode_h264(keyframe.config, keyframe.sample)?,
    Codec::Hevc => decode_hevc(keyframe.config, keyframe.sample)?,
  };
  // What the stream says about its own color wins over what the container says about it.
  let matrix = picture.matrix.filter(|code| *code != 2).or(keyframe.matrix);
  let full_range = picture.full_range.or(keyframe.full_range).unwrap_or(false);
  let upright = to_rgb(&picture, matrix, full_range)?;
  let turned = match keyframe.rotation {
    90 => imageops::rotate90(&upright),
    180 => imageops::rotate180(&upright),
    270 => imageops::rotate270(&upright),
    _ => upright,
  };
  let (width, height) = turned.dimensions();
  Ok(PosterOutcome::Poster(Poster {
    format: config.format,
    width,
    height,
    time_sec: keyframe.time_sec,
    data: encode(&turned, config.format, config.quality)?,
  }))
}
