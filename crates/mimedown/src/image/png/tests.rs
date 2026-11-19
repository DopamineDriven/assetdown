use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use super::*;
use crate::source::Piece;
use crate::testing::{image_facts, png_chunk, png_header, png_of};
use crate::{MediaType, specs, specs_source};

fn picture() -> Vec<u8> {
  png_chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0, 0, 0, 1, 0, 1])
}

fn end() -> Vec<u8> {
  png_chunk(b"IEND", &[])
}

/// A PNG with this header, these chunks in front of the picture, and nothing after it.
fn png_with(header: Vec<u8>, before_picture: &[Vec<u8>]) -> Vec<u8> {
  let mut chunks = vec![header];
  chunks.extend_from_slice(before_picture);
  chunks.extend([picture(), end()]);
  png_of(&chunks)
}

fn detail(facts: &ImageFacts) -> &PngDetail {
  match &facts.detail {
    ImageDetail::Png(detail) => detail,
    other => panic!("not a PNG's detail: {other:?}"),
  }
}

fn deflated(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).unwrap();
  encoder.finish().unwrap()
}

#[test]
fn the_header_gives_the_size_the_depth_and_how_color_is_kept() {
  let cases = [
    (0, ColorModel::Grayscale, ColorSpace::Gray, false),
    (2, ColorModel::Rgb, ColorSpace::Srgb, false),
    (3, ColorModel::Indexed, ColorSpace::Srgb, false),
    (4, ColorModel::GrayscaleAlpha, ColorSpace::Gray, true),
    (6, ColorModel::Rgba, ColorSpace::Srgb, true),
  ];
  for (color_type, color_model, color_space, alpha) in cases {
    let facts = image_facts(specs(&png_with(png_header(640, 480, 8, color_type), &[])));
    assert_eq!((facts.width, facts.height), (640, 480));
    assert_eq!((facts.frames, facts.animated), (1, false));
    assert_eq!(facts.color_model, color_model);
    // A file that says nothing of its color is taken for sRGB, or for gray.
    assert_eq!(facts.color_space, color_space);
    assert_eq!(facts.has_alpha, Some(alpha));
    assert_eq!(facts.orientation, None);
    assert_eq!(
      (detail(&facts).bit_depth, detail(&facts).color_type),
      (8, color_type)
    );
    assert!(!detail(&facts).interlaced);
  }
  let facts = image_facts(specs(&png_with(png_header(4, 3, 16, 2), &[])));
  assert_eq!(facts.aspect_ratio(), Some(4.0 / 3.0));
  assert_eq!(detail(&facts).bit_depth, 16);
}

#[test]
fn a_transparency_chunk_makes_a_picture_without_an_alpha_channel_see_through() {
  for color_type in [2, 3] {
    let file = png_with(
      png_header(8, 8, 8, color_type),
      &[png_chunk(b"tRNS", &[0, 0, 0, 0, 0, 0])],
    );
    assert_eq!(image_facts(specs(&file)).has_alpha, Some(true));
  }
}

#[test]
fn an_animation_control_chunk_makes_it_an_animated_png() {
  // Three frames, played without end.
  let file = png_with(
    png_header(100, 100, 8, 6),
    &[png_chunk(b"acTL", &[0, 0, 0, 3, 0, 0, 0, 0])],
  );
  let found = specs(&file);
  assert_eq!(found.media_type, MediaType::Apng);
  assert_eq!(crate::sniff(&file).media_type(), MediaType::Apng);
  let facts = image_facts(found);
  assert_eq!((facts.frames, facts.animated), (3, true));
  assert_eq!(detail(&facts).loop_count, Some(0));

  // The same chunk after the picture is not an animation's.
  let late = png_of(&[
    png_header(100, 100, 8, 6),
    picture(),
    png_chunk(b"acTL", &[0, 0, 0, 3, 0, 0, 0, 0]),
    end(),
  ]);
  assert_eq!(crate::sniff(&late).media_type(), MediaType::Png);
  assert_eq!(
    specs(&png_with(png_header(1, 1, 8, 6), &[])).media_type,
    MediaType::Png
  );
}

#[test]
fn what_the_file_says_of_its_color_is_believed_the_most_exact_statement_first() {
  let srgb = png_chunk(b"sRGB", &[0]);
  let profile = |name: &str| {
    png_chunk(
      b"iCCP",
      &[name.as_bytes(), &[0, 0], &deflated(b"profile")].concat(),
    )
  };
  let coded = |primaries: u8| png_chunk(b"cICP", &[primaries, 13, 0, 1]);
  let of = |chunks: &[Vec<u8>]| image_facts(specs(&png_with(png_header(8, 8, 8, 2), chunks)));

  assert_eq!(
    of(std::slice::from_ref(&srgb)).color_space,
    ColorSpace::Srgb
  );
  let p3 = of(&[profile("Display P3")]);
  assert_eq!(p3.color_space, ColorSpace::DisplayP3);
  assert_eq!(p3.icc_profile.as_deref(), Some("Display P3"));
  assert_eq!(
    of(&[profile("Adobe RGB (1998)")]).color_space,
    ColorSpace::AdobeRgb
  );
  assert_eq!(
    of(&[profile("ProPhoto RGB")]).color_space,
    ColorSpace::ProphotoRgb
  );
  assert_eq!(
    of(&[profile("sRGB IEC61966-2.1")]).color_space,
    ColorSpace::Srgb
  );
  // A profile whose name says nothing is a profile, and what it stands for is not known.
  let unnamed = of(&[profile("ICC profile")]);
  assert_eq!(unnamed.color_space, ColorSpace::Unknown);
  assert_eq!(unnamed.icc_profile.as_deref(), Some("ICC profile"));
  assert_eq!(of(&[coded(9)]).color_space, ColorSpace::Rec2020);
  assert_eq!(
    of(&[coded(12), profile("sRGB")]).color_space,
    ColorSpace::DisplayP3
  );
  assert_eq!(of(&[]).icc_profile, None);
}

#[test]
fn the_density_is_pixels_to_the_inch_or_only_a_pixels_shape() {
  let density = |across: u32, down: u32, unit: u8| {
    let payload = [&across.to_be_bytes()[..], &down.to_be_bytes(), &[unit]].concat();
    image_facts(specs(&png_with(
      png_header(8, 8, 8, 2),
      &[png_chunk(b"pHYs", &payload)],
    )))
  };
  // 2,835 pixels to the meter is 72 to the inch.
  let (across, down) = detail(&density(2835, 2835, 1)).dpi.unwrap();
  assert!((across - 72.0).abs() < 0.01 && (down - 72.0).abs() < 0.01);
  let shaped = density(2, 1, 0);
  assert_eq!(
    (detail(&shaped).dpi, detail(&shaped).pixel_aspect_ratio),
    (None, Some(2.0))
  );
  let facts = image_facts(specs(&png_with(
    png_header(8, 8, 8, 2),
    &[png_chunk(b"gAMA", &45455u32.to_be_bytes())],
  )));
  assert_eq!(detail(&facts).gamma, Some(0.45455));
}

#[test]
fn text_is_read_however_it_is_kept() {
  let chunks = [
    png_chunk(b"tEXt", b"Software\0made by hand"),
    png_chunk(
      b"zTXt",
      &[&b"Comment\0\0"[..], &deflated(b"kept small")].concat(),
    ),
    png_chunk(b"iTXt", "Title\0\0\0en\0Titel\0Gr\u{fc}\u{df}e".as_bytes()),
    png_chunk(
      b"iTXt",
      &[
        &b"Description\0\x01\0\0\0"[..],
        &deflated("ein Gru\u{df}".as_bytes()),
      ]
      .concat(),
    ),
  ];
  let facts = image_facts(specs(&png_with(png_header(8, 8, 8, 2), &chunks)));
  let text: Vec<(PngTextKind, &str, &str)> = detail(&facts)
    .text
    .iter()
    .map(|text| (text.kind, text.keyword.as_str(), text.text.as_str()))
    .collect();
  assert_eq!(
    text,
    [
      (PngTextKind::Plain, "Software", "made by hand"),
      (PngTextKind::Compressed, "Comment", "kept small"),
      (PngTextKind::International, "Title", "Gr\u{fc}\u{df}e"),
      (PngTextKind::International, "Description", "ein Gru\u{df}"),
    ]
  );
}

#[test]
fn the_time_of_the_last_change_is_read_as_the_utc_it_is_kept_in() {
  let time = png_chunk(b"tIME", &[0x07, 0xEA, 11, 17, 23, 36, 27]);
  let facts = image_facts(specs(&png_with(png_header(8, 8, 8, 2), &[time])));
  assert_eq!(
    detail(&facts).modification_time.as_deref(),
    Some("2026-11-17T23:36:27Z")
  );
}

#[test]
fn chunks_after_the_picture_are_read_when_they_are_there() {
  let file = png_of(&[
    png_header(8, 8, 8, 2),
    picture(),
    png_chunk(b"tEXt", b"Comment\0written last"),
    end(),
  ]);
  let facts = image_facts(specs(&file));
  assert_eq!(detail(&facts).text[0].text, "written last");
}

#[test]
fn a_head_that_stops_before_the_picture_asks_for_more_and_one_that_reaches_it_does_not() {
  let text = png_chunk(b"tEXt", &[&b"Comment\0"[..], &[b'x'; 500]].concat());
  let file = png_of(&[
    png_header(8, 8, 8, 2),
    text,
    picture(),
    png_chunk(b"tEXt", b"After\0the picture"),
    end(),
  ]);
  let head = |length: usize| {
    let pieces = [Piece {
      offset: 0,
      bytes: &file[..length],
    }];
    specs_source(&Source::from_pieces(&pieces, None).unwrap())
  };
  // The header is there and the text chunk after it is cut off. No less is asked for than the
  // rest of that chunk and the eight bytes after it, which say whether the picture comes next
  // or an animation's chunk does; and it is asked for by the block.
  let short = head(100);
  let needs = short.needs.unwrap();
  assert_eq!(needs.offset, 100);
  assert!(needs.length >= 461, "{needs:?}");
  assert_eq!(image_facts(short).width, 8);
  // The picture has begun: what follows it may or may not be there, and is not asked for.
  let enough = head(570);
  assert_eq!((enough.needs, enough.warnings.len()), (None, 0));
  assert_eq!(detail(&image_facts(enough)).text.len(), 1);
}

#[test]
fn a_damaged_file_is_still_a_png_and_says_what_is_wrong() {
  let headless = specs(&png_of(&[picture(), end()]));
  assert_eq!(
    (headless.media_type, &headless.facts),
    (MediaType::Png, &crate::Facts::None)
  );
  assert_eq!(headless.warnings, ["the file has no header chunk"]);

  let file = png_with(
    png_header(8, 8, 8, 2),
    &[png_chunk(b"tEXt", b"Comment\0cut off here")],
  );
  let cut = specs(&file[..50]);
  assert_eq!(cut.warnings, ["the file ends inside its tEXt chunk"]);
  assert_eq!(image_facts(cut).width, 8);
}

#[test]
fn a_chunk_that_inflates_past_what_is_read_is_left_out() {
  let bomb = deflated(&vec![b'a'; 5 << 20]);
  let chunks = [
    png_chunk(b"zTXt", &[&b"Comment\0\0"[..], &bomb].concat()),
    png_chunk(b"tEXt", b"Software\0still read"),
  ];
  let found = specs(&png_with(png_header(8, 8, 8, 2), &chunks));
  assert_eq!(
    found.warnings,
    ["the text under \"Comment\" could not be read and was left out"]
  );
  assert_eq!(detail(&image_facts(found)).text.len(), 1);
}

#[test]
fn a_gray_picture_is_gray_whatever_curve_its_shades_follow() {
  let facts = image_facts(specs(&png_with(
    png_header(8, 8, 8, 0),
    &[png_chunk(b"sRGB", &[0])],
  )));
  assert_eq!(facts.color_space, ColorSpace::Gray);
}

#[test]
fn the_profile_the_camera_s_notes_and_the_packet_are_read_from_their_chunks() {
  use crate::testing::{XMP_PACKET, exif_block, icc_profile};
  let profile = icc_profile(Some("Display P3"), true, b"RGB ");
  let chunks = [
    // The file's own name for the profile says nothing; the profile's does.
    png_chunk(
      b"iCCP",
      &[&b"ICC Profile\0\0"[..], &deflated(&profile)].concat(),
    ),
    png_chunk(b"eXIf", &exif_block(true)),
    png_chunk(
      b"iTXt",
      &[&b"XML:com.adobe.xmp\0\0\0\0\0"[..], XMP_PACKET.as_bytes()].concat(),
    ),
    png_chunk(b"tEXt", b"Software\0made by hand"),
  ];
  let found = specs(&png_with(png_header(8, 8, 8, 6), &chunks));
  assert_eq!(found.warnings.len(), 0);
  let facts = image_facts(found);
  assert_eq!(
    (facts.color_space, facts.icc_profile.as_deref()),
    (ColorSpace::DisplayP3, Some("Display P3"))
  );
  assert_eq!(
    facts.icc.as_ref().map(|profile| profile.class.as_str()),
    Some("mntr")
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
    facts
      .xmp
      .as_ref()
      .and_then(|xmp| xmp.icc_profile.as_deref()),
    Some("Display P3")
  );
  // The packet is read for what it says and is not kept among the text.
  let keywords: Vec<&str> = detail(&facts)
    .text
    .iter()
    .map(|text| text.keyword.as_str())
    .collect();
  assert_eq!(keywords, ["Software"]);
}

#[test]
fn a_profile_with_no_name_of_its_own_goes_by_the_name_the_file_gives_it() {
  use crate::testing::icc_profile;
  let profile = icc_profile(None, false, b"GRAY");
  let chunk = png_chunk(
    b"iCCP",
    &[&b"Dot Gain 20%\0\0"[..], &deflated(&profile)].concat(),
  );
  let facts = image_facts(specs(&png_with(png_header(8, 8, 8, 0), &[chunk])));
  assert_eq!(
    (facts.icc_profile.as_deref(), facts.color_space),
    (Some("Dot Gain 20%"), ColorSpace::Gray)
  );
}

#[test]
fn a_camera_s_notes_kept_as_text_in_hexadecimal_are_read_too() {
  use crate::testing::exif_block;
  // How ImageMagick has long kept EXIF in a PNG: a name, a length, and the bytes in hex.
  let block = [&b"Exif\0\0"[..], &exif_block(true)].concat();
  let hex: String = block.iter().map(|byte| format!("{byte:02x}")).collect();
  let text = format!("\nexif\n    {}\n{hex}\n", block.len());
  let chunk = png_chunk(
    b"zTXt",
    &[
      &b"Raw profile type exif\0\0"[..],
      &deflated(text.as_bytes()),
    ]
    .concat(),
  );
  let facts = image_facts(specs(&png_with(png_header(8, 8, 8, 2), &[chunk])));
  assert_eq!(
    (facts.orientation, facts.camera_model.as_deref()),
    (Some(6), Some("M11"))
  );
  // It is read for what it says and is not kept among the text.
  assert_eq!(detail(&facts).text.len(), 0);
}
