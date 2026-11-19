use super::*;
use crate::source::Piece;
use crate::{Facts, MediaType, specs, specs_source};

/// A fixture another crate of the workspace tracks.
fn fixture(name: &str) -> Vec<u8> {
  let path = format!(
    "{}/../videodown-node/__test__/fixtures/{name}",
    env!("CARGO_MANIFEST_DIR")
  );
  std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn video(found: crate::Specs) -> VideoFacts {
  match found.facts {
    Facts::Video(facts) => *facts,
    other => panic!(
      "no video facts for {}: {other:?} {:?}",
      found.media_type, found.warnings
    ),
  }
}

#[test]
fn a_video_has_the_facts_its_parser_gives() {
  let cases = [
    ("h264-aac-faststart.mp4", MediaType::Mp4),
    ("h264-aac.mov", MediaType::Mov),
    ("h264-aac.mkv", MediaType::Mkv),
    ("av1-aac.mp4", MediaType::Mp4),
  ];
  for (name, media_type) in cases {
    let bytes = fixture(name);
    let found = specs(&bytes);
    assert_eq!(
      (found.media_type, found.needs, found.warnings.len()),
      (media_type, None, 0),
      "{name}"
    );
    let facts = video(found);
    let theirs = videodown::meta::parse_video(&bytes).unwrap();
    let track = theirs.video.as_ref().unwrap();
    assert_eq!(
      (facts.width, facts.height),
      (track.display_width, track.display_height),
      "{name}"
    );
    assert_eq!(facts.codec.as_deref(), Some(track.codec.as_str()), "{name}");
    assert_eq!(facts.frame_rate, track.frame_rate, "{name}");
    assert_eq!(facts.rotation, Some(track.rotation), "{name}");
    assert_eq!(facts.has_audio, !theirs.audio.is_empty(), "{name}");
    assert_eq!(
      facts.keyframe_times_ms.len(),
      theirs.keyframes.times_sec.len(),
      "{name}"
    );
    let seconds = theirs.duration_sec.unwrap();
    assert_eq!(
      facts.duration_ms,
      Some((seconds * 1000.0).round() as u64),
      "{name}"
    );
    assert!(facts.width > 0 && facts.duration_ms.unwrap() > 0, "{name}");
    assert!(
      facts.resolution.is_some() && facts.orientation.is_some() && facts.bitrate_kbps.is_some(),
      "{name}"
    );
    assert!(facts.aspect_ratio().is_some());
  }
}

#[test]
fn a_file_whose_description_comes_after_its_media_asks_for_the_rest() {
  let bytes = fixture("h264-aac-mdat-first.mp4");
  let size = u64::try_from(bytes.len()).unwrap();
  let pieces = [crate::Piece {
    offset: 0,
    bytes: &bytes[..4096],
  }];
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert!(found.needs.is_some(), "{found:?}");
  assert!(video(specs(&bytes)).has_audio);
}

/// Three quarters of `file`, with and without the file's size beside it.
fn most_of(file: &[u8]) -> (VideoFacts, VideoFacts) {
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..file.len() * 3 / 4],
  }];
  let size = u64::try_from(file.len()).ok();
  let sized = specs_source(&Source::from_pieces(&pieces, size).unwrap());
  assert_eq!(sized.needs, None);
  let unsized_ = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  (video(sized), video(unsized_))
}

#[test]
fn the_rate_of_a_file_is_not_worked_out_from_the_part_of_it_that_was_given() {
  // Its description comes first, so most of the file describes all of it. Its boxes state
  // their sizes, which come to the file's: the rate is the whole file's with or without it.
  let file = fixture("h264-aac-faststart.mp4");
  let whole = video(specs(&file));
  let (sized, unsized_) = most_of(&file);
  assert_eq!(sized.bitrate_kbps, whole.bitrate_kbps);
  assert_eq!(unsized_.bitrate_kbps, whole.bitrate_kbps);

  // The same film with its media box left open, as one written while it is recorded is: the
  // box runs to the end of the file, wherever that is.
  let mut open = file.clone();
  let media = open.windows(4).position(|kind| kind == b"mdat").unwrap();
  open[media - 4..media].copy_from_slice(&[0; 4]);
  let whole = video(specs(&open));
  let (sized, unsized_) = most_of(&open);
  assert_eq!(sized.bitrate_kbps, whole.bitrate_kbps);
  // Without the size there is nothing to divide: three quarters of a file is not the file.
  assert_eq!(unsized_.duration_ms, whole.duration_ms);
  assert_eq!(unsized_.bitrate_kbps, None);
}
