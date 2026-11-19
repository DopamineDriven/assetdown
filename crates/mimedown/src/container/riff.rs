//! RIFF: a file of chunks, whose first twelve bytes say what kind of file the chunks make.

use crate::source::{Missing, Source};

/// What a file of chunks opens with: RIFF itself, RIFF for a file too long for a length of four
/// bytes, and RIFF with its lengths written the other way round.
const CONTAINERS: [&[u8; 4]; 3] = [b"RIFF", b"RF64", b"RIFX"];

/// The form a RIFF file declares: `WEBP`, `WAVE`, `AVI `.
pub(crate) fn form(source: &Source<'_>) -> Result<Option<[u8; 4]>, Missing> {
  let [container @ .., _, _, _, _] = source.array::<8>(0)?;
  if !CONTAINERS.contains(&&container) {
    return Ok(None);
  }
  source.array(8).map(Some)
}

#[cfg(test)]
mod tests;
