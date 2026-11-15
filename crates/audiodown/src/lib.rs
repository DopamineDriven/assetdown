//! MP3 and WAV metadata, PCM decoding and waveform peaks.
//!
//! `metadata` reads a file's headers and tags without decoding it, `decode` turns the stream into
//! interleaved 32-bit float PCM with Symphonia, and `waveform` reduces PCM to per-channel peak
//! envelopes on the rayon pool. Nothing here depends on a binding: every function takes bytes and
//! returns plain Rust data.

pub mod decode;
pub mod metadata;
pub mod waveform;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use decode::{DecodeConfig, decode};
use metadata::{AudioMeta, parse_audio};
use waveform::{Waveform, WaveformConfig, peaks};

/// Metadata plus decoded waveform from one input snapshot.
///
/// Metadata parsing only scans headers, decoding walks the whole stream; the
/// two are independent, so they run under `rayon::join` and the waveform
/// reduction itself fans out across the pool.
pub fn analyze(
  data: Arc<Vec<u8>>,
  decode_config: DecodeConfig,
  waveform_config: WaveformConfig,
) -> Result<(AudioMeta, Waveform), String> {
  let (meta, waveform) = rayon::join(
    || parse_audio(&data),
    || decode(Arc::clone(&data), decode_config).and_then(|pcm| peaks(&pcm, waveform_config)),
  );
  Ok((meta?, waveform?))
}
