//! What a video file says of itself: its size on screen, how long it plays, how it is coded.
//! The facts are `videodown`'s, read when the `video` feature is on; without it a video file
//! is named and nothing more.

#[cfg(feature = "video")]
use crate::image::Reading;
#[cfg(feature = "video")]
use crate::source::Source;

/// What a video file says of itself. The fields are the columns of a video's metadata row.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VideoFacts {
  /// The size the picture is shown at, once it is turned and its pixels are made square.
  pub width: u32,
  pub height: u32,
  /// How long the file plays, in milliseconds.
  pub duration_ms: Option<u64>,
  /// Frames a second.
  pub frame_rate: Option<f64>,
  /// Kilobits a second over the whole file.
  pub bitrate_kbps: Option<u32>,
  /// How the picture is coded: `h264`, `hevc`, `av1`, `vp9`.
  pub codec: Option<String>,
  pub has_audio: bool,
  /// `1080p`, `720p`, `4K`: the name of the shorter side.
  pub resolution: Option<String>,
  /// `landscape`, `portrait` or `square`.
  pub orientation: Option<String>,
  /// How far the picture is turned for display, in degrees: 0, 90, 180 or 270.
  pub rotation: Option<u32>,
  /// When each keyframe comes, in milliseconds from the start.
  pub keyframe_times_ms: Vec<f64>,
}

impl VideoFacts {
  /// Width over height. None for a file that states no size.
  pub fn aspect_ratio(&self) -> Option<f64> {
    (self.height != 0).then(|| f64::from(self.width) / f64::from(self.height))
  }
}

/// The facts of an MP4, a MOV, a WebM or a Matroska file. A file read in part gives what its
/// head holds, and asks for the rest when the head does not hold the whole description.
#[cfg(feature = "video")]
pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<VideoFacts> {
  use videodown::meta::{Orientation, parse_video, resolution_label};

  let whole = source.whole_file();
  let bytes = whole.unwrap_or_else(|| source.head(usize::MAX));
  let meta = match parse_video(bytes) {
    Ok(meta) => meta,
    Err(error) => {
      match whole {
        Some(_) => reading.warn(format!("the file could not be read as video: {error}")),
        None => drop(
          reading
            .needs
            .get_or_insert(source.rest_from(source.head_len())),
        ),
      }
      return None;
    }
  };
  if whole.is_none() && !meta.complete {
    reading
      .needs
      .get_or_insert(source.rest_from(source.head_len()));
  }
  // How many bytes the file has. Part of a file whose size is not known does not say, and
  // then only a size the container itself states can give a rate.
  let size = match source.size() {
    Some(size) => usize::try_from(size).ok(),
    None => meta.declared_size.map(|_| bytes.len()),
  };
  let milliseconds = |seconds: f64| {
    let milliseconds = (seconds * 1000.0).round();
    // Checked to be a whole number that is not below zero, so the conversion keeps its value.
    (milliseconds.is_finite() && milliseconds >= 0.0).then_some(milliseconds as u64)
  };
  let video = meta.video.as_ref();
  let (width, height) = video.map_or((0, 0), |video| (video.display_width, video.display_height));
  Some(VideoFacts {
    width,
    height,
    duration_ms: meta.duration_sec.and_then(milliseconds),
    frame_rate: video.and_then(|video| video.frame_rate),
    bitrate_kbps: size
      .and_then(|size| meta.bitrate_bps(size))
      .map(|bits| bits / 1000),
    codec: video.map(|video| video.codec.clone()),
    has_audio: !meta.audio.is_empty(),
    resolution: resolution_label(width, height),
    orientation: Orientation::of(width, height)
      .map(|orientation| format!("{orientation:?}").to_ascii_lowercase()),
    rotation: video.map(|video| video.rotation),
    keyframe_times_ms: meta
      .keyframes
      .times_sec
      .iter()
      .map(|seconds| seconds * 1000.0)
      .collect(),
  })
}

#[cfg(all(test, feature = "video"))]
mod tests;
