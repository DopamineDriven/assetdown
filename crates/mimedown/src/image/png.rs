//! PNG: a signature and then chunks, each a length, a type of four letters, its bytes and a
//! checksum. The chunks are walked by their lengths, so one that is not wanted costs a read of
//! eight bytes.

use std::io::Read;

use flate2::read::ZlibDecoder;

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::exif::{self, Exif};
use crate::container::icc::{self, IccProfile};
use crate::container::xmp::{self, Xmp};
use crate::source::{Missing, Source};

/// How a text chunk keeps its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngTextKind {
  /// `tEXt`: Latin-1, as it is.
  Plain,
  /// `zTXt`: Latin-1, compressed.
  Compressed,
  /// `iTXt`: UTF-8, compressed or not.
  International,
}

/// One text chunk: a keyword and what is said under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngText {
  pub kind: PngTextKind,
  pub keyword: String,
  pub text: String,
}

/// What only a PNG says.
#[derive(Debug, Clone, PartialEq)]
pub struct PngDetail {
  /// Bits in each sample: 1, 2, 4, 8 or 16.
  pub bit_depth: u8,
  /// The header's own number for how color is kept: 0, 2, 3, 4 or 6.
  pub color_type: u8,
  pub interlaced: bool,
  /// Pixels to the inch across and down, when the file gives its density in a real unit.
  pub dpi: Option<(f64, f64)>,
  /// A pixel's width over its height, when the file gives a density with no unit.
  pub pixel_aspect_ratio: Option<f64>,
  pub gamma: Option<f64>,
  /// How many times an animation plays. Zero is without end.
  pub loop_count: Option<u32>,
  pub text: Vec<PngText>,
  /// When the picture was last changed. The format keeps this in UTC.
  pub modification_time: Option<String>,
}

const SIGNATURE: u64 = 8;

/// No more chunks than this are walked, no chunk longer than this is read, and no compressed
/// chunk is inflated past it. Text beyond the last figure in total is left out.
const MOST_CHUNKS: usize = 100_000;
const LONGEST_CHUNK: u32 = 4 << 20;
const LONGEST_INFLATED: u64 = 4 << 20;
const MOST_TEXT: usize = 256 << 10;

/// Whether the file opens as a PNG does and the checksum of its header chunk holds: the
/// signature, a chunk of thirteen bytes called `IHDR`, and the checksum of those. A signature
/// is a constant in every program that reads the format; a checksum that holds is not.
pub(crate) fn header_holds(source: &Source<'_>) -> Result<bool, Missing> {
  let opening = source.array::<33>(0)?;
  let (start, rest) = opening.split_at(12);
  let (checked, checksum) = rest.split_at(17);
  let mut crc = flate2::Crc::new();
  crc.update(checked);
  Ok(
    start == b"\x89PNG\r\n\x1A\n\0\0\0\x0D"
      && checked.starts_with(b"IHDR")
      && checksum == crc.sum().to_be_bytes(),
  )
}

/// Whether the file is an animated PNG: an animation control chunk stands before the picture.
pub(crate) fn is_animated(source: &Source<'_>) -> Result<bool, Missing> {
  let mut at = SIGNATURE;
  for _ in 0..MOST_CHUNKS {
    let [l0, l1, l2, l3, kind @ ..] = source.array::<8>(at)?;
    match &kind {
      b"acTL" => return Ok(true),
      b"IDAT" | b"IEND" => return Ok(false),
      _ => at = at.saturating_add(12 + u64::from(u32::from_be_bytes([l0, l1, l2, l3]))),
    }
  }
  Ok(false)
}

/// What the chunks said, gathered as they are walked.
#[derive(Default)]
struct Chunks {
  header: Option<[u8; 13]>,
  transparency: bool,
  frames: Option<u32>,
  loop_count: Option<u32>,
  standard_rgb: bool,
  coded_primaries: Option<u8>,
  profile_name: Option<String>,
  profile: Option<IccProfile>,
  exif: Option<Exif>,
  xmp: Option<Xmp>,
  gamma: Option<f64>,
  density: Option<(u32, u32, u8)>,
  text: Vec<PngText>,
  text_bytes: usize,
  modification_time: Option<String>,
}

/// The keyword an XMP packet is kept under.
const XMP_KEYWORD: &str = "XML:com.adobe.xmp";

/// The keywords ImageMagick keeps a camera's notes under, from before the format had a chunk
/// for them: the block's name, its length, and its bytes in hexadecimal.
const RAW_EXIF_KEYWORDS: [&str; 2] = ["Raw profile type exif", "Raw profile type APP1"];

/// The bytes a raw profile's text stands for.
fn raw_profile(text: &str) -> Option<Vec<u8>> {
  // A line for the name, a line for the length, and then the digits, in lines of their own.
  let mut lines = text.trim_start().lines();
  lines.next()?;
  lines.next()?;
  let digits: Vec<u8> = lines
    .flat_map(str::bytes)
    .filter(u8::is_ascii_hexdigit)
    .collect();
  digits
    .as_chunks::<2>()
    .0
    .iter()
    .map(|pair| {
      let pair = std::str::from_utf8(pair).ok()?;
      u8::from_str_radix(pair, 16).ok()
    })
    .collect()
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let mut chunks = Chunks::default();
  let mut at = SIGNATURE;
  let mut picture_seen = false;
  for _ in 0..MOST_CHUNKS {
    let head = match source.array::<8>(at) {
      Ok(head) => head,
      // What follows the picture is optional, so it is not asked for.
      Err(Missing::NotGiven(_)) if picture_seen => break,
      Err(missing) => {
        reading.take(Err::<(), _>(missing), "before the chunk that closes it");
        break;
      }
    };
    let [l0, l1, l2, l3, kind @ ..] = head;
    let length = u32::from_be_bytes([l0, l1, l2, l3]);
    let payload_at = at.saturating_add(8);
    at = payload_at.saturating_add(u64::from(length) + 4);
    match &kind {
      b"IEND" => break,
      b"IDAT" => picture_seen = true,
      b"tRNS" => chunks.transparency = true,
      b"sRGB" => chunks.standard_rgb = true,
      b"IHDR" | b"acTL" | b"cICP" | b"iCCP" | b"gAMA" | b"pHYs" | b"tEXt" | b"zTXt" | b"iTXt"
      | b"tIME" | b"eXIf" => {
        let name = String::from_utf8_lossy(&kind).into_owned();
        if length > LONGEST_CHUNK {
          reading.warn(format!("a {name} chunk of {length} bytes was left out"));
          continue;
        }
        let wanted = usize::try_from(length).unwrap_or(0);
        let payload = match source.read(payload_at, wanted) {
          Ok(payload) => payload,
          Err(Missing::NotGiven(_)) if picture_seen => break,
          Err(missing) => {
            reading.take(Err::<(), _>(missing), &format!("inside its {name} chunk"));
            break;
          }
        };
        chunks.take(&kind, payload, reading);
      }
      _ => {}
    }
  }
  chunks.facts(reading)
}

fn be32(bytes: &[u8]) -> Option<u32> {
  Some(u32::from_be_bytes(*bytes.first_chunk::<4>()?))
}

/// Latin-1, which is what a keyword and the older text chunks are written in.
fn latin1(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| char::from(*byte)).collect()
}

/// Inflate a compressed chunk, up to the most that is read. None when it is damaged or longer.
fn inflate(bytes: &[u8]) -> Option<Vec<u8>> {
  let mut inflated = Vec::new();
  let read = ZlibDecoder::new(bytes)
    .take(LONGEST_INFLATED + 1)
    .read_to_end(&mut inflated)
    .ok()?;
  (u64::try_from(read).ok()? <= LONGEST_INFLATED).then_some(inflated)
}

impl Chunks {
  fn take(&mut self, kind: &[u8; 4], payload: &[u8], reading: &mut Reading) {
    match kind {
      b"IHDR" => self.header = payload.first_chunk::<13>().copied(),
      b"acTL" => {
        self.frames = be32(payload);
        self.loop_count = payload.get(4..).and_then(be32);
      }
      b"cICP" => self.coded_primaries = payload.first().copied(),
      b"gAMA" => self.gamma = be32(payload).map(|gamma| f64::from(gamma) / 100_000.0),
      b"eXIf" => self.exif = exif::from_bytes(payload),
      b"pHYs" => {
        if let [x0, x1, x2, x3, y0, y1, y2, y3, unit] = *payload {
          let across = u32::from_be_bytes([x0, x1, x2, x3]);
          let down = u32::from_be_bytes([y0, y1, y2, y3]);
          self.density = Some((across, down, unit));
        }
      }
      b"tIME" => {
        if let [y0, y1, month, day, hour, minute, second] = *payload {
          let year = u16::from_be_bytes([y0, y1]);
          self.modification_time = Some(format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
          ));
        }
      }
      // The name the file gives the profile, a zero, the compression method, and the profile
      // compressed.
      b"iCCP" => {
        let mut parts = payload.splitn(2, |byte| *byte == 0);
        self.profile_name = parts.next().map(latin1);
        self.profile = parts
          .next()
          .and_then(|rest| rest.get(1..))
          .and_then(inflate)
          .and_then(|profile| icc::parse(&profile));
        if self.profile.is_none() {
          reading.warn("the color profile could not be read");
        }
      }
      b"tEXt" | b"zTXt" | b"iTXt" => self.take_text(kind, payload, reading),
      _ => {}
    }
  }

  fn take_text(&mut self, kind: &[u8; 4], payload: &[u8], reading: &mut Reading) {
    let Some(end) = payload.iter().position(|byte| *byte == 0) else {
      return;
    };
    let keyword = latin1(&payload[..end]);
    let rest = &payload[end + 1..];
    let found = match kind {
      b"tEXt" => Some((PngTextKind::Plain, latin1(rest))),
      // The compression method, then the text.
      b"zTXt" => rest
        .get(1..)
        .and_then(inflate)
        .map(|text| (PngTextKind::Compressed, latin1(&text))),
      // Whether it is compressed, the method, a language, the keyword translated, the text.
      _ => international(rest).map(|text| (PngTextKind::International, text)),
    };
    let Some((kind, text)) = found else {
      reading.warn(format!(
        "the text under {keyword:?} could not be read and was left out"
      ));
      return;
    };
    // An XMP packet is read for what it says and is not kept as text beside the rest. So are
    // a camera's notes written out in hexadecimal.
    if keyword == XMP_KEYWORD {
      self.xmp = xmp::parse(text.as_bytes());
      return;
    }
    if RAW_EXIF_KEYWORDS.contains(&keyword.as_str()) {
      let block = raw_profile(&text);
      let block = block
        .as_deref()
        .map(|block| block.strip_prefix(b"Exif\0\0").unwrap_or(block));
      // A chunk made for the purpose is the better source when the file has both.
      self.exif = self
        .exif
        .take()
        .or_else(|| block.and_then(exif::from_bytes));
      return;
    }
    if self.text_bytes + text.len() > MOST_TEXT {
      reading.warn(format!(
        "the text under {keyword:?} was left out: the file holds more text than is kept"
      ));
      return;
    }
    self.text_bytes += text.len();
    self.text.push(PngText {
      kind,
      keyword,
      text,
    });
  }

  fn facts(self, reading: &mut Reading) -> Option<ImageFacts> {
    let Some(header) = self.header else {
      // Asked for already when the header was simply not among the bytes given.
      if reading.needs.is_none() && reading.warnings.is_empty() {
        reading.warn("the file has no header chunk");
      }
      return None;
    };
    let [
      w0,
      w1,
      w2,
      w3,
      h0,
      h1,
      h2,
      h3,
      bit_depth,
      color_type,
      _,
      _,
      interlace,
    ] = header;
    let (color_model, gray) = match color_type {
      0 => (ColorModel::Grayscale, true),
      2 => (ColorModel::Rgb, false),
      3 => (ColorModel::Indexed, false),
      4 => (ColorModel::GrayscaleAlpha, true),
      6 => (ColorModel::Rgba, false),
      _ => (ColorModel::Unknown, false),
    };
    // What the file says of its own color, the most exact statement first: coded values, then
    // a profile, then the chunk that says sRGB. A picture in shades of gray is gray whatever
    // curve its shades follow, and a file that says nothing is taken for sRGB, as the format
    // asks.
    let coded = self.coded_primaries.and_then(|primaries| match primaries {
      1 => Some(ColorSpace::Rec709),
      9 => Some(ColorSpace::Rec2020),
      12 => Some(ColorSpace::DisplayP3),
      _ => None,
    });
    let named = self.profile_name.as_deref().and_then(ColorSpace::named);
    let unstated = if gray {
      ColorSpace::Gray
    } else if self.standard_rgb || self.profile_name.is_none() {
      ColorSpace::Srgb
    } else {
      ColorSpace::Unknown
    };
    let (dpi, pixel_aspect_ratio) = match self.density {
      // Pixels to the meter.
      Some((across, down, 1)) => (
        Some((f64::from(across) * 0.0254, f64::from(down) * 0.0254)),
        None,
      ),
      Some((across, down, _)) if down != 0 => (None, Some(f64::from(across) / f64::from(down))),
      _ => (None, None),
    };
    let frames = self.frames.unwrap_or(1).max(1);
    let detail = ImageDetail::Png(PngDetail {
      bit_depth,
      color_type,
      interlaced: interlace == 1,
      dpi,
      pixel_aspect_ratio,
      gamma: self.gamma,
      loop_count: self.loop_count,
      text: self.text,
      modification_time: self.modification_time,
    });
    let mut facts = ImageFacts {
      frames,
      animated: frames > 1,
      has_alpha: Some(matches!(color_type, 4 | 6) || self.transparency),
      color_model,
      color_space: named.unwrap_or(unstated),
      icc_profile: self.profile_name,
      xmp: self.xmp,
      ..ImageFacts::sized(
        u32::from_be_bytes([w0, w1, w2, w3]),
        u32::from_be_bytes([h0, h1, h2, h3]),
        detail,
      )
    };
    // The profile's own name is the better one; the file's name for it is kept when the
    // profile has none.
    if let Some(profile) = self.profile {
      let filed_as = facts.icc_profile.take();
      let described = profile.description.is_some();
      facts.take_profile(profile);
      if !described {
        facts.icc_profile = filed_as.or(facts.icc_profile);
      }
    }
    if let Some(coded) = coded {
      facts.color_space = coded;
    }
    if let Some(exif) = self.exif {
      facts.take_exif(exif);
    }
    Some(facts)
  }
}

/// The text of an `iTXt` chunk after its keyword.
fn international(rest: &[u8]) -> Option<String> {
  let (&compressed, rest) = rest.split_first()?;
  let rest = rest.get(1..)?;
  // The language and the translated keyword each end in a zero.
  let language_end = rest.iter().position(|byte| *byte == 0)?;
  let rest = &rest[language_end + 1..];
  let translated_end = rest.iter().position(|byte| *byte == 0)?;
  let text = &rest[translated_end + 1..];
  let text = if compressed == 1 {
    inflate(text)?
  } else {
    text.to_vec()
  };
  Some(String::from_utf8_lossy(&text).into_owned())
}

#[cfg(test)]
mod tests;
