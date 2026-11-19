//! WebP: a RIFF file whose first chunk says which of three forms it takes. The two simple
//! forms are one picture and keep its size in the picture's own header; the extended form
//! opens with a chunk of flags and the canvas size, and can go on to a color profile, the
//! frames of an animation, EXIF and XMP.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::{exif, icc, xmp};
use crate::source::{Missing, Source};

/// What only a WebP says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebpDetail {
  /// The file takes the extended form, which is the one that can carry more than a picture.
  pub extended: bool,
  /// The picture is kept without loss. None for an extended file whose picture was not
  /// among the bytes given.
  pub lossless: Option<bool>,
  /// How many times an animation plays. Zero is without end.
  pub loop_count: Option<u16>,
  /// How long one pass through an animation's frames lasts.
  pub duration_ms: u64,
}

/// What the extended form's flags say the file carries.
const HAS_PROFILE: u8 = 0x20;
const HAS_ALPHA: u8 = 0x10;
const HAS_EXIF: u8 = 0x08;
const HAS_XMP: u8 = 0x04;
const IS_ANIMATED: u8 = 0x02;

/// No more chunks than this are walked, and no chunk longer than this is read.
const MOST_CHUNKS: usize = 1_000_000;
const LONGEST_CHUNK: u32 = 8 << 20;

fn le24(low: u8, middle: u8, high: u8) -> u32 {
  u32::from_le_bytes([low, middle, high, 0])
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let [k0, k1, k2, k3, ..] = reading.take(source.array::<8>(12), "inside its first chunk")?;
  let detail = |extended, lossless| WebpDetail {
    extended,
    lossless,
    loop_count: None,
    duration_ms: 0,
  };
  let simple = |width, height, alpha: bool, lossless| ImageFacts {
    has_alpha: Some(alpha),
    color_model: if alpha {
      ColorModel::Rgba
    } else {
      ColorModel::Rgb
    },
    color_space: ColorSpace::Srgb,
    ..ImageFacts::sized(
      width,
      height,
      ImageDetail::Webp(detail(false, Some(lossless))),
    )
  };
  match &[k0, k1, k2, k3] {
    // A frame tag of three bytes, a start code of three, and then the width and the height,
    // fourteen bits each, with two bits of scaling above them.
    b"VP8 " => {
      let [w0, w1, h0, h1] = reading.take(source.array::<4>(26), "inside its picture's header")?;
      let side = |low, high| u32::from(u16::from_le_bytes([low, high]) & 0x3FFF);
      Some(simple(side(w0, w1), side(h0, h1), false, false))
    }
    // A signature byte, then fourteen bits of width less one, fourteen of height less one,
    // and a bit that says whether alpha is used.
    b"VP8L" => {
      let header = reading.take(source.array::<4>(21), "inside its picture's header")?;
      let bits = u32::from_le_bytes(header);
      Some(simple(
        (bits & 0x3FFF) + 1,
        ((bits >> 14) & 0x3FFF) + 1,
        bits & (1 << 28) != 0,
        true,
      ))
    }
    b"VP8X" => extended(source, reading),
    _ => {
      reading.warn("the file's first chunk is not a picture's");
      None
    }
  }
}

/// What the chunks after the flags said, gathered as they are walked.
#[derive(Default)]
struct Chunks {
  frames: u32,
  duration_ms: u64,
  loop_count: Option<u16>,
  lossless: Option<bool>,
  profile: Option<Vec<u8>>,
  exif: Option<exif::Exif>,
  xmp: Option<xmp::Xmp>,
}

impl Chunks {
  /// Walk the chunks from `at` to the end of the file. An error is the read that could not be
  /// made.
  fn walk(
    &mut self,
    source: &Source<'_>,
    mut at: u64,
    reading: &mut Reading,
  ) -> Result<(), Missing> {
    for _ in 0..MOST_CHUNKS {
      let [k0, k1, k2, k3, l0, l1, l2, l3] = match source.array::<8>(at) {
        Ok(head) => head,
        // The file ends where its last chunk does.
        Err(Missing::PastEnd) => return Ok(()),
        Err(missing) => return Err(missing),
      };
      let kind = [k0, k1, k2, k3];
      let length = u32::from_le_bytes([l0, l1, l2, l3]);
      let payload_at = at + 8;
      // A chunk of odd length is followed by a byte of padding.
      at = payload_at + u64::from(length) + u64::from(length & 1);
      match &kind {
        b"VP8 " => self.lossless = self.lossless.or(Some(false)),
        b"VP8L" => self.lossless = self.lossless.or(Some(true)),
        // The background color, and how many times to play.
        b"ANIM" => {
          let [_, _, _, _, l0, l1] = source.array::<6>(payload_at)?;
          self.loop_count = Some(u16::from_le_bytes([l0, l1]));
        }
        // Where the frame sits and how big it is, three bytes each, then how long it shows,
        // a byte of flags, and the frame's own chunks.
        b"ANMF" => {
          let frame = source.array::<24>(payload_at)?;
          self.frames += 1;
          self.duration_ms += u64::from(le24(frame[12], frame[13], frame[14]));
          let inside = [frame[16], frame[17], frame[18], frame[19]];
          if self.lossless.is_none() && &inside != b"ALPH" {
            self.lossless = Some(&inside == b"VP8L");
          }
        }
        b"ICCP" | b"EXIF" | b"XMP " => {
          if length > LONGEST_CHUNK {
            let name = String::from_utf8_lossy(&kind).trim().to_string();
            reading.warn(format!("a {name} chunk of {length} bytes was left out"));
            continue;
          }
          let payload = source.read(payload_at, usize::try_from(length).unwrap_or(0))?;
          match &kind {
            b"ICCP" => self.profile = Some(payload.to_vec()),
            // Some writers put the six bytes a JPEG puts in front of the block.
            b"EXIF" => {
              self.exif = exif::from_bytes(payload.strip_prefix(b"Exif\0\0").unwrap_or(payload))
            }
            _ => self.xmp = xmp::parse(payload),
          }
        }
        _ => {}
      }
    }
    Ok(())
  }
}

/// The extended form: flags, the canvas, and then whatever the flags promise.
fn extended(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let header = reading.take(source.array::<10>(20), "inside its extended header")?;
  let [flags, _, _, _, w0, w1, w2, h0, h1, h2] = header;
  let (width, height) = (le24(w0, w1, w2) + 1, le24(h0, h1, h2) + 1);
  let animated = flags & IS_ANIMATED != 0;

  let mut chunks = Chunks::default();
  match chunks.walk(source, 30, reading) {
    Ok(()) => {}
    Err(Missing::PastEnd) => reading.warn("the file ends inside a chunk"),
    // What the flags promise lies further on. A file that promises nothing more than a
    // picture has said all it has to say already.
    Err(Missing::NotGiven(range)) => {
      let more = flags & (HAS_EXIF | HAS_XMP | IS_ANIMATED) != 0
        || (flags & HAS_PROFILE != 0 && chunks.profile.is_none());
      if more {
        reading.needs.get_or_insert(source.rest_from(range.offset));
      }
    }
  }

  let alpha = flags & HAS_ALPHA != 0;
  let frames = if animated { chunks.frames.max(1) } else { 1 };
  let detail = ImageDetail::Webp(WebpDetail {
    extended: true,
    lossless: chunks.lossless,
    loop_count: chunks.loop_count,
    duration_ms: chunks.duration_ms,
  });
  let mut facts = ImageFacts {
    frames,
    animated: animated && frames > 1,
    has_alpha: Some(alpha),
    color_model: if alpha {
      ColorModel::Rgba
    } else {
      ColorModel::Rgb
    },
    // A WebP without a profile is sRGB.
    color_space: ColorSpace::Srgb,
    xmp: chunks.xmp,
    ..ImageFacts::sized(width, height, detail)
  };
  if let Some(profile) = chunks.profile {
    match icc::parse(&profile) {
      Some(profile) => facts.take_profile(profile),
      None => {
        facts.icc_profile = Some("embedded".to_string());
        reading.warn("the color profile could not be read");
      }
    }
  }
  if let Some(exif) = chunks.exif {
    facts.take_exif(exif);
  }
  Some(facts)
}

#[cfg(test)]
mod tests;
