use super::*;
use crate::source::Piece;
use crate::testing::{XMP_PACKET, exif_block, icc_profile, image_facts};
use crate::{MediaType, specs, specs_source};

fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
  let length = u32::try_from(payload.len()).unwrap();
  let mut chunk = [&kind[..], &length.to_le_bytes(), payload].concat();
  // A chunk of odd length is padded to an even one.
  chunk.resize(chunk.len() + payload.len() % 2, 0);
  chunk
}

fn webp(chunks: &[Vec<u8>]) -> Vec<u8> {
  let body = chunks.concat();
  [
    &b"RIFF"[..],
    &u32::try_from(body.len() + 4).unwrap().to_le_bytes(),
    b"WEBP",
    &body,
  ]
  .concat()
}

/// The extended form's first chunk: what the file carries, and the canvas.
fn flags(flags: u8, width: u32, height: u32) -> Vec<u8> {
  let (width, height) = ((width - 1).to_le_bytes(), (height - 1).to_le_bytes());
  chunk(
    b"VP8X",
    &[&[flags, 0, 0, 0][..], &width[..3], &height[..3]].concat(),
  )
}

/// A lossy picture's chunk: a frame tag, the start code, the width and the height.
fn lossy(width: u16, height: u16) -> Vec<u8> {
  chunk(
    b"VP8 ",
    &[
      &[0x30, 0x01, 0x00, 0x9D, 0x01, 0x2A][..],
      &width.to_le_bytes(),
      &height.to_le_bytes(),
      &[0; 6],
    ]
    .concat(),
  )
}

/// A lossless picture's chunk: a signature byte and then the size and the alpha bit.
fn lossless(width: u32, height: u32, alpha: bool) -> Vec<u8> {
  let bits = (width - 1) | ((height - 1) << 14) | (u32::from(alpha) << 28);
  chunk(
    b"VP8L",
    &[&[0x2F][..], &bits.to_le_bytes(), &[0; 5]].concat(),
  )
}

/// One frame of an animation, shown for `milliseconds`.
fn frame(milliseconds: u32, picture: &[u8]) -> Vec<u8> {
  chunk(
    b"ANMF",
    &[
      &[0; 12][..],
      &milliseconds.to_le_bytes()[..3],
      &[0],
      picture,
    ]
    .concat(),
  )
}

fn detail(facts: &ImageFacts) -> &WebpDetail {
  match &facts.detail {
    ImageDetail::Webp(detail) => detail,
    other => panic!("not a WebP's detail: {other:?}"),
  }
}

#[test]
fn the_two_simple_forms_keep_their_size_in_the_picture_itself() {
  let found = specs(&webp(&[lossy(640, 480)]));
  assert_eq!(found.media_type, MediaType::Webp);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (640, 480, 1));
  assert!(!facts.animated);
  assert_eq!(
    (facts.has_alpha, facts.color_model, facts.color_space),
    (Some(false), ColorModel::Rgb, ColorSpace::Srgb)
  );
  assert_eq!(
    (detail(&facts).extended, detail(&facts).lossless),
    (false, Some(false))
  );

  let facts = image_facts(specs(&webp(&[lossless(1024, 512, true)])));
  assert_eq!((facts.width, facts.height), (1024, 512));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(true), ColorModel::Rgba)
  );
  assert_eq!(detail(&facts).lossless, Some(true));
  assert_eq!(
    image_facts(specs(&webp(&[lossless(3, 5, false)]))).has_alpha,
    Some(false)
  );
}

#[test]
fn the_extended_form_keeps_its_size_in_three_bytes_each_whatever_they_are() {
  // 1,024 by 768: a byte of each is zero, which is a digit like any other.
  let facts = image_facts(specs(&webp(&[flags(0, 1024, 768), lossy(1024, 768)])));
  assert_eq!((facts.width, facts.height), (1024, 768));
  assert_eq!(
    (facts.has_alpha, facts.animated, facts.frames),
    (Some(false), false, 1)
  );
  assert_eq!(
    (detail(&facts).extended, detail(&facts).lossless),
    (true, Some(false))
  );
  // The largest a canvas can be.
  let facts = image_facts(specs(&webp(&[flags(HAS_ALPHA, 16_777_216, 1)])));
  assert_eq!(
    (facts.width, facts.height, facts.has_alpha),
    (16_777_216, 1, Some(true))
  );
}

#[test]
fn the_frames_of_an_animation_are_counted_with_how_long_they_last() {
  let file = webp(&[
    flags(IS_ANIMATED | HAS_ALPHA, 400, 300),
    chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]),
    frame(100, &lossless(400, 300, true)),
    frame(250, &lossless(400, 300, true)),
    frame(50, &lossless(400, 300, true)),
  ]);
  let facts = image_facts(specs(&file));
  assert_eq!((facts.frames, facts.animated), (3, true));
  assert_eq!(
    (detail(&facts).loop_count, detail(&facts).duration_ms),
    (Some(0), 400)
  );
  assert_eq!(detail(&facts).lossless, Some(true));
}

#[test]
fn the_profile_the_camera_s_notes_and_the_packet_are_read_from_their_chunks() {
  let profile = icc_profile(Some("Display P3"), true, b"RGB ");
  let file = webp(&[
    flags(HAS_PROFILE | HAS_EXIF | HAS_XMP, 64, 64),
    chunk(b"ICCP", &profile),
    lossy(64, 64),
    // Three bytes of nothing, so that the chunk after it starts on an even offset.
    chunk(b"EXIF", &exif_block(true)),
    chunk(b"XMP ", XMP_PACKET.as_bytes()),
  ]);
  let found = specs(&file);
  assert_eq!(found.warnings.len(), 0);
  let facts = image_facts(found);
  assert_eq!(
    (facts.color_space, facts.icc_profile.as_deref()),
    (ColorSpace::DisplayP3, Some("Display P3"))
  );
  assert_eq!(
    (facts.orientation, facts.camera_model.as_deref()),
    (Some(6), Some("M11"))
  );
  assert_eq!(
    facts.exif_date_time_original.as_deref(),
    Some("2026-11-17T23:36:27-05:00")
  );
  assert_eq!(
    facts.xmp.as_ref().and_then(|xmp| xmp.title.as_deref()),
    Some("A harbor at dusk")
  );

  // A block written the way a JPEG writes it, with six bytes in front.
  let prefixed = [&b"Exif\0\0"[..], &exif_block(false)].concat();
  let file = webp(&[
    flags(HAS_EXIF, 64, 64),
    lossy(64, 64),
    chunk(b"EXIF", &prefixed),
  ]);
  assert_eq!(image_facts(specs(&file)).orientation, Some(6));
}

#[test]
fn a_head_asks_for_the_rest_only_when_the_flags_promise_something_further_on() {
  let picture = chunk(
    b"VP8 ",
    &[
      &[0x30, 0x01, 0x00, 0x9D, 0x01, 0x2A, 64, 0, 64, 0][..],
      &[0; 9000],
    ]
    .concat(),
  );
  let head = |file: &[u8], size: Option<u64>| {
    let pieces = [Piece {
      offset: 0,
      bytes: &file[..4096],
    }];
    specs_source(&Source::from_pieces(&pieces, size).unwrap())
  };
  // Nothing promised: the canvas and the alpha flag are the whole answer.
  let plain = webp(&[flags(HAS_ALPHA, 64, 64), picture.clone()]);
  let found = head(&plain, None);
  assert_eq!(found.needs, None);
  assert_eq!(image_facts(found).width, 64);

  // EXIF promised, after a picture the head does not reach the end of.
  let noted = webp(&[
    flags(HAS_EXIF, 64, 64),
    picture,
    chunk(b"EXIF", &exif_block(true)),
  ]);
  let size = u64::try_from(noted.len()).unwrap();
  let found = head(&noted, Some(size));
  let needs = found.needs.unwrap();
  assert_eq!(needs.offset + needs.length, size);
  assert_eq!(image_facts(found).orientation, None);
  assert_eq!(image_facts(specs(&noted)).orientation, Some(6));
}

#[test]
fn a_first_chunk_that_is_no_picture_is_a_webp_that_says_so() {
  let found = specs(&webp(&[chunk(b"JUNK", &[0; 16])]));
  assert_eq!(
    (found.media_type, &found.facts),
    (MediaType::Webp, &crate::Facts::None)
  );
  assert_eq!(
    found.warnings,
    ["the file's first chunk is not a picture's"]
  );
}

#[test]
fn the_animation_flag_over_a_single_frame_is_a_still_picture() {
  let file = webp(&[
    flags(IS_ANIMATED, 400, 300),
    chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]),
    frame(100, &lossless(400, 300, false)),
  ]);
  let facts = image_facts(specs(&file));
  assert_eq!((facts.frames, facts.animated), (1, false));
}
