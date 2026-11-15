//! MP4, MOV, WebM and MKV container metadata, the keyframe index and audio-track waveform peaks.
//!
//! Pure Rust throughout, so the crate builds for every target, wasm included. `sniff` names the container from its first bytes, `meta` reads tracks,
//! duration, orientation, cover art and keyframes out of the headers, `audio` decodes the audio
//! track to interleaved 32-bit float PCM with Symphonia, and `waveform` reduces PCM to per-channel
//! peak envelopes on the rayon pool. `poster` is the one place a picture is decoded: a single
//! keyframe of H.264 or HEVC, by decoders that are pure Rust as well. Nothing here depends on a
//! binding: every function takes bytes and returns plain Rust data.

pub mod audio;
pub mod meta;
pub mod poster;
pub mod sniff;
pub mod waveform;

mod boxes;
mod mkv;
mod mp4;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use audio::{AudioOutcome, DecodeConfig, decode};
use meta::{VideoMeta, parse_video};
use waveform::{Waveform, WaveformConfig, peaks};

/// What the audio-track envelope call produced, so a missing waveform is
/// never confused with a failure.
#[derive(Debug)]
pub enum WaveformOutcome {
  Peaks(Waveform),
  NoAudio,
  Unsupported(String),
  /// The buffer is a prefix of the file; decoding it would yield a partial
  /// envelope presented as whole, so none is produced.
  Truncated,
}

pub fn audio_peaks(
  data: Arc<Vec<u8>>,
  decode_config: DecodeConfig,
  waveform_config: WaveformConfig,
) -> Result<WaveformOutcome, String> {
  if meta::is_truncated(&data)? {
    return Ok(WaveformOutcome::Truncated);
  }
  match decode(data, decode_config)? {
    AudioOutcome::Pcm(pcm) => peaks(&pcm, waveform_config).map(WaveformOutcome::Peaks),
    AudioOutcome::NoAudio => Ok(WaveformOutcome::NoAudio),
    AudioOutcome::Unsupported(reason) => Ok(WaveformOutcome::Unsupported(reason)),
  }
}

/// Metadata plus the audio-track envelope from one input snapshot.
///
/// Header parsing touches a few kilobytes; decoding walks the whole audio
/// track. They are independent, so they run under `rayon::join`, and the
/// waveform reduction itself fans out across the pool.
pub fn analyze(
  data: Arc<Vec<u8>>,
  decode_config: DecodeConfig,
  waveform_config: WaveformConfig,
) -> Result<(VideoMeta, WaveformOutcome), String> {
  let (meta, wave) = rayon::join(
    || parse_video(&data),
    || audio_peaks(Arc::clone(&data), decode_config, waveform_config),
  );
  Ok((meta?, wave?))
}
