use super::*;
use crate::testing::{png_header, png_of};

fn whole(bytes: &[u8]) -> Arc<Vec<u8>> {
  Arc::new(bytes.to_vec())
}

fn none_of<T: std::fmt::Debug>(outcome: Outcome<T>) -> WhyNot {
  match outcome {
    Outcome::Nothing(why) => why,
    Outcome::Made(made) => panic!("something was made: {made:?}"),
  }
}

#[test]
fn a_picture_has_no_waveform_no_poster_and_no_keyframes() {
  let picture = png_of(&[png_header(8, 8, 8, 6)]);
  let no_reader = WhyNot::NoReader(MediaType::Png);
  assert_eq!(
    none_of(waveform(whole(&picture), WaveformOptions::default())),
    no_reader
  );
  assert_eq!(
    none_of(poster(&picture, PosterOptions::default())),
    no_reader
  );
  assert_eq!(none_of(keyframes(&picture)), no_reader);
}

#[test]
fn bytes_that_name_no_type_have_none_of_them_either() {
  let nothing = [0u8; 64];
  let no_reader = WhyNot::NoReader(MediaType::OctetStream);
  assert_eq!(
    none_of(waveform(whole(&nothing), WaveformOptions::default())),
    no_reader
  );
  assert_eq!(
    none_of(poster(&nothing, PosterOptions::default())),
    no_reader
  );
  assert_eq!(none_of(keyframes(&nothing)), no_reader);
}

#[test]
fn sound_in_a_type_that_is_not_decoded_here_says_which_type_it_is() {
  let flac = b"fLaC\0\0\0\x22\x10\0\x10\0\0\0\x0E\0\x10\x2C\x0A\xC4\x42\xF0\0\0\0\0";
  assert_eq!(
    none_of(waveform(whole(flac), WaveformOptions::default())),
    WhyNot::NoReader(MediaType::Flac)
  );
  // Sound has no still to take and no keyframes.
  let wave = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xAC\0\0\x88\x58\x01\0\x02\0\x10\0data\0\0\0\0";
  assert_eq!(
    none_of(poster(wave, PosterOptions::default())),
    WhyNot::NoReader(MediaType::Wav)
  );
  assert_eq!(none_of(keyframes(wave)), WhyNot::NoReader(MediaType::Wav));
}

#[cfg(not(feature = "audio"))]
#[test]
fn without_the_audio_feature_an_audio_file_s_waveform_is_said_to_be_switched_off() {
  let wave = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xAC\0\0\x88\x58\x01\0\x02\0\x10\0data\0\0\0\0";
  assert_eq!(
    none_of(waveform(whole(wave), WaveformOptions::default())),
    WhyNot::FeatureOff("audio")
  );
}

#[cfg(not(feature = "video"))]
#[test]
fn without_the_video_feature_what_a_film_has_is_said_to_be_switched_off() {
  use crate::testing::ftyp;
  let film = [
    ftyp(b"isom", &[b"isom", b"mp42"]),
    vec![0, 0, 0, 8, b'm', b'o', b'o', b'v'],
  ]
  .concat();
  let off = WhyNot::FeatureOff("video");
  assert_eq!(
    none_of(waveform(whole(&film), WaveformOptions::default())),
    off
  );
  assert_eq!(none_of(poster(&film, PosterOptions::default())), off);
  assert_eq!(none_of(keyframes(&film)), off);
}

#[test]
fn every_reason_has_a_name() {
  let reasons = [
    (WhyNot::NoReader(MediaType::Png), "noReader"),
    (WhyNot::FeatureOff("audio"), "featureOff"),
    (WhyNot::Absent, "absent"),
    (WhyNot::Unsupported("av1".to_string()), "unsupported"),
    (WhyNot::Truncated, "truncated"),
    (WhyNot::Failed("it broke".to_string()), "failed"),
  ];
  for (reason, name) in reasons {
    assert_eq!(reason.as_str(), name);
  }
  assert_eq!(PosterFormat::Webp.media_type(), MediaType::Webp);
}

/// A fixture another crate of the workspace tracks.
#[cfg(any(feature = "audio", feature = "video"))]
fn fixture(krate: &str, name: &str) -> Vec<u8> {
  let path = format!(
    "{}/../{krate}/__test__/fixtures/{name}",
    env!("CARGO_MANIFEST_DIR")
  );
  std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[cfg(any(feature = "audio", feature = "video"))]
fn made<T: std::fmt::Debug>(outcome: Outcome<T>) -> T {
  match outcome {
    Outcome::Made(made) => made,
    Outcome::Nothing(why) => panic!("nothing was made: {why:?}"),
  }
}

#[cfg(feature = "audio")]
mod of_sound {
  use super::*;

  #[test]
  fn an_audio_file_gives_the_shape_of_its_sound() {
    for name in ["stereo.wav", "cbr.mp3"] {
      let bytes = Arc::new(fixture("audiodown-node", name));
      let shape = made(waveform(Arc::clone(&bytes), WaveformOptions::default()));
      assert_eq!(shape.channels.len(), shape.channel_count, "{name}");
      // A thousand and twenty-four peaks, each with where it begins and the last with where
      // it ends.
      assert_eq!(shape.boundaries.len(), 1025, "{name}");
      for channel in &shape.channels {
        let lengths = [
          channel.min.len(),
          channel.max.len(),
          channel.peak.len(),
          channel.rms.len(),
        ];
        assert_eq!(lengths, [1024; 4], "{name}");
      }
      // It is what `audiodown` gives when asked itself.
      let pcm =
        audiodown::decode::decode(bytes, audiodown::decode::DecodeConfig::default()).unwrap();
      let theirs =
        audiodown::waveform::peaks(&pcm, audiodown::waveform::WaveformConfig::default()).unwrap();
      assert_eq!(
        (shape.sample_rate, shape.frame_count),
        (theirs.sample_rate, theirs.frame_count),
        "{name}"
      );
      assert_eq!(shape.duration_ms, theirs.duration_sec * 1000.0, "{name}");
      assert_eq!(shape.channels[0].max, theirs.channels[0].max, "{name}");
      assert_eq!(shape.channels[0].rms, theirs.channels[0].rms, "{name}");
    }
  }

  #[test]
  fn a_waveform_can_be_cut_by_the_frame() {
    let bytes = Arc::new(fixture("audiodown-node", "stereo.wav"));
    let options = WaveformOptions {
      resolution: Resolution::FramesPerPeak(1000),
      ..WaveformOptions::default()
    };
    let shape = made(waveform(bytes, options));
    assert_eq!(
      shape.channels[0].peak.len(),
      shape.frame_count.div_ceil(1000)
    );
    assert_eq!(shape.boundaries.get(1), Some(&1000));
  }

  #[test]
  fn the_usual_options_are_the_ones_audiodown_has() {
    let ours = WaveformOptions::default();
    let decode = audiodown::decode::DecodeConfig::default();
    assert_eq!(
      (ours.gapless, ours.strict, ours.max_decoded_samples),
      (decode.gapless, decode.strict, decode.max_decoded_samples)
    );
    let cut = audiodown::waveform::WaveformConfig::default();
    assert!(matches!(
      cut.resolution,
      audiodown::waveform::Resolution::PeakCount(1024)
    ));
    assert_eq!(ours.parallel, cut.parallel);
  }

  #[test]
  fn sound_that_cannot_be_decoded_says_what_went_wrong() {
    // A WAVE file's header with a format no decoder has.
    let broken = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\xEE\xEE\x01\0\x44\xAC\0\0\x88\x58\x01\0\x02\0\x10\0data\0\0\0\0";
    let why = none_of(waveform(whole(broken), WaveformOptions::default()));
    assert!(
      matches!(&why, WhyNot::Failed(reason) if !reason.is_empty()),
      "{why:?}"
    );
  }
}

#[cfg(feature = "video")]
mod of_film {
  use super::*;
  use crate::{Facts, specs};

  #[test]
  fn a_film_gives_the_shape_of_its_sound() {
    let bytes = Arc::new(fixture("videodown-node", "h264-aac-faststart.mp4"));
    let shape = made(waveform(bytes, WaveformOptions::default()));
    assert_eq!(shape.channels.len(), shape.channel_count);
    assert_eq!(shape.channels[0].peak.len(), 1024);
    assert!(shape.duration_ms > 0.0);
  }

  #[test]
  fn a_film_with_no_sound_has_no_waveform() {
    let bytes = Arc::new(fixture("videodown-node", "silent.mp4"));
    assert_eq!(
      none_of(waveform(bytes, WaveformOptions::default())),
      WhyNot::Absent
    );
  }

  #[test]
  fn a_film_of_which_only_the_start_was_given_gives_no_waveform() {
    let film = fixture("videodown-node", "h264-aac-mdat-first.mp4");
    let start = Arc::new(film[..film.len() / 2].to_vec());
    assert_eq!(
      none_of(waveform(start, WaveformOptions::default())),
      WhyNot::Truncated
    );
  }

  #[test]
  fn a_poster_is_a_picture_of_the_size_the_film_is_shown_at() {
    let film = fixture("videodown-node", "h264-aac-faststart.mp4");
    let Facts::Video(shown) = specs(&film).facts else {
      panic!("no video facts");
    };
    for format in [PosterFormat::Jpeg, PosterFormat::Png, PosterFormat::Webp] {
      let options = PosterOptions {
        format,
        ..PosterOptions::default()
      };
      let still = made(poster(&film, options));
      assert_eq!(still.media_type, format.media_type());
      assert_eq!((still.width, still.height), (shown.width, shown.height));
      // The picture is what it says it is: it is named and measured as any other.
      let found = specs(&still.data);
      assert_eq!(found.media_type, format.media_type());
      let Facts::Image(picture) = found.facts else {
        panic!("the poster has no image facts");
      };
      assert_eq!((picture.width, picture.height), (still.width, still.height));
    }
  }

  #[test]
  fn a_poster_of_a_film_that_is_turned_is_turned_with_it() {
    let upright = made(poster(
      &fixture("videodown-node", "h264-aac-faststart.mp4"),
      PosterOptions::default(),
    ));
    let turned = made(poster(
      &fixture("videodown-node", "rotated-90.mp4"),
      PosterOptions::default(),
    ));
    assert_eq!(
      (turned.width, turned.height),
      (upright.height, upright.width)
    );
  }

  #[test]
  fn a_poster_can_be_taken_from_later_in_the_film() {
    let film = fixture(
      "videodown-node",
      "video-gemini-omni-1.1-flash-720p-16x9-008-delta_data.mp4",
    );
    let index = made(keyframes(&film));
    let later = *index.times_ms.last().unwrap();
    assert!(later > 0.0);
    let options = PosterOptions {
      at_ms: Some(later + 1.0),
      ..PosterOptions::default()
    };
    let still = made(poster(&film, options));
    assert_eq!(still.time_ms, later);
    assert_eq!(
      made(poster(&film, PosterOptions::default())).time_ms,
      index.times_ms[0]
    );
  }

  #[test]
  fn a_film_in_a_coding_that_is_not_decoded_here_has_no_poster_and_says_which_coding() {
    let film = fixture("videodown-node", "av1-aac.mp4");
    assert_eq!(
      none_of(poster(&film, PosterOptions::default())),
      WhyNot::Unsupported("av1".to_string())
    );
  }

  #[test]
  fn the_keyframes_of_a_film_are_listed_with_where_the_list_came_from() {
    let film = fixture("videodown-node", "h264-aac-faststart.mp4");
    let index = made(keyframes(&film));
    assert_eq!((index.source.as_str(), index.complete), ("stss", true));
    assert!(!index.times_ms.is_empty());
    assert!(index.times_ms.is_sorted());
    // The same times `specs` gives.
    let Facts::Video(facts) = specs(&film).facts else {
      panic!("no video facts");
    };
    assert_eq!(index.times_ms, facts.keyframe_times_ms);
    let webm = made(keyframes(&fixture("videodown-node", "vp9-opus.webm")));
    assert_eq!(webm.source, "cues");
  }

  #[test]
  fn the_usual_options_are_the_ones_videodown_has() {
    let ours = PosterOptions::default();
    let theirs = videodown::poster::PosterConfig::default();
    assert_eq!(
      (ours.at_ms, ours.quality),
      (theirs.time_sec, theirs.quality)
    );
    assert_eq!(theirs.format, videodown::poster::PosterFormat::Jpeg);
    let decode = videodown::audio::DecodeConfig::default();
    let sound = WaveformOptions::default();
    assert_eq!(
      (sound.gapless, sound.strict, sound.max_decoded_samples),
      (decode.gapless, decode.strict, decode.max_decoded_samples)
    );
  }
}

#[test]
fn the_crate_says_which_parsers_it_was_built_with() {
  let built = crate::built_with();
  // A verb that needs a parser is switched off exactly when the parser is not built in.
  let wave = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xAC\0\0\x88\x58\x01\0\x02\0\x10\0data\0\0\0\0";
  let off = matches!(
    waveform(whole(wave), WaveformOptions::default()),
    Outcome::Nothing(WhyNot::FeatureOff("audio"))
  );
  assert_eq!(built.audio, !off);
  assert_eq!(
    (built.pdf, built.office, built.video),
    (
      cfg!(feature = "pdf"),
      cfg!(feature = "office"),
      cfg!(feature = "video")
    )
  );
}
