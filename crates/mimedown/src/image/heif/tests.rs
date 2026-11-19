use super::*;
use crate::source::Piece;
use crate::testing::{
  HeifItem, XMP_PACKET, boxed, exif_block, heif, heif_alpha, heif_coded_color, heif_depths,
  heif_profile, heif_sequence, heif_size, icc_profile, image_facts,
};
use crate::{MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &HeifDetail {
  match &facts.detail {
    ImageDetail::Heif(detail) => detail,
    other => panic!("not this family's detail: {other:?}"),
  }
}

/// An AVIF of one picture with these properties beside its size.
fn avif(properties: Vec<Vec<u8>>) -> Vec<u8> {
  let all = [vec![heif_size(1920, 1080)], properties].concat();
  heif(
    b"avif",
    &[b"avif", b"mif1"],
    1,
    &[HeifItem::picture(1, b"av01", all)],
    &[],
  )
}

fn turn(angle: u8) -> Vec<u8> {
  boxed(b"irot", &[angle])
}

fn mirror(axis: u8) -> Vec<u8> {
  boxed(b"imir", &[axis])
}

#[test]
fn the_picture_s_own_properties_give_its_size_its_depth_and_its_color() {
  let file = avif(vec![heif_depths(&[10, 10, 10]), heif_coded_color(1, 13, 6)]);
  let found = specs(&file);
  assert_eq!(
    (found.media_type, found.needs, found.warnings.len()),
    (MediaType::Avif, None, 0)
  );
  let facts = image_facts(found);
  assert_eq!(
    (facts.width, facts.height, facts.frames, facts.animated),
    (1920, 1080, 1, false)
  );
  assert_eq!(
    (facts.has_alpha, facts.color_model, facts.color_space),
    (Some(false), ColorModel::Ycbcr, ColorSpace::Srgb)
  );
  assert_eq!(facts.orientation, None);
  let detail = detail(&facts);
  assert_eq!(
    (detail.codec.as_deref(), detail.bit_depth, detail.tiled),
    (Some("av01"), Some(10), false)
  );
}

#[test]
fn a_picture_stored_as_a_grid_is_as_large_as_the_grid_and_not_as_a_tile() {
  // The tiles' size stands first among the properties, as a phone writes it.
  let tile = vec![heif_size(512, 512), heif_depths(&[8, 8, 8])];
  let items = [
    HeifItem::picture(1, b"hvc1", tile.clone()),
    HeifItem::picture(2, b"hvc1", tile),
    HeifItem::picture(
      3,
      b"grid",
      vec![heif_size(4032, 3024), heif_depths(&[8, 8, 8])],
    ),
  ];
  let file = heif(
    b"heic",
    &[b"mif1", b"heic"],
    3,
    &items,
    &[(b"dimg", 3, &[1, 2])],
  );
  let found = specs(&file);
  assert_eq!(found.media_type, MediaType::Heic);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (4032, 3024));
  assert_eq!(
    (detail(&facts).tiled, detail(&facts).codec.as_deref()),
    (true, Some("hvc1"))
  );
}

#[test]
fn how_the_picture_is_turned_is_what_its_properties_say() {
  let of = |properties: Vec<Vec<u8>>| image_facts(specs(&avif(properties))).orientation;
  assert_eq!(of(vec![]), None);
  // A quarter turn against the clock is what EXIF numbers 8; three of them is 6.
  assert_eq!(of(vec![turn(1)]), Some(8));
  assert_eq!(of(vec![turn(2)]), Some(3));
  assert_eq!(of(vec![turn(3)]), Some(6));
  assert_eq!(of(vec![turn(0)]), Some(1));
  // Top for bottom, and left for right.
  assert_eq!(of(vec![mirror(0)]), Some(4));
  assert_eq!(of(vec![mirror(1)]), Some(2));
  // Both, in the order they are listed.
  assert_eq!(of(vec![mirror(1), turn(1)]), Some(5));
  assert_eq!(of(vec![mirror(1), turn(3)]), Some(7));
}

#[test]
fn an_item_that_is_the_picture_s_alpha_makes_it_see_through() {
  let items = [
    HeifItem::picture(1, b"av01", vec![heif_size(64, 64)]),
    HeifItem::picture(2, b"av01", vec![heif_size(64, 64), heif_alpha()]),
  ];
  let with_alpha = heif(b"avif", &[b"avif"], 1, &items, &[(b"auxl", 2, &[1])]);
  let facts = image_facts(specs(&with_alpha));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(true), ColorModel::Rgba)
  );
  // The same item, pointing at nothing, is no one's alpha.
  let facts = image_facts(specs(&heif(b"avif", &[b"avif"], 1, &items, &[])));
  assert_eq!(facts.has_alpha, Some(false));
}

#[test]
fn the_color_is_named_by_a_profile_or_by_coded_values_and_is_srgb_when_unsaid() {
  let of = |properties: Vec<Vec<u8>>| image_facts(specs(&avif(properties)));
  let profiled = of(vec![heif_profile(&icc_profile(
    Some("Display P3"),
    true,
    b"RGB ",
  ))]);
  assert_eq!(
    (profiled.color_space, profiled.icc_profile.as_deref()),
    (ColorSpace::DisplayP3, Some("Display P3"))
  );
  assert_eq!(
    of(vec![heif_coded_color(9, 16, 9)]).color_space,
    ColorSpace::Rec2020
  );
  assert_eq!(
    of(vec![heif_coded_color(12, 13, 6)]).color_space,
    ColorSpace::DisplayP3
  );
  assert_eq!(
    of(vec![heif_coded_color(1, 1, 1)]).color_space,
    ColorSpace::Rec709
  );
  assert_eq!(of(vec![]).color_space, ColorSpace::Srgb);
  // A matrix of zero keeps red, green and blue as they are; one channel is gray.
  assert_eq!(
    of(vec![heif_coded_color(1, 13, 0)]).color_model,
    ColorModel::Rgb
  );
  let gray = of(vec![heif_depths(&[8])]);
  assert_eq!(
    (gray.color_model, gray.color_space),
    (ColorModel::Grayscale, ColorSpace::Gray)
  );
}

#[test]
fn a_camera_s_notes_and_a_packet_are_items_of_their_own() {
  let notes = HeifItem {
    id: 2,
    kind: *b"Exif",
    properties: Vec::new(),
    content_type: "",
    // Four bytes that say the block starts at once, and the block.
    data: [&[0, 0, 0, 0][..], &exif_block(false)].concat(),
  };
  let packet = HeifItem {
    id: 3,
    kind: *b"mime",
    properties: Vec::new(),
    content_type: "application/rdf+xml",
    data: XMP_PACKET.as_bytes().to_vec(),
  };
  let picture = HeifItem::picture(1, b"hvc1", vec![heif_size(4032, 3024), turn(3)]);
  let file = heif(
    b"heic",
    &[b"mif1", b"heic"],
    1,
    &[picture, notes, packet],
    &[(b"cdsc", 2, &[1])],
  );
  let found = specs(&file);
  assert_eq!(found.warnings.len(), 0);
  let facts = image_facts(found);
  assert_eq!(
    (facts.camera_make.as_deref(), facts.camera_model.as_deref()),
    (Some("Leica Camera AG"), Some("M11"))
  );
  assert_eq!(
    facts.exif_date_time_original.as_deref(),
    Some("2026-11-17T23:36:27-05:00")
  );
  assert!(facts.gps_lat.is_some());
  assert_eq!(
    facts.xmp.as_ref().and_then(|xmp| xmp.title.as_deref()),
    Some("A harbor at dusk")
  );
  // The turn is the property's, which here agrees with the notes'.
  assert_eq!(facts.orientation, Some(6));

  // With no property to turn it, the picture is not turned, whatever the notes say.
  let untouched = HeifItem::picture(1, b"hvc1", vec![heif_size(4032, 3024)]);
  let notes = HeifItem {
    id: 2,
    kind: *b"Exif",
    properties: Vec::new(),
    content_type: "",
    data: [&[0, 0, 0, 0][..], &exif_block(true)].concat(),
  };
  let facts = image_facts(specs(&heif(
    b"heic",
    &[b"heic"],
    1,
    &[untouched, notes],
    &[],
  )));
  assert_eq!(
    (facts.orientation, facts.camera_model.as_deref()),
    (None, Some("M11"))
  );
}

#[test]
fn a_sequence_counts_the_pictures_of_its_track() {
  let still = heif(
    b"avis",
    &[b"avis", b"msf1"],
    1,
    &[HeifItem::picture(1, b"av01", vec![heif_size(320, 240)])],
    &[],
  );
  let file = [still, heif_sequence(24)].concat();
  let found = specs(&file);
  assert_eq!(found.media_type, MediaType::Avif);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.frames, facts.animated), (320, 24, true));
}

#[test]
fn a_head_that_holds_the_list_of_items_is_enough_for_a_still_picture() {
  let mut picture = HeifItem::picture(1, b"av01", vec![heif_size(1920, 1080)]);
  picture.data = vec![0; 50_000];
  let file = heif(b"avif", &[b"avif"], 1, &[picture], &[]);
  let head = |length: usize| {
    let pieces = [Piece {
      offset: 0,
      bytes: &file[..length],
    }];
    specs_source(&Source::from_pieces(&pieces, None).unwrap())
  };
  let enough = head(1024);
  assert_eq!(enough.needs, None);
  assert_eq!(image_facts(enough).width, 1920);
  // A head that stops inside the list asks for the rest of it.
  let short = head(60);
  assert_eq!(short.facts, crate::Facts::None);
  assert_eq!(short.needs.map(|range| range.offset), Some(60));
}

#[test]
fn a_file_with_no_list_of_items_says_so() {
  let file = [
    crate::testing::ftyp(b"heic", &[b"heic"]),
    boxed(b"mdat", &[0; 16]),
  ]
  .concat();
  let found = specs(&file);
  assert_eq!(
    (found.media_type, &found.facts),
    (MediaType::Heic, &crate::Facts::None)
  );
  assert_eq!(found.warnings, ["the file has no list of items"]);
}

#[test]
fn a_grid_that_says_nothing_of_its_color_has_the_color_of_its_tiles() {
  // How a phone writes it: the profile and the depth are the tiles', and the grid has a size.
  let profile = heif_profile(&icc_profile(Some("Display P3"), true, b"RGB "));
  let tile = vec![heif_size(512, 512), profile, heif_depths(&[10, 10, 10])];
  let items = [
    HeifItem::picture(1, b"hvc1", tile.clone()),
    HeifItem::picture(2, b"hvc1", tile),
    HeifItem::picture(3, b"grid", vec![heif_size(4032, 3024)]),
  ];
  let file = heif(
    b"heic",
    &[b"mif1", b"heic"],
    3,
    &items,
    &[(b"dimg", 3, &[1, 2])],
  );
  let facts = image_facts(specs(&file));
  assert_eq!((facts.width, facts.height), (4032, 3024));
  assert_eq!(
    (facts.color_space, facts.icc_profile.as_deref()),
    (ColorSpace::DisplayP3, Some("Display P3"))
  );
  assert_eq!(detail(&facts).bit_depth, Some(10));
}
