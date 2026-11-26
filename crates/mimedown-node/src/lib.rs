#![deny(clippy::all)]

//! `@d0paminedriven/mimedown`: what a file is, told from its bytes, for Node.
//!
//! This crate is the Node-API surface: this file holds the functions, their promise-returning
//! twins and the class, `types.rs` holds the JS boundary types, and `mime.rs` the union of
//! every type's string. The work itself is done by the `mimedown` crate, which knows nothing
//! about Node.
//!
//! Every `*Async` function and method returns an [`AsyncTask`]: the input is copied on the JS
//! thread, the work runs on libuv's pool, and the result is turned into JS values back on the
//! JS thread when the promise resolves. The bytes are copied because a caller may go on to
//! change its buffer while the work runs.

use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

mod mime;
mod types;

pub use mime::Mime;
pub use types::{
  AudioFacts, BmpDetail, BuiltWith, ByteRange, ColorModel, ColorSpace, Density, DocumentDetail,
  DocumentFacts, Evidence, Facts, GifDetail, HdrDetail, HdrEncoding, HeifDetail, Hotspot,
  IccProfile, IcoDetail, IcoEntry, IcoKind, ImageDetail, ImageFacts, Jp2Detail, JpegDetail,
  JxlDetail, JxrDetail, Keyframes, KeyframesOutcome, OfficeDetail, Options, Parser, PdfDetail,
  Piece, PngDetail, PngText, PngTextKind, Poster, PosterFormat, PosterOptions, PosterOutcome,
  Retry, RtfDetail, Sniffed, Specs, SvgDetail, TextDetail, TiffDetail, TopLevel, VideoFacts,
  Waveform, WaveformChannel, WaveformOptions, WaveformOutcome, WebpDetail, WhyNot, Xmp,
};

// ── What a caller passes ────────────────────────────────────────────────────────────────────

fn invalid(message: impl Into<String>) -> Error {
  Error::new(Status::InvalidArg, message.into())
}

/// The largest whole number a JS number holds exactly.
const SAFE: u64 = 9_007_199_254_740_991;

/// A JS number that has to be a whole number from `least` to `most`. A fraction, `NaN` and
/// `Infinity` are refused and not quietly cut down.
fn whole(value: f64, name: &str, least: u64, most: u64) -> Result<u64> {
  let fits =
    value.is_finite() && value.fract() == 0.0 && value >= least as f64 && value <= most as f64;
  if !fits {
    return Err(invalid(format!(
      "{name} must be a whole number from {least} to {most}"
    )));
  }
  // Checked above to be a whole number in range, so the conversion keeps its value.
  Ok(value as u64)
}

fn count(value: f64, name: &str, least: u64, most: u64) -> Result<usize> {
  usize::try_from(whole(value, name, least, most)?)
    .map_err(|_| invalid(format!("{name} is larger than this machine can count")))
}

/// What the caller said beside the bytes, kept so that it can cross to another thread.
#[derive(Clone, Default)]
pub struct Said {
  file_name: Option<String>,
  retry_on_unknown: bool,
}

impl Said {
  fn new(options: Option<Options>) -> Self {
    let options = options.unwrap_or_default();
    Said {
      file_name: options.file_name,
      retry_on_unknown: options.retry_on_unknown.unwrap_or(false),
    }
  }

  fn options(&self) -> mimedown::Options<'_> {
    mimedown::Options {
      file_name: self.file_name.as_deref(),
      retry_on_unknown: self.retry_on_unknown,
    }
  }
}

/// The pieces of a file as the library takes them, where they lie in the caller's buffers, and
/// the file's size: each offset and the size a whole number.
fn placed(pieces: &[Piece], size: Option<f64>) -> Result<(Vec<mimedown::Piece<'_>>, Option<u64>)> {
  let size = size.map(|size| whole(size, "size", 0, SAFE)).transpose()?;
  let pieces = pieces
    .iter()
    .map(|piece| {
      Ok(mimedown::Piece {
        offset: whole(piece.offset, "offset", 0, SAFE)?,
        bytes: &piece.bytes,
      })
    })
    .collect::<Result<Vec<_>>>()?;
  Ok((pieces, size))
}

/// A file read from pieces of it. What is wrong with the pieces is the caller's mistake.
fn read_pieces<T>(
  pieces: &[mimedown::Piece<'_>],
  size: Option<u64>,
  read: impl FnOnce(&mimedown::Source<'_>) -> T,
) -> Result<T> {
  let source =
    mimedown::Source::from_pieces(pieces, size).map_err(|error| invalid(error.to_string()))?;
  Ok(read(&source))
}

/// The pieces of a file read where they lie in the caller's buffers.
fn from_pieces<T>(
  pieces: &[Piece],
  size: Option<f64>,
  read: impl FnOnce(&mimedown::Source<'_>) -> T,
) -> Result<T> {
  let (pieces, size) = placed(pieces, size)?;
  read_pieces(&pieces, size, read)
}

/// The pieces of a file copied out of the caller's buffers, so that they can cross to another
/// thread: where each starts and its bytes, and the file's size when it is known.
pub struct Kept {
  pieces: Vec<(u64, Vec<u8>)>,
  size: Option<u64>,
}

impl Kept {
  /// The pieces are checked here, on the JS thread and before they are copied, so that what
  /// is wrong with them is thrown where they were passed and not from a promise.
  fn new(pieces: &[Piece], size: Option<f64>) -> Result<Self> {
    let (pieces, size) = placed(pieces, size)?;
    read_pieces(&pieces, size, |_| ())?;
    Ok(Kept {
      pieces: pieces
        .iter()
        .map(|piece| (piece.offset, piece.bytes.to_vec()))
        .collect(),
      size,
    })
  }

  fn read<T>(&self, read: impl FnOnce(&mimedown::Source<'_>) -> T) -> Result<T> {
    let pieces: Vec<mimedown::Piece<'_>> = self
      .pieces
      .iter()
      .map(|(offset, bytes)| mimedown::Piece {
        offset: *offset,
        bytes,
      })
      .collect();
    read_pieces(&pieces, self.size, read)
  }
}

fn waveform_options(options: Option<WaveformOptions>) -> Result<mimedown::WaveformOptions> {
  let options = options.unwrap_or_default();
  let usual = mimedown::WaveformOptions::default();
  let resolution = match (options.peak_count, options.samples_per_peak) {
    (Some(_), Some(_)) => {
      return Err(invalid("pass either peakCount or samplesPerPeak, not both"));
    }
    (None, Some(frames)) => {
      mimedown::Resolution::FramesPerPeak(count(frames, "samplesPerPeak", 1, u64::from(u32::MAX))?)
    }
    (Some(peaks), None) => mimedown::Resolution::Peaks(count(peaks, "peakCount", 1, 1_000_000)?),
    (None, None) => usual.resolution,
  };
  let max_decoded_samples = match options.max_decoded_samples {
    Some(most) => count(most, "maxDecodedSamples", 1, u64::from(u32::MAX))?,
    None => usual.max_decoded_samples,
  };
  Ok(mimedown::WaveformOptions {
    resolution,
    gapless: options.gapless.unwrap_or(usual.gapless),
    strict: options.strict.unwrap_or(usual.strict),
    max_decoded_samples,
    parallel: options.parallel.unwrap_or(usual.parallel),
  })
}

fn poster_options(options: Option<PosterOptions>) -> Result<mimedown::PosterOptions> {
  let options = options.unwrap_or_default();
  let usual = mimedown::PosterOptions::default();
  if let Some(at) = options.at_ms
    && !(at.is_finite() && at >= 0.0)
  {
    return Err(invalid(
      "atMs must be a number of milliseconds, zero or more",
    ));
  }
  let quality = match options.quality {
    Some(quality) => u8::try_from(whole(quality, "quality", 1, 100)?)
      .map_err(|_| invalid("quality must be a whole number from 1 to 100"))?,
    None => usual.quality,
  };
  Ok(mimedown::PosterOptions {
    at_ms: options.at_ms,
    format: match options.format {
      Some(PosterFormat::Jpeg) | None => mimedown::PosterFormat::Jpeg,
      Some(PosterFormat::Png) => mimedown::PosterFormat::Png,
      Some(PosterFormat::Webp) => mimedown::PosterFormat::Webp,
    },
    quality,
  })
}

fn snapshot(data: &[u8]) -> Arc<Vec<u8>> {
  Arc::new(data.to_vec())
}

// ── Work that runs off the JS thread ────────────────────────────────────────────────────────

pub struct SniffTask {
  bytes: Arc<Vec<u8>>,
  said: Said,
}

#[napi]
impl Task for SniffTask {
  type Output = mimedown::Sniffed;
  type JsValue = Sniffed;

  fn compute(&mut self) -> Result<Self::Output> {
    let source = mimedown::Source::whole(&self.bytes);
    Ok(mimedown::sniff_with(&source, self.said.options()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct SpecsTask {
  bytes: Arc<Vec<u8>>,
  said: Said,
}

#[napi]
impl Task for SpecsTask {
  type Output = mimedown::Specs;
  type JsValue = Specs;

  fn compute(&mut self) -> Result<Self::Output> {
    let source = mimedown::Source::whole(&self.bytes);
    Ok(mimedown::specs_with(&source, self.said.options()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct SniffRangesTask {
  kept: Kept,
  said: Said,
}

#[napi]
impl Task for SniffRangesTask {
  type Output = mimedown::Sniffed;
  type JsValue = Sniffed;

  fn compute(&mut self) -> Result<Self::Output> {
    self
      .kept
      .read(|source| mimedown::sniff_with(source, self.said.options()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct SpecsRangesTask {
  kept: Kept,
  said: Said,
}

#[napi]
impl Task for SpecsRangesTask {
  type Output = mimedown::Specs;
  type JsValue = Specs;

  fn compute(&mut self) -> Result<Self::Output> {
    self
      .kept
      .read(|source| mimedown::specs_with(source, self.said.options()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct WaveformTask {
  bytes: Arc<Vec<u8>>,
  options: mimedown::WaveformOptions,
}

#[napi]
impl Task for WaveformTask {
  type Output = mimedown::Outcome<mimedown::Waveform>;
  type JsValue = WaveformOutcome;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::waveform(Arc::clone(&self.bytes), self.options))
  }

  // Typed arrays are made here, on the JS thread.
  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct PosterTask {
  bytes: Arc<Vec<u8>>,
  options: mimedown::PosterOptions,
}

#[napi]
impl Task for PosterTask {
  type Output = mimedown::Outcome<mimedown::Poster>;
  type JsValue = PosterOutcome;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::poster(&self.bytes, self.options))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct KeyframesTask {
  bytes: Arc<Vec<u8>>,
}

#[napi]
impl Task for KeyframesTask {
  type Output = mimedown::Outcome<mimedown::Keyframes>;
  type JsValue = KeyframesOutcome;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::keyframes(&self.bytes))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// The tables read no file and answer at once. Each has a promise all the same, so that a
// caller who has only promises, as a page has of a worker, can ask them too.

pub struct MediaTypesOfTask {
  extension: String,
}

#[napi]
impl Task for MediaTypesOfTask {
  type Output = Vec<mimedown::MediaType>;
  type JsValue = Vec<Mime>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::media_types_of(&self.extension))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(Into::into).collect())
  }
}

pub struct MediaTypeOfPathTask {
  path: String,
}

#[napi]
impl Task for MediaTypeOfPathTask {
  type Output = Option<mimedown::MediaType>;
  type JsValue = Option<Mime>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::media_type_of_path(&self.path))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.map(Into::into))
  }
}

pub struct ExtensionsOfTask {
  mime: String,
}

#[napi]
impl Task for ExtensionsOfTask {
  type Output = &'static [&'static str];
  type JsValue = Vec<String>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::extensions_of(&self.mime))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(extensions(output))
  }
}

pub struct BuiltWithTask;

#[napi]
impl Task for BuiltWithTask {
  type Output = mimedown::BuiltWith;
  type JsValue = BuiltWith;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(mimedown::built_with())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

// ── What a file is ──────────────────────────────────────────────────────────────────────────

/// What the whole file in `data` is, from as few of its bytes as that takes.
#[napi]
pub fn sniff(data: &[u8], options: Option<Options>) -> Sniffed {
  let said = Said::new(options);
  mimedown::sniff_with(&mimedown::Source::whole(data), said.options()).into()
}

#[napi]
pub fn sniff_async(data: &[u8], options: Option<Options>) -> AsyncTask<SniffTask> {
  AsyncTask::new(SniffTask {
    bytes: snapshot(data),
    said: Said::new(options),
  })
}

/// The same, from pieces of a file: the head, or the head and whatever an earlier answer's
/// `needs` asked for. `size` is the file's whole size, when it is known.
#[napi]
pub fn sniff_from_ranges(
  pieces: Vec<Piece>,
  size: Option<f64>,
  options: Option<Options>,
) -> Result<Sniffed> {
  let said = Said::new(options);
  from_pieces(&pieces, size, |source| {
    mimedown::sniff_with(source, said.options()).into()
  })
}

#[napi]
pub fn sniff_from_ranges_async(
  pieces: Vec<Piece>,
  size: Option<f64>,
  options: Option<Options>,
) -> Result<AsyncTask<SniffRangesTask>> {
  Ok(AsyncTask::new(SniffRangesTask {
    kept: Kept::new(&pieces, size)?,
    said: Said::new(options),
  }))
}

/// What the whole file in `data` is, and what it says of itself.
#[napi]
pub fn specs(data: &[u8], options: Option<Options>) -> Specs {
  let said = Said::new(options);
  mimedown::specs_with(&mimedown::Source::whole(data), said.options()).into()
}

#[napi]
pub fn specs_async(data: &[u8], options: Option<Options>) -> AsyncTask<SpecsTask> {
  AsyncTask::new(SpecsTask {
    bytes: snapshot(data),
    said: Said::new(options),
  })
}

/// The same, from pieces of a file. An answer whose `needs` is not `null` names the range
/// that would complete it: fetch that range and ask again with every piece fetched so far.
#[napi]
pub fn specs_from_ranges(
  pieces: Vec<Piece>,
  size: Option<f64>,
  options: Option<Options>,
) -> Result<Specs> {
  let said = Said::new(options);
  from_pieces(&pieces, size, |source| {
    mimedown::specs_with(source, said.options()).into()
  })
}

#[napi]
pub fn specs_from_ranges_async(
  pieces: Vec<Piece>,
  size: Option<f64>,
  options: Option<Options>,
) -> Result<AsyncTask<SpecsRangesTask>> {
  Ok(AsyncTask::new(SpecsRangesTask {
    kept: Kept::new(&pieces, size)?,
    said: Said::new(options),
  }))
}

// ── The tables ──────────────────────────────────────────────────────────────────────────────

/// The types a file extension is in use for, the usual one first. The dot is optional.
#[napi]
pub fn media_types_of(extension: String) -> Vec<Mime> {
  mimedown::media_types_of(&extension)
    .into_iter()
    .map(Into::into)
    .collect()
}

#[napi]
pub fn media_types_of_async(extension: String) -> AsyncTask<MediaTypesOfTask> {
  AsyncTask::new(MediaTypesOfTask { extension })
}

/// The type a path's extension names, when it names one.
#[napi]
pub fn media_type_of_path(path: String) -> Option<Mime> {
  mimedown::media_type_of_path(&path).map(Into::into)
}

#[napi]
pub fn media_type_of_path_async(path: String) -> AsyncTask<MediaTypeOfPathTask> {
  AsyncTask::new(MediaTypeOfPathTask { path })
}

fn extensions(of_a_type: &[&str]) -> Vec<String> {
  of_a_type
    .iter()
    .map(|extension| extension.to_string())
    .collect()
}

/// The file extensions in use for a type, the usual one first, without the dot.
#[napi]
pub fn extensions_of(mime: String) -> Vec<String> {
  extensions(mimedown::extensions_of(&mime))
}

#[napi]
pub fn extensions_of_async(mime: String) -> AsyncTask<ExtensionsOfTask> {
  AsyncTask::new(ExtensionsOfTask { mime })
}

/// Which of the parsers the addon was built with.
#[napi]
pub fn built_with() -> BuiltWith {
  mimedown::built_with().into()
}

#[napi]
pub fn built_with_async() -> AsyncTask<BuiltWithTask> {
  AsyncTask::new(BuiltWithTask)
}

// ── What takes more than a header ───────────────────────────────────────────────────────────

/// The shape of the sound in an audio file or a film. This decodes the whole of the sound.
#[napi]
pub fn waveform(data: &[u8], options: Option<WaveformOptions>) -> Result<WaveformOutcome> {
  Ok(mimedown::waveform(snapshot(data), waveform_options(options)?).into())
}

#[napi]
pub fn waveform_async(
  data: &[u8],
  options: Option<WaveformOptions>,
) -> Result<AsyncTask<WaveformTask>> {
  Ok(AsyncTask::new(WaveformTask {
    bytes: snapshot(data),
    options: waveform_options(options)?,
  }))
}

/// A still from a film: one keyframe, decoded and written as a picture.
#[napi]
pub fn poster(data: &[u8], options: Option<PosterOptions>) -> Result<PosterOutcome> {
  Ok(mimedown::poster(data, poster_options(options)?).into())
}

#[napi]
pub fn poster_async(data: &[u8], options: Option<PosterOptions>) -> Result<AsyncTask<PosterTask>> {
  Ok(AsyncTask::new(PosterTask {
    bytes: snapshot(data),
    options: poster_options(options)?,
  }))
}

/// Where a film's keyframes are. Nothing is decoded.
#[napi]
pub fn keyframes(data: &[u8]) -> KeyframesOutcome {
  mimedown::keyframes(data).into()
}

#[napi]
pub fn keyframes_async(data: &[u8]) -> AsyncTask<KeyframesTask> {
  AsyncTask::new(KeyframesTask {
    bytes: snapshot(data),
  })
}

// ── A file held for more than one question ──────────────────────────────────────────────────

/// A whole file, copied once and kept, for asking more than one thing of it.
#[napi]
pub struct MimeDown {
  bytes: Arc<Vec<u8>>,
  said: Said,
}

#[napi]
impl MimeDown {
  #[napi(constructor)]
  pub fn new(data: &[u8], options: Option<Options>) -> Self {
    MimeDown {
      bytes: snapshot(data),
      said: Said::new(options),
    }
  }

  #[napi(getter)]
  pub fn byte_size(&self) -> f64 {
    // A file's length is far below the 2^53 at which a number stops being exact.
    self.bytes.len() as f64
  }

  #[napi]
  pub fn sniff(&self) -> Sniffed {
    mimedown::sniff_with(&mimedown::Source::whole(&self.bytes), self.said.options()).into()
  }

  #[napi]
  pub fn sniff_async(&self) -> AsyncTask<SniffTask> {
    AsyncTask::new(SniffTask {
      bytes: Arc::clone(&self.bytes),
      said: self.said.clone(),
    })
  }

  #[napi]
  pub fn specs(&self) -> Specs {
    mimedown::specs_with(&mimedown::Source::whole(&self.bytes), self.said.options()).into()
  }

  #[napi]
  pub fn specs_async(&self) -> AsyncTask<SpecsTask> {
    AsyncTask::new(SpecsTask {
      bytes: Arc::clone(&self.bytes),
      said: self.said.clone(),
    })
  }

  #[napi]
  pub fn waveform(&self, options: Option<WaveformOptions>) -> Result<WaveformOutcome> {
    Ok(mimedown::waveform(Arc::clone(&self.bytes), waveform_options(options)?).into())
  }

  #[napi]
  pub fn waveform_async(
    &self,
    options: Option<WaveformOptions>,
  ) -> Result<AsyncTask<WaveformTask>> {
    Ok(AsyncTask::new(WaveformTask {
      bytes: Arc::clone(&self.bytes),
      options: waveform_options(options)?,
    }))
  }

  #[napi]
  pub fn poster(&self, options: Option<PosterOptions>) -> Result<PosterOutcome> {
    Ok(mimedown::poster(&self.bytes, poster_options(options)?).into())
  }

  #[napi]
  pub fn poster_async(&self, options: Option<PosterOptions>) -> Result<AsyncTask<PosterTask>> {
    Ok(AsyncTask::new(PosterTask {
      bytes: Arc::clone(&self.bytes),
      options: poster_options(options)?,
    }))
  }

  #[napi]
  pub fn keyframes(&self) -> KeyframesOutcome {
    mimedown::keyframes(&self.bytes).into()
  }

  #[napi]
  pub fn keyframes_async(&self) -> AsyncTask<KeyframesTask> {
    AsyncTask::new(KeyframesTask {
      bytes: Arc::clone(&self.bytes),
    })
  }
}
