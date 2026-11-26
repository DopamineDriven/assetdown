use super::*;
use crate::testing::{image_facts, png_header, png_of};
use crate::{ImageDetail, specs};

/// An icon (`kind` 1) or cursor (`kind` 2) of these pictures. Each is its entry's first eight
/// bytes and the bytes of the picture itself.
fn ico(kind: u8, pictures: &[([u8; 8], Vec<u8>)]) -> Vec<u8> {
  let count = u16::try_from(pictures.len()).unwrap();
  let mut file = [&[0, 0, kind, 0][..], &count.to_le_bytes()].concat();
  let mut picture_at = 6 + 16 * pictures.len();
  for (entry, picture) in pictures {
    file.extend_from_slice(entry);
    file.extend_from_slice(&u32::try_from(picture.len()).unwrap().to_le_bytes());
    file.extend_from_slice(&u32::try_from(picture_at).unwrap().to_le_bytes());
    picture_at += picture.len();
  }
  for (_, picture) in pictures {
    file.extend_from_slice(picture);
  }
  file
}

/// A bitmap picture: a header of 40 bytes and nothing behind it.
fn bitmap() -> Vec<u8> {
  let mut header = 40u32.to_le_bytes().to_vec();
  header.resize(40, 0);
  header
}

fn detail(facts: &ImageFacts) -> &IcoDetail {
  match &facts.detail {
    ImageDetail::Ico(detail) => detail,
    other => panic!("not an icon's detail: {other:?}"),
  }
}

#[test]
fn an_icon_is_as_large_as_its_largest_picture() {
  let file = ico(
    1,
    &[
      ([16, 16, 0, 0, 1, 0, 32, 0], bitmap()),
      // A side of zero is 256.
      ([0, 0, 0, 0, 1, 0, 32, 0], bitmap()),
      ([32, 32, 16, 0, 1, 0, 4, 0], bitmap()),
    ],
  );
  let facts = image_facts(specs(&file));
  assert_eq!((facts.width, facts.height), (256, 256));
  // Three sizes of one picture are a set, not frames: they are in the detail below.
  assert_eq!((facts.frames, facts.animated), (1, false));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(true), ColorModel::Rgba)
  );
  let detail = detail(&facts);
  assert_eq!(detail.kind, IcoKind::Icon);
  let sizes: Vec<(u32, Option<u16>, bool)> = detail
    .entries
    .iter()
    .map(|entry| (entry.width, entry.bits_per_pixel, entry.png))
    .collect();
  assert_eq!(
    sizes,
    [
      (16, Some(32), false),
      (256, Some(32), false),
      (32, Some(4), false)
    ]
  );
}

#[test]
fn a_picture_that_is_a_png_states_its_own_size() {
  let large = png_of(&[png_header(512, 512, 8, 6)]);
  let file = ico(
    1,
    &[
      ([48, 48, 0, 0, 1, 0, 32, 0], bitmap()),
      ([0, 0, 0, 0, 0, 0, 0, 0], large),
    ],
  );
  let facts = image_facts(specs(&file));
  assert_eq!((facts.width, facts.height), (512, 512));
  assert_eq!(facts.color_model, ColorModel::Rgba);
  let png = &detail(&facts).entries[1];
  assert_eq!((png.width, png.png, png.bits_per_pixel), (512, true, None));
}

#[test]
fn a_cursor_says_where_it_points() {
  let file = ico(2, &[([32, 32, 0, 0, 7, 0, 9, 0], bitmap())]);
  let facts = image_facts(specs(&file));
  let detail = detail(&facts);
  assert_eq!(detail.kind, IcoKind::Cursor);
  assert_eq!(
    (detail.entries[0].hotspot, detail.entries[0].bits_per_pixel),
    (Some((7, 9)), None)
  );
  assert_eq!((facts.width, facts.color_model), (32, ColorModel::Unknown));
}
