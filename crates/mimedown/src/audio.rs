//! What an audio file says of itself: how long it plays, at what rate, and what its tags call
//! it. The facts are `audiodown`'s, read when the `audio` feature is on; without it an audio
//! file is named and nothing more.

#[cfg(feature = "audio")]
use crate::image::Reading;
#[cfg(feature = "audio")]
use crate::source::Source;

/// What an audio file says of itself. The fields are the columns of an audio file's metadata
/// row. A fact that was not read is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioFacts {
  /// How long the file plays, in milliseconds.
  pub duration_ms: Option<u64>,
  /// Kilobits a second, averaged over the file when its rate varies.
  pub bitrate_kbps: Option<u32>,
  /// Samples a second.
  pub sample_rate: Option<u32>,
  pub channels: Option<u32>,
  /// How the sound is coded: `mp3`, `pcm`.
  pub codec: Option<String>,
  pub title: Option<String>,
  pub artist: Option<String>,
  pub album: Option<String>,
  pub year: Option<u16>,
  pub genre: Option<String>,
}

/// The year a tag's date begins with or holds: the first four digits in a row.
#[cfg(feature = "audio")]
fn year(written: Option<&str>) -> Option<u16> {
  let written = written?;
  let digits: Vec<(usize, char)> = written
    .char_indices()
    .filter(|(_, c)| c.is_ascii_digit())
    .collect();
  digits
    .windows(4)
    .find(|run| run[3].0 - run[0].0 == 3)
    .and_then(|run| written.get(run[0].0..=run[3].0)?.parse().ok())
}

#[cfg(feature = "audio")]
fn milliseconds(seconds: Option<f64>) -> Option<u64> {
  let milliseconds = (seconds? * 1000.0).round();
  // Checked to be a whole number that is not below zero, so the conversion keeps its value.
  (milliseconds.is_finite() && milliseconds >= 0.0).then_some(milliseconds as u64)
}

/// Bits a second as kilobits a second, to the nearest one.
#[cfg(feature = "audio")]
fn kilobits(bits: u64) -> Option<u32> {
  u32::try_from((bits + 500) / 1000).ok()
}

/// What a WAVE file's header states of the sound it holds: where the sound starts, how many
/// bytes of it there are, and how many of them go to a second.
#[cfg(feature = "audio")]
struct StatedSound {
  starts: u64,
  length: u64,
  byte_rate: u32,
}

/// What the header in `head` states. None for a length too long for the four bytes it is
/// written in, which is kept elsewhere, and for a length of nothing, which is a file that
/// was still being written.
#[cfg(feature = "audio")]
fn stated_sound(head: &[u8], wav: &audiodown::metadata::WavMeta) -> Option<StatedSound> {
  let starts = wav.data_offset?;
  let written = head.get(starts.checked_sub(4)?..starts)?;
  let length = u32::from_le_bytes(*written.first_chunk()?);
  let stated = length != 0 && length != u32::MAX && wav.byte_rate > 0;
  stated.then_some(StatedSound {
    starts: u64::try_from(starts).ok()?,
    length: u64::from(length),
    byte_rate: wav.byte_rate,
  })
}

/// What a WAVE file's tags call it, when they stand after its sound: the file's header as far
/// as the sound, a sound of no length, and then what follows the sound, read as a file.
#[cfg(feature = "audio")]
fn tags_after_the_sound(head: &[u8], stated: &StatedSound, after: &[u8]) -> Option<AudioFacts> {
  use audiodown::metadata::{AudioMeta, parse_audio};

  let starts = usize::try_from(stated.starts).ok()?;
  let header = head.get(..starts.checked_sub(4)?)?;
  let file = [header, &[0; 4], after].concat();
  match parse_audio(&file).ok()? {
    AudioMeta::Wav(wav) => Some(AudioFacts {
      year: year(wav.tags.date.as_deref()),
      title: wav.tags.title,
      artist: wav.tags.artist,
      album: wav.tags.album,
      genre: wav.tags.genre,
      ..AudioFacts::default()
    }),
    AudioMeta::Mp3(_) => None,
  }
}

/// The facts of an MP3 or a WAVE file.
///
/// From part of a file, a WAVE file's header says how long it plays, so its sound is never
/// asked for: only what follows the sound is, since what the file is called may be there. An
/// MP3's length is given when the file states it up front; otherwise the rest is asked for,
/// since the length is found by walking the sound itself.
#[cfg(feature = "audio")]
pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<AudioFacts> {
  use crate::source::{ByteRange, Missing};
  use audiodown::metadata::{AudioMeta, DurationSource, MpegLayer, parse_audio};

  let whole = source.whole_file();
  let bytes = whole.unwrap_or_else(|| source.head(usize::MAX));
  let meta = match parse_audio(bytes) {
    Ok(meta) => meta,
    Err(error) => {
      match whole {
        Some(_) => reading.warn(format!("the file could not be read as audio: {error}")),
        None => drop(
          reading
            .needs
            .get_or_insert(source.rest_from(source.head_len())),
        ),
      }
      return None;
    }
  };
  let stated = match &meta {
    AudioMeta::Wav(wav) if whole.is_none() => stated_sound(bytes, wav),
    _ => None,
  };
  let (mut facts, stated_up_front) = match meta {
    AudioMeta::Mp3(mp3) => {
      let frame = mp3.frame.as_ref();
      let codec = match frame.map(|frame| frame.layer) {
        Some(MpegLayer::I) => "mp1",
        Some(MpegLayer::II) => "mp2",
        _ => "mp3",
      };
      // The rate over the whole of the sound, which is the first frame's when it never varies.
      let average = mp3
        .walk
        .as_ref()
        .and_then(|walk| kilobits(u64::from(walk.bitrate_bps())))
        .filter(|kbps| *kbps > 0);
      let facts = AudioFacts {
        duration_ms: milliseconds(mp3.duration_sec),
        bitrate_kbps: average.or(frame.map(|frame| frame.bitrate_kbps)),
        sample_rate: frame.map(|frame| frame.sample_rate),
        channels: frame.map(|frame| frame.channels),
        codec: Some(codec.to_string()),
        year: year(mp3.tags.year.as_deref()),
        title: mp3.tags.title,
        artist: mp3.tags.artist,
        album: mp3.tags.album,
        genre: mp3.tags.genre,
      };
      (facts, mp3.duration_source == Some(DurationSource::Xing))
    }
    AudioMeta::Wav(wav) => {
      let known = wav.sample_rate > 0;
      let facts = AudioFacts {
        duration_ms: milliseconds(wav.duration_sec),
        bitrate_kbps: known
          .then(|| kilobits(u64::from(wav.byte_rate) * 8))
          .flatten(),
        sample_rate: known.then_some(wav.sample_rate),
        channels: known.then_some(u32::from(wav.channels)),
        codec: Some(wav.codec()),
        year: year(wav.tags.date.as_deref()),
        title: wav.tags.title,
        artist: wav.tags.artist,
        album: wav.tags.album,
        genre: wav.tags.genre,
      };
      (facts, wav.duration_source == Some(DurationSource::Fact))
    }
  };
  if whole.is_none() {
    // Where the sound ends, by the header's account, and whether the file is long enough to
    // hold it.
    let sound_ends = stated.as_ref().map(|stated| stated.starts + stated.length);
    let holds = |ends: u64| source.size().is_none_or(|size| ends <= size);
    let asked = match (&stated, sound_ends) {
      (Some(stated), Some(ends)) if holds(ends) => {
        if !stated_up_front {
          facts.duration_ms =
            milliseconds(Some(stated.length as f64 / f64::from(stated.byte_rate)));
        }
        // A chunk of odd length is followed by a byte of padding.
        let follows = ends + (stated.length & 1);
        let (after, asked) = match source.size() {
          // Whether anything follows the sound is not known until it is asked for. Once it
          // was given, what is there is read and nothing more is asked for: with no size to
          // go by, nothing says where the file ends.
          None => match source.given_from(follows) {
            [] => (None, Some(ByteRange::forward(follows, 8))),
            after => (Some(after), None),
          },
          // Room for at least one chunk's header after the sound.
          Some(size) if size >= follows + 8 => {
            let wanted = usize::try_from(size - follows).unwrap_or(usize::MAX);
            match source.read(follows, wanted) {
              Ok(after) => (Some(after), None),
              Err(Missing::NotGiven(range)) => (None, Some(range)),
              Err(Missing::PastEnd) => (None, None),
            }
          }
          Some(_) => (None, None),
        };
        if let Some(called) = after.and_then(|after| tags_after_the_sound(bytes, stated, after)) {
          facts.title = called.title.or(facts.title);
          facts.artist = called.artist.or(facts.artist);
          facts.album = called.album.or(facts.album);
          facts.genre = called.genre.or(facts.genre);
          facts.year = called.year.or(facts.year);
        }
        asked
      }
      _ => {
        if !stated_up_front {
          facts.duration_ms = None;
          facts.bitrate_kbps = facts
            .bitrate_kbps
            .filter(|_| matches!(facts.codec.as_deref(), Some("pcm")));
        }
        Some(source.rest_from(source.head_len()))
      }
    };
    if let Some(range) = asked {
      reading.needs.get_or_insert(range);
    }
  }
  Some(facts)
}

#[cfg(all(test, feature = "audio"))]
mod tests;
