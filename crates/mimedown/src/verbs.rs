//! What takes more than a header to give: the shape of a file's sound, a still from a film,
//! and where a film's keyframes are.
//!
//! Each is a verb of its own and not part of [`crate::specs`]: the first two decode, and a
//! caller who wants a row of facts should not pay for that. Each names the file's type itself
//! and hands the file to the parser that reads it, `audiodown` or `videodown`, when its feature
//! is on. None of them fails: a file with no sound has no waveform, and that is an answer
//! which says why.

use std::sync::Arc;

use crate::media_type::{MediaType, Parser};
use crate::sniff::{Sniffed, sniff};

/// What a verb gave: the thing asked for, or the reason there is none.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome<T> {
  Made(T),
  Nothing(WhyNot),
}

/// Why a verb gave nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhyNot {
  /// No reader here gives this for a file of this type: a picture has no waveform, and
  /// sound in a type that is not decoded here has none to give either.
  NoReader(MediaType),
  /// A file of this type has it, and the crate was built without the feature that reads it.
  FeatureOff(&'static str),
  /// The file holds none: a film with no sound has no waveform.
  Absent,
  /// It is there, in a coding that is not decoded here, which is named.
  Unsupported(String),
  /// Only the start of the file was given, and what was asked for takes more of it.
  Truncated,
  /// The file could not be read, for the reason given.
  Failed(String),
}

impl WhyNot {
  pub fn as_str(&self) -> &'static str {
    match self {
      WhyNot::NoReader(_) => "noReader",
      WhyNot::FeatureOff(_) => "featureOff",
      WhyNot::Absent => "absent",
      WhyNot::Unsupported(_) => "unsupported",
      WhyNot::Truncated => "truncated",
      WhyNot::Failed(_) => "failed",
    }
  }
}

/// How finely a waveform is cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
  /// This many peaks over the whole of the sound, or one for each frame when it has fewer.
  Peaks(usize),
  /// This many frames to a peak; the last peak may cover fewer.
  FramesPerPeak(usize),
}

/// How a waveform is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveformOptions {
  pub resolution: Resolution,
  /// Leave off the silence an encoder pads the sound with, when the file says how much.
  pub gapless: bool,
  /// Stop at the first packet that does not decode, and do not step over it.
  pub strict: bool,
  /// The most samples, over all channels, that are held in memory.
  pub max_decoded_samples: usize,
  /// Spread the work over the machine's cores.
  pub parallel: bool,
}

impl Default for WaveformOptions {
  fn default() -> Self {
    WaveformOptions {
      resolution: Resolution::Peaks(1024),
      gapless: true,
      strict: true,
      max_decoded_samples: 64_000_000,
      parallel: true,
    }
  }
}

/// One channel of a waveform: for each peak, the lowest and highest sample, the largest
/// either way, and the root of the mean of their squares.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelPeaks {
  pub min: Vec<f32>,
  pub max: Vec<f32>,
  pub peak: Vec<f32>,
  pub rms: Vec<f64>,
}

/// The shape of a file's sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
  pub sample_rate: u32,
  pub channel_count: usize,
  /// Samples in each channel.
  pub frame_count: usize,
  pub duration_ms: f64,
  /// Where each peak begins and ends, in frames: peak `i` covers the frames from
  /// `boundaries[i]` up to `boundaries[i + 1]`.
  pub boundaries: Vec<u32>,
  pub channels: Vec<ChannelPeaks>,
  /// The encoder's padding was left off.
  pub gapless: bool,
  /// Packets that did not decode and were stepped over.
  pub skipped_packets: u32,
}

/// The image a poster is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PosterFormat {
  #[default]
  Jpeg,
  Png,
  /// WebP that loses nothing, which is the only kind written here.
  Webp,
}

impl PosterFormat {
  pub fn media_type(self) -> MediaType {
    match self {
      PosterFormat::Jpeg => MediaType::Jpeg,
      PosterFormat::Png => MediaType::Png,
      PosterFormat::Webp => MediaType::Webp,
    }
  }
}

/// How a poster is made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PosterOptions {
  /// The keyframe at or before this many milliseconds is taken; the first one when `None`.
  pub at_ms: Option<f64>,
  pub format: PosterFormat,
  /// From 1 to 100, for a JPEG. The other two formats lose nothing and ignore it.
  pub quality: u8,
}

impl Default for PosterOptions {
  fn default() -> Self {
    PosterOptions {
      at_ms: None,
      format: PosterFormat::Jpeg,
      quality: 90,
    }
  }
}

/// A still from a film, at the size the film is shown at.
#[derive(Debug, Clone, PartialEq)]
pub struct Poster {
  /// The type of `data`.
  pub media_type: MediaType,
  pub width: u32,
  pub height: u32,
  /// When the keyframe it was made from is shown, in milliseconds from the start.
  pub time_ms: f64,
  pub data: Vec<u8>,
}

/// Where a film's keyframes are.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes {
  /// When each is shown, in milliseconds from the start, in order.
  pub times_ms: Vec<f64>,
  /// What the list was read from, as `videodown` names it: `stss`, `all-samples`, `cues`,
  /// `blocks`.
  pub source: String,
  /// How many pictures the track holds in all.
  pub sample_count: Option<u64>,
  /// The list covers the whole film. It does not when only the start of the file was given.
  pub complete: bool,
}

/// The type the bytes name, and the parser that reads it when one of the two here does.
fn read_by(bytes: &[u8]) -> (MediaType, Option<Parser>) {
  let media_type = match sniff(bytes) {
    Sniffed::Named { media_type, .. } => media_type,
    Sniffed::Unknown { .. } => MediaType::OctetStream,
  };
  (media_type, media_type.parser())
}

/// A waveform as `audiodown` and `videodown` each give it: the same shape under two names.
#[cfg(any(feature = "audio", feature = "video"))]
macro_rules! shaped {
  ($theirs:expr) => {{
    let theirs = $theirs;
    Waveform {
      sample_rate: theirs.sample_rate,
      channel_count: theirs.channel_count,
      frame_count: theirs.frame_count,
      duration_ms: theirs.duration_sec * 1000.0,
      boundaries: theirs.boundaries,
      channels: theirs
        .channels
        .into_iter()
        .map(|channel| ChannelPeaks {
          min: channel.min,
          max: channel.max,
          peak: channel.peak,
          rms: channel.rms,
        })
        .collect(),
      gapless: theirs.gapless_enabled,
      skipped_packets: theirs.skipped_packets,
    }
  }};
}

#[cfg(feature = "audio")]
fn waveform_of_sound(bytes: Arc<Vec<u8>>, options: WaveformOptions) -> Outcome<Waveform> {
  use audiodown::decode::{DecodeConfig, decode};
  use audiodown::waveform::{Resolution as Cut, WaveformConfig, peaks};

  let decoding = DecodeConfig {
    gapless: options.gapless,
    strict: options.strict,
    max_decoded_samples: options.max_decoded_samples,
  };
  let cutting = WaveformConfig {
    resolution: match options.resolution {
      Resolution::Peaks(count) => Cut::PeakCount(count),
      Resolution::FramesPerPeak(frames) => Cut::SamplesPerPeak(frames),
    },
    parallel: options.parallel,
  };
  match decode(bytes, decoding).and_then(|pcm| peaks(&pcm, cutting)) {
    Ok(shape) => Outcome::Made(shaped!(shape)),
    Err(reason) => Outcome::Nothing(WhyNot::Failed(reason)),
  }
}

#[cfg(not(feature = "audio"))]
fn waveform_of_sound(_bytes: Arc<Vec<u8>>, _options: WaveformOptions) -> Outcome<Waveform> {
  Outcome::Nothing(WhyNot::FeatureOff("audio"))
}

#[cfg(feature = "video")]
fn waveform_of_film(bytes: Arc<Vec<u8>>, options: WaveformOptions) -> Outcome<Waveform> {
  use videodown::WaveformOutcome;
  use videodown::audio::DecodeConfig;
  use videodown::waveform::{Resolution as Cut, WaveformConfig};

  let decoding = DecodeConfig {
    gapless: options.gapless,
    strict: options.strict,
    max_decoded_samples: options.max_decoded_samples,
  };
  let cutting = WaveformConfig {
    resolution: match options.resolution {
      Resolution::Peaks(count) => Cut::PeakCount(count),
      Resolution::FramesPerPeak(frames) => Cut::SamplesPerPeak(frames),
    },
    parallel: options.parallel,
  };
  let why = match videodown::audio_peaks(bytes, decoding, cutting) {
    Ok(WaveformOutcome::Peaks(shape)) => return Outcome::Made(shaped!(shape)),
    Ok(WaveformOutcome::NoAudio) => WhyNot::Absent,
    Ok(WaveformOutcome::Unsupported(coding)) => WhyNot::Unsupported(coding),
    Ok(WaveformOutcome::Truncated) => WhyNot::Truncated,
    Err(reason) => WhyNot::Failed(reason),
  };
  Outcome::Nothing(why)
}

#[cfg(not(feature = "video"))]
fn waveform_of_film(_bytes: Arc<Vec<u8>>, _options: WaveformOptions) -> Outcome<Waveform> {
  Outcome::Nothing(WhyNot::FeatureOff("video"))
}

#[cfg(feature = "video")]
fn poster_of_film(bytes: &[u8], options: PosterOptions) -> Outcome<Poster> {
  use videodown::poster::{PosterConfig, PosterFormat as Written, PosterOutcome};

  let config = PosterConfig {
    time_sec: options.at_ms.map(|milliseconds| milliseconds / 1000.0),
    format: match options.format {
      PosterFormat::Jpeg => Written::Jpeg,
      PosterFormat::Png => Written::Png,
      PosterFormat::Webp => Written::Webp,
    },
    quality: options.quality,
  };
  let why = match videodown::poster::poster(bytes, config) {
    Ok(PosterOutcome::Poster(still)) => {
      return Outcome::Made(Poster {
        media_type: options.format.media_type(),
        width: still.width,
        height: still.height,
        time_ms: still.time_sec * 1000.0,
        data: still.data,
      });
    }
    Ok(PosterOutcome::NoVideo) => WhyNot::Absent,
    Ok(PosterOutcome::Unsupported(coding)) => WhyNot::Unsupported(coding),
    // A poster takes the bytes of one keyframe, which the start of a file may not hold.
    Err(_) if videodown::meta::is_truncated(bytes) == Ok(true) => WhyNot::Truncated,
    Err(reason) => WhyNot::Failed(reason),
  };
  Outcome::Nothing(why)
}

#[cfg(not(feature = "video"))]
fn poster_of_film(_bytes: &[u8], _options: PosterOptions) -> Outcome<Poster> {
  Outcome::Nothing(WhyNot::FeatureOff("video"))
}

#[cfg(feature = "video")]
fn keyframes_of_film(bytes: &[u8]) -> Outcome<Keyframes> {
  use videodown::meta::{KeyframeSource, parse_video};

  let index = match parse_video(bytes) {
    Ok(meta) => meta.keyframes,
    Err(reason) => return Outcome::Nothing(WhyNot::Failed(reason)),
  };
  // A film with no picture has nothing to list.
  if index.source == KeyframeSource::None {
    return Outcome::Nothing(WhyNot::Absent);
  }
  Outcome::Made(Keyframes {
    times_ms: index
      .times_sec
      .iter()
      .map(|seconds| seconds * 1000.0)
      .collect(),
    source: index.source.as_str().to_string(),
    sample_count: index.sample_count,
    complete: index.complete,
  })
}

#[cfg(not(feature = "video"))]
fn keyframes_of_film(_bytes: &[u8]) -> Outcome<Keyframes> {
  Outcome::Nothing(WhyNot::FeatureOff("video"))
}

/// The shape of the sound in an audio file or a film. The bytes are the whole file, and are
/// shared and not copied: decoding reads them from another thread.
pub fn waveform(bytes: Arc<Vec<u8>>, options: WaveformOptions) -> Outcome<Waveform> {
  match read_by(&bytes) {
    (_, Some(Parser::Audiodown)) => waveform_of_sound(bytes, options),
    (_, Some(Parser::Videodown)) => waveform_of_film(bytes, options),
    (media_type, _) => Outcome::Nothing(WhyNot::NoReader(media_type)),
  }
}

/// A still from a film: one keyframe, decoded and written as a picture.
pub fn poster(bytes: &[u8], options: PosterOptions) -> Outcome<Poster> {
  match read_by(bytes) {
    (_, Some(Parser::Videodown)) => poster_of_film(bytes, options),
    (media_type, _) => Outcome::Nothing(WhyNot::NoReader(media_type)),
  }
}

/// Where a film's keyframes are. Nothing is decoded: the list is in the file's own index.
pub fn keyframes(bytes: &[u8]) -> Outcome<Keyframes> {
  match read_by(bytes) {
    (_, Some(Parser::Videodown)) => keyframes_of_film(bytes),
    (media_type, _) => Outcome::Nothing(WhyNot::NoReader(media_type)),
  }
}

#[cfg(test)]
mod tests;
