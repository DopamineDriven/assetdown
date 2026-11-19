use super::*;
use crate::source::{ByteRange, Piece};
use crate::testing::image_facts;
use crate::{ColorSpace, ImageDetail, specs, specs_source};

/// A header and the screen's size, with a palette of four colors shared by the frames.
fn start(version: &[u8; 3], width: u16, height: u16) -> Vec<u8> {
  [
    &b"GIF"[..],
    version,
    &width.to_le_bytes(),
    &height.to_le_bytes(),
    &[0x81, 0, 0],
    &[0; 12],
  ]
  .concat()
}

/// How the next frame is shown: whether a color is see-through, and for how many hundredths.
fn control(transparent: bool, hundredths: u16) -> Vec<u8> {
  let [d0, d1] = hundredths.to_le_bytes();
  vec![0x21, 0xF9, 4, u8::from(transparent), d0, d1, 0, 0]
}

fn frame() -> Vec<u8> {
  // Where it sits, its size, no palette of its own; a code size; one run of pixels; the end.
  [
    &[0x2C][..],
    &[0; 4],
    &[1, 0, 1, 0, 0],
    &[2, 2, 0x4C, 0x01, 0],
  ]
  .concat()
}

fn repeat(times: u16) -> Vec<u8> {
  let [l0, l1] = times.to_le_bytes();
  [&[0x21, 0xFF, 11][..], b"NETSCAPE2.0", &[3, 1, l0, l1, 0]].concat()
}

fn detail(facts: &ImageFacts) -> &GifDetail {
  match &facts.detail {
    ImageDetail::Gif(detail) => detail,
    other => panic!("not a GIF's detail: {other:?}"),
  }
}

#[test]
fn a_still_gif_is_one_frame_on_its_screen() {
  let file = [start(b"89a", 320, 240), frame(), vec![0x3B]].concat();
  let facts = image_facts(specs(&file));
  assert_eq!((facts.width, facts.height), (320, 240));
  assert_eq!((facts.frames, facts.animated), (1, false));
  assert_eq!(facts.has_alpha, Some(false));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Indexed, ColorSpace::Unknown)
  );
  assert_eq!(detail(&facts).version, "89a");
  assert_eq!(detail(&facts).global_colors, Some(4));
  assert_eq!(
    (detail(&facts).loop_count, detail(&facts).duration_ms),
    (None, 0)
  );
}

#[test]
fn the_frames_of_an_animation_are_counted_with_how_long_they_last_and_how_often_they_play() {
  let file = [
    start(b"89a", 64, 64),
    repeat(0),
    control(true, 10),
    frame(),
    control(false, 25),
    frame(),
    control(false, 5),
    frame(),
    vec![0x3B],
  ]
  .concat();
  let facts = image_facts(specs(&file));
  assert_eq!((facts.frames, facts.animated), (3, true));
  // One frame names a color that is see-through.
  assert_eq!(facts.has_alpha, Some(true));
  assert_eq!(detail(&facts).loop_count, Some(0));
  assert_eq!(detail(&facts).duration_ms, 400);
}

#[test]
fn an_older_gif_has_no_palette_to_share_when_it_says_so() {
  let mut file = start(b"87a", 2, 2);
  file[10] = 0;
  file.truncate(13);
  file.extend([frame(), vec![0x3B]].concat());
  let facts = image_facts(specs(&file));
  assert_eq!(
    (
      detail(&facts).version.as_str(),
      detail(&facts).global_colors
    ),
    ("87a", None)
  );
  assert_eq!(facts.frames, 1);
}

#[test]
fn a_gif_cut_short_gives_the_frames_it_reached_and_says_so() {
  let file = [start(b"89a", 64, 64), frame(), frame(), frame(), vec![0x3B]].concat();
  let cut = specs(&file[..file.len() - 8]);
  assert_eq!(
    cut.warnings,
    ["the file ends before the block that closes it"]
  );
  assert_eq!(image_facts(cut).frames, 2);
}

#[test]
fn the_head_of_a_gif_asks_for_the_rest_because_the_frames_are_counted_to_the_end() {
  let file = [start(b"89a", 64, 64), frame(), frame(), vec![0x3B]].concat();
  let size = u64::try_from(file.len()).unwrap();
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..40],
  }];
  let head = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(head.needs, Some(ByteRange::forward(40, size - 40)));
  let facts = image_facts(head);
  assert_eq!((facts.width, facts.frames), (64, 1));
  // With no size to go by, the next stretch is asked for.
  let head = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  assert_eq!(head.needs, Some(ByteRange::forward(40, 1 << 20)));
}
