//! GIF: a header, the size of the screen the frames are drawn on, and then blocks to the end of
//! the file. The frames are counted by walking the blocks, so the count takes the whole file.

use super::{ColorModel, ImageDetail, ImageFacts, Reading};
use crate::source::{Missing, Source};

/// What only a GIF says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifDetail {
  /// `87a` or `89a`.
  pub version: String,
  /// How many times the animation plays again after the first. Zero is without end. None for
  /// a file that does not say, which plays once.
  pub loop_count: Option<u16>,
  /// How long one pass through the frames lasts.
  pub duration_ms: u64,
  /// How many colors the palette shared by the frames holds.
  pub global_colors: Option<u16>,
}

/// No more blocks than this are walked.
const MOST_BLOCKS: usize = 1_000_000;

/// How many bytes a color table takes, from the three bits that give its size.
fn table_bytes(packed: u8) -> u64 {
  3 * (2u64 << (packed & 0x07))
}

/// Where the run of sub-blocks that starts at `at` ends: each is a length and that many bytes,
/// and a length of zero closes the run.
fn after_sub_blocks(source: &Source<'_>, mut at: u64) -> Result<u64, Missing> {
  loop {
    let [length] = source.array::<1>(at)?;
    at += 1 + u64::from(length);
    if length == 0 {
      return Ok(at);
    }
  }
}

/// What the blocks said, gathered as they are walked.
#[derive(Default)]
struct Blocks {
  frames: u32,
  transparent: bool,
  loop_count: Option<u16>,
  hundredths: u64,
}

impl Blocks {
  /// Walk the blocks from `at` to the trailer. An error is the read that could not be made.
  fn walk(&mut self, source: &Source<'_>, mut at: u64) -> Result<(), Missing> {
    for _ in 0..MOST_BLOCKS {
      let [introducer] = source.array::<1>(at)?;
      match introducer {
        // A frame: where it sits and how big it is, a palette of its own, then its pixels.
        0x2C => {
          let descriptor = source.array::<9>(at + 1)?;
          let own_table = if descriptor[8] & 0x80 != 0 {
            table_bytes(descriptor[8])
          } else {
            0
          };
          // One byte of code size stands in front of the pixels.
          at = after_sub_blocks(source, at + 10 + own_table + 1)?;
          self.frames += 1;
        }
        0x21 => {
          let [label] = source.array::<1>(at + 1)?;
          match label {
            // How the next frame is shown: whether one color is see-through, and for how long.
            0xF9 => {
              let [_, packed, d0, d1, _] = source.array::<5>(at + 2)?;
              self.transparent |= packed & 0x01 != 0;
              self.hundredths += u64::from(u16::from_le_bytes([d0, d1]));
            }
            // An application's own block. Netscape's says how often to play the frames again.
            0xFF => {
              let [_, name @ ..] = source.array::<12>(at + 2)?;
              if name == *b"NETSCAPE2.0" || name == *b"ANIMEXTS1.0" {
                let [length, kind, l0, l1] = source.array::<4>(at + 14)?;
                if length == 3 && kind == 1 {
                  self.loop_count = Some(u16::from_le_bytes([l0, l1]));
                }
              }
            }
            _ => {}
          }
          at = after_sub_blocks(source, at + 2)?;
        }
        _ => return Ok(()),
      }
    }
    Ok(())
  }
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let header = reading.take(source.array::<13>(0), "inside its header")?;
  let [_, _, _, v0, v1, v2, w0, w1, h0, h1, packed, _, _] = header;
  let global_table = packed & 0x80 != 0;
  let first_block = 13 + if global_table { table_bytes(packed) } else { 0 };

  let mut blocks = Blocks::default();
  match blocks.walk(source, first_block) {
    Ok(()) => {}
    Err(Missing::PastEnd) => reading.warn("the file ends before the block that closes it"),
    // The frames cannot be counted from part of the file: the rest is asked for.
    Err(Missing::NotGiven(range)) => {
      reading.needs.get_or_insert(source.rest_from(range.offset));
    }
  }

  let frames = blocks.frames.max(1);
  let detail = ImageDetail::Gif(GifDetail {
    version: String::from_utf8_lossy(&[v0, v1, v2]).into_owned(),
    loop_count: blocks.loop_count,
    duration_ms: blocks.hundredths * 10,
    global_colors: global_table.then(|| 2u16 << (packed & 0x07)),
  });
  Some(ImageFacts {
    frames,
    animated: frames > 1,
    has_alpha: Some(blocks.transparent),
    color_model: ColorModel::Indexed,
    ..ImageFacts::sized(
      u32::from(u16::from_le_bytes([w0, w1])),
      u32::from(u16::from_le_bytes([h0, h1])),
      detail,
    )
  })
}

#[cfg(test)]
mod tests;
