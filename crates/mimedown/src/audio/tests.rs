use super::*;
use crate::source::Piece;
use crate::{Facts, MediaType, specs, specs_source};

fn audio(found: crate::Specs) -> AudioFacts {
  match found.facts {
    Facts::Audio(facts) => *facts,
    other => panic!(
      "no audio facts for {}: {other:?} {:?}",
      found.media_type, found.warnings
    ),
  }
}

/// A WAVE file of 16-bit stereo sound at 44.1 kHz that plays for `seconds`, with a title and
/// an artist in its list of information.
fn wav(seconds: u32) -> Vec<u8> {
  let data = vec![0u8; usize::try_from(176_400 * seconds).unwrap()];
  let info = [
    &b"INFO"[..],
    b"INAM",
    &6u32.to_le_bytes(),
    b"Dusk\0\0",
    b"IART",
    &10u32.to_le_bytes(),
    b"A Harbor\0\0",
    b"ICRD",
    &12u32.to_le_bytes(),
    b"2026-11-17\0\0",
  ]
  .concat();
  let format = [
    &1u16.to_le_bytes()[..],
    &2u16.to_le_bytes(),
    &44_100u32.to_le_bytes(),
    &176_400u32.to_le_bytes(),
    &4u16.to_le_bytes(),
    &16u16.to_le_bytes(),
  ]
  .concat();
  let chunks = [
    &b"fmt "[..],
    &16u32.to_le_bytes(),
    &format,
    b"LIST",
    &u32::try_from(info.len()).unwrap().to_le_bytes(),
    &info,
    b"data",
    &u32::try_from(data.len()).unwrap().to_le_bytes(),
    &data,
  ]
  .concat();
  [
    &b"RIFF"[..],
    &u32::try_from(chunks.len() + 4).unwrap().to_le_bytes(),
    b"WAVE",
    &chunks,
  ]
  .concat()
}

/// An MP3 of `frames` frames at 128 kbit/s and 44.1 kHz in stereo, behind a tag that names it.
fn mp3(frames: usize) -> Vec<u8> {
  let text = |id: &[u8; 4], value: &str| {
    [
      &id[..],
      &u32::try_from(value.len() + 1).unwrap().to_be_bytes(),
      &[0, 0, 0],
      value.as_bytes(),
    ]
    .concat()
  };
  let tag_frames = [
    text(b"TIT2", "Dusk"),
    text(b"TPE1", "A Harbor"),
    text(b"TYER", "2026"),
  ]
  .concat();
  let size = u32::try_from(tag_frames.len()).unwrap();
  let synchsafe = [
    (size >> 21) as u8 & 0x7F,
    (size >> 14) as u8 & 0x7F,
    (size >> 7) as u8 & 0x7F,
    size as u8 & 0x7F,
  ];
  let mut file = [&b"ID3\x03\0\0"[..], &synchsafe, &tag_frames].concat();
  for _ in 0..frames {
    let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
    frame.resize(417, 0);
    file.extend(frame);
  }
  file
}

#[test]
fn a_wave_file_says_how_long_it_plays_and_at_what_rate() {
  let found = specs(&wav(2));
  assert_eq!(
    (found.media_type, found.needs, found.warnings.len()),
    (MediaType::Wav, None, 0)
  );
  let facts = audio(found);
  assert_eq!(facts.duration_ms, Some(2000));
  assert_eq!(
    (facts.sample_rate, facts.channels, facts.bitrate_kbps),
    (Some(44_100), Some(2), Some(1411))
  );
  assert_eq!(facts.codec.as_deref(), Some("pcm"));
  assert_eq!(
    (facts.title.as_deref(), facts.artist.as_deref(), facts.year),
    (Some("Dusk"), Some("A Harbor"), Some(2026))
  );
}

#[test]
fn an_mp3_says_how_long_it_plays_and_what_its_tag_calls_it() {
  let facts = audio(specs(&mp3(100)));
  // A hundred frames of 1,152 samples at 44.1 kHz.
  assert_eq!(facts.duration_ms, Some(2612));
  assert_eq!(
    (facts.sample_rate, facts.channels, facts.bitrate_kbps),
    (Some(44_100), Some(2), Some(128))
  );
  assert_eq!(facts.codec.as_deref(), Some("mp3"));
  assert_eq!(
    (facts.title.as_deref(), facts.artist.as_deref(), facts.year),
    (Some("Dusk"), Some("A Harbor"), Some(2026))
  );
  assert_eq!((facts.album, facts.genre), (None, None));
}

#[test]
fn part_of_a_file_gives_what_its_head_says_and_asks_for_the_rest_to_say_how_long_it_plays() {
  let file = mp3(100);
  let size = u64::try_from(file.len()).unwrap();
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..4096],
  }];
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(
    found.needs,
    Some(crate::ByteRange {
      offset: 4096,
      length: size - 4096,
      from_end: false
    })
  );
  let facts = audio(found);
  // The length is found by walking every frame, and these are not all of them.
  assert_eq!(facts.duration_ms, None);
  assert_eq!(
    (facts.sample_rate, facts.title.as_deref()),
    (Some(44_100), Some("Dusk"))
  );
}

#[test]
fn the_year_is_the_four_digits_a_date_holds() {
  assert_eq!(year(Some("2026-11-17")), Some(2026));
  assert_eq!(year(Some("17 Nov 1999")), Some(1999));
  assert_eq!(year(Some("'99")), None);
  assert_eq!(year(None), None);
}

/// The same file with its list of information after the sound, where some writers put it.
fn wav_with_its_information_last(seconds: u32) -> Vec<u8> {
  let file = wav(seconds);
  // The header and the format chunk come to 36 bytes; the list follows, with its own length.
  let length = 8 + usize::try_from(u32::from_le_bytes(file[40..44].try_into().unwrap())).unwrap();
  let (list, sound) = file[36..].split_at(length);
  [&file[..36], sound, list].concat()
}

/// What the head of `file` gives, and then what the head and every range asked for give.
fn head_of(file: &[u8], size: Option<u64>) -> crate::Specs {
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..4096],
  }];
  specs_source(&Source::from_pieces(&pieces, size).unwrap())
}

#[test]
fn a_wave_file_s_head_says_how_long_it_plays_and_its_sound_is_not_asked_for() {
  let file = wav(30);
  let found = head_of(&file, Some(u64::try_from(file.len()).unwrap()));
  // The header states how many bytes of sound there are, and nothing follows them.
  assert_eq!(found.needs, None);
  let facts = audio(found);
  assert_eq!(facts.duration_ms, Some(30_000));
  assert_eq!(facts, audio(specs(&file)));
}

#[test]
fn what_follows_a_wave_file_s_sound_is_asked_for_and_the_sound_is_not() {
  let file = wav_with_its_information_last(30);
  let size = u64::try_from(file.len()).unwrap();
  let sound_ends = 44 + 176_400 * 30;
  let found = head_of(&file, Some(size));
  // What the file is called may be in what follows the sound, so that is asked for.
  assert_eq!(
    found.needs,
    Some(crate::ByteRange {
      offset: sound_ends,
      length: size - sound_ends,
      from_end: false
    })
  );
  let facts = audio(found);
  assert_eq!((facts.duration_ms, &facts.title), (Some(30_000), &None));

  let after = usize::try_from(sound_ends).unwrap();
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..4096],
    },
    Piece {
      offset: sound_ends,
      bytes: &file[after..],
    },
  ];
  let both = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(both.needs, None);
  let facts = audio(both);
  assert_eq!(facts.title.as_deref(), Some("Dusk"));
  assert_eq!(facts, audio(specs(&file)));
}

#[test]
fn a_wave_file_whose_size_is_not_known_is_asked_for_what_may_follow_its_sound() {
  let file = wav(30);
  let found = head_of(&file, None);
  let needs = found.needs.expect("whether anything follows the sound");
  // The header, the format, the list of information (64 bytes) and then the sound.
  assert_eq!(needs.offset, 44 + 64 + 176_400 * 30);
  assert_eq!(audio(found).duration_ms, Some(30_000));
}

#[test]
fn a_wave_file_shorter_than_it_says_is_asked_for_whole() {
  let mut file = wav(30);
  file.truncate(file.len() - 1_000_000);
  let size = u64::try_from(file.len()).unwrap();
  let found = head_of(&file, Some(size));
  assert_eq!(
    found.needs,
    Some(crate::ByteRange {
      offset: 4096,
      length: size - 4096,
      from_end: false
    })
  );
  // How long it plays is how much sound is really there, which the head cannot say.
  assert_eq!(audio(found).duration_ms, None);
}

#[test]
fn what_follows_a_wave_file_s_sound_is_asked_for_once_when_its_size_is_not_known() {
  let file = wav_with_its_information_last(2);
  let asked = head_of(&file, None).needs.unwrap();
  // Given what was asked for, the answer has what the file is called and asks no more.
  let start = usize::try_from(asked.offset).unwrap();
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..4096],
    },
    Piece {
      offset: asked.offset,
      bytes: &file[start..],
    },
  ];
  let found = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  assert_eq!(found.needs, None);
  let facts = audio(found);
  assert_eq!(facts.title.as_deref(), Some("Dusk"));
  assert_eq!(facts.duration_ms, Some(2_000));
}
