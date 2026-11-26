use super::*;
use crate::source::Piece;
use crate::testing::{TiffValue, XMP_PACKET, big_tiff, icc_profile, image_facts, tiff, tiff_at};
use crate::{MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &TiffDetail {
  match &facts.detail {
    ImageDetail::Tiff(detail) => detail,
    other => panic!("not a TIFF's detail: {other:?}"),
  }
}

/// A page of this size, with what its samples mean and how many there are.
fn page(width: u32, height: u32, photometric: u16, samples: u16) -> Vec<(u16, TiffValue)> {
  vec![
    (256, TiffValue::Long(vec![width])),
    (257, TiffValue::Long(vec![height])),
    (258, TiffValue::Short(vec![8; usize::from(samples)])),
    (259, TiffValue::Short(vec![5])),
    (262, TiffValue::Short(vec![photometric])),
    (277, TiffValue::Short(vec![samples])),
  ]
}

#[test]
fn the_first_directory_gives_the_size_and_what_the_samples_mean() {
  for little in [true, false] {
    let found = specs(&tiff(little, &[page(1200, 900, 2, 3)]));
    assert_eq!(found.media_type, MediaType::Tiff);
    let facts = image_facts(found);
    assert_eq!((facts.width, facts.height, facts.frames), (1200, 900, 1));
    assert_eq!(
      (facts.color_model, facts.color_space),
      (ColorModel::Rgb, ColorSpace::Srgb)
    );
    assert_eq!(facts.has_alpha, Some(false));
    let detail = detail(&facts);
    assert_eq!(
      (detail.big, detail.little_endian, detail.pages),
      (false, little, 1)
    );
    assert_eq!((detail.compression, detail.photometric), (Some(5), Some(2)));
    assert_eq!(
      (detail.bits_per_sample.as_slice(), detail.samples_per_pixel),
      (&[8, 8, 8][..], 3)
    );
  }
}

#[test]
fn a_value_of_zero_is_a_value() {
  // White is zero: the number for it is 0, which is as much a value as any other.
  let facts = image_facts(specs(&tiff(true, &[page(100, 100, 0, 1)])));
  assert_eq!(
    (facts.color_model, facts.color_space),
    (ColorModel::Grayscale, ColorSpace::Gray)
  );
  assert_eq!(detail(&facts).photometric, Some(0));
}

#[test]
fn what_the_samples_mean_decides_how_color_is_kept() {
  let of = |photometric: u16, samples: u16| {
    image_facts(specs(&tiff(true, &[page(8, 8, photometric, samples)])))
  };
  assert_eq!(of(1, 1).color_model, ColorModel::Grayscale);
  assert_eq!(of(3, 1).color_model, ColorModel::Indexed);
  let cmyk = of(5, 4);
  assert_eq!(
    (cmyk.color_model, cmyk.color_space),
    (ColorModel::Cmyk, ColorSpace::Cmyk)
  );
  assert_eq!(of(6, 3).color_model, ColorModel::Ycbcr);
  let lab = of(8, 3);
  assert_eq!(
    (lab.color_model, lab.color_space),
    (ColorModel::Lab, ColorSpace::Lab)
  );
}

#[test]
fn a_sample_beyond_the_colors_own_is_alpha_when_the_file_says_it_is() {
  let mut with_alpha = page(8, 8, 2, 4);
  with_alpha.push((338, TiffValue::Short(vec![2])));
  let facts = image_facts(specs(&tiff(true, &[with_alpha])));
  assert_eq!(
    (facts.has_alpha, facts.color_model),
    (Some(true), ColorModel::Rgba)
  );
  // A fourth sample the file calls unspecified is not alpha.
  let mut unspecified = page(8, 8, 2, 4);
  unspecified.push((338, TiffValue::Short(vec![0])));
  assert_eq!(
    image_facts(specs(&tiff(true, &[unspecified]))).has_alpha,
    Some(false)
  );
}

#[test]
fn the_pages_are_counted_by_following_each_directory_to_the_next() {
  let file = tiff(
    false,
    &[page(8, 8, 2, 3), page(16, 16, 2, 3), page(4, 4, 1, 1)],
  );
  let facts = image_facts(specs(&file));
  // The size is the first page's.
  // Pages are not frames: nothing here moves.
  assert_eq!((facts.width, facts.frames, facts.animated), (8, 1, false));
  assert_eq!(detail(&facts).pages, 3);
}

#[test]
fn a_bigtiff_is_read_by_the_same_reader() {
  for little in [true, false] {
    let file = big_tiff(
      little,
      &[(256, 4, 70_000), (257, 4, 50_000), (262, 3, 1), (277, 3, 1)],
    );
    let facts = image_facts(specs(&file));
    assert_eq!((facts.width, facts.height), (70_000, 50_000));
    assert_eq!(facts.color_model, ColorModel::Grayscale);
    assert!(detail(&facts).big);
  }
}

#[test]
fn the_density_the_software_the_date_the_profile_and_the_packet_are_read() {
  let mut entries = page(8, 8, 2, 3);
  entries.extend([
    (282, TiffValue::Fractions(vec![(118, 1)])),
    (283, TiffValue::Fractions(vec![(118, 1)])),
    (296, TiffValue::Short(vec![3])),
    (305, TiffValue::Text("made by hand")),
    (306, TiffValue::Text("2026:11:17 23:36:27")),
    (700, TiffValue::Bytes(XMP_PACKET.as_bytes().to_vec())),
    (
      34675,
      TiffValue::Bytes(icc_profile(Some("Adobe RGB (1998)"), false, b"RGB ")),
    ),
  ]);
  let facts = image_facts(specs(&tiff(true, &[entries])));
  // 118 pixels to the centimeter.
  let (across, _) = detail(&facts).dpi.unwrap();
  assert!((across - 299.72).abs() < 0.01);
  assert_eq!(detail(&facts).software.as_deref(), Some("made by hand"));
  assert_eq!(
    detail(&facts).date_time.as_deref(),
    Some("2026-11-17T23:36:27")
  );
  assert_eq!(
    (facts.color_space, facts.icc_profile.as_deref()),
    (ColorSpace::AdobeRgb, Some("Adobe RGB (1998)"))
  );
  assert_eq!(
    facts
      .xmp
      .as_ref()
      .and_then(|xmp| xmp.creator_tool.as_deref()),
    Some("Adobe Photoshop 26.0 (Windows)")
  );
}

#[test]
fn a_camera_s_notes_are_read_from_the_file_s_own_directories() {
  let mut entries = page(6000, 4000, 2, 3);
  entries.extend([
    (0x010F, TiffValue::Text("Hasselblad")),
    (0x0112, TiffValue::Short(vec![3])),
    (
      0x8769,
      TiffValue::Directory(vec![(0x9003, TiffValue::Text("2026:11:17 23:36:27"))]),
    ),
  ]);
  let facts = image_facts(specs(&tiff(true, &[entries])));
  assert_eq!(
    (facts.orientation, facts.camera_make.as_deref()),
    (Some(3), Some("Hasselblad"))
  );
  assert_eq!(
    facts.exif_date_time_original.as_deref(),
    Some("2026-11-17T23:36:27")
  );
}

#[test]
fn a_directory_at_the_end_of_the_file_is_asked_for_by_where_the_header_says_it_is() {
  let file = tiff_at(true, 50_000, &[page(640, 480, 2, 3)]);
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..4096],
  }];
  let head = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  assert_eq!(
    (head.media_type, &head.facts),
    (MediaType::Tiff, &crate::Facts::None)
  );
  let needs = head.needs.unwrap();
  assert_eq!(needs.offset, 50_000);

  let end = usize::try_from(needs.offset).unwrap();
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..4096],
    },
    Piece {
      offset: needs.offset,
      bytes: &file[end..],
    },
  ];
  let both = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  assert_eq!(both.needs, None);
  assert_eq!(image_facts(both).width, 640);
}

#[test]
fn a_file_cut_off_before_its_directory_is_a_tiff_that_says_so() {
  let file = tiff_at(true, 50_000, &[page(640, 480, 2, 3)]);
  let cut = specs(&file[..4096]);
  assert_eq!(
    (cut.media_type, &cut.facts),
    (MediaType::Tiff, &crate::Facts::None)
  );
  assert_eq!(cut.warnings, ["the file ends before its first directory"]);
}

/// One directory of a camera's file: whether it is a smaller copy, its size, and what its
/// samples mean.
fn copy(reduced: bool, width: u32, height: u32, photometric: u16) -> Vec<(u16, TiffValue)> {
  vec![
    (254, TiffValue::Long(vec![u32::from(reduced)])),
    (256, TiffValue::Long(vec![width])),
    (257, TiffValue::Long(vec![height])),
    (262, TiffValue::Short(vec![photometric])),
  ]
}

/// A raw file as most cameras lay it out: a small picture first, with who made it and how it
/// is to be turned, and what the sensor recorded in a directory of its own.
fn camera_file(make: &'static str) -> Vec<u8> {
  let mut first = copy(true, 160, 120, 2);
  first.push((271, TiffValue::Text(make)));
  first.push((274, TiffValue::Short(vec![6])));
  first.push((
    330,
    TiffValue::Directories(vec![
      copy(true, 1620, 1080, 6),
      copy(false, 6032, 4032, 32803),
    ]),
  ));
  tiff(true, &[first])
}

#[test]
fn a_camera_s_file_gives_the_size_of_what_its_sensor_recorded() {
  let found = specs(&camera_file("NIKON CORPORATION"));
  assert_eq!(found.media_type, MediaType::Nef);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (6032, 4032, 1));
  // Who made it and how it is to be turned are said once, in the first directory.
  assert_eq!(facts.camera_make.as_deref(), Some("NIKON CORPORATION"));
  assert_eq!(facts.orientation, Some(6));
  assert_eq!(detail(&facts).photometric, Some(32803));
  assert_eq!(facts.color_model, ColorModel::Unknown);
}

#[test]
fn the_other_directories_of_a_camera_s_file_are_copies_and_not_pages() {
  let mut first = copy(false, 5184, 3456, 6);
  first.push((0xC612, TiffValue::Bytes(vec![1, 4, 0, 0])));
  let found = specs(&tiff(true, &[first, copy(true, 160, 120, 6)]));
  assert_eq!(found.media_type, MediaType::Dng);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (5184, 3456, 1));
  assert_eq!(detail(&facts).pages, 2);
}

#[test]
fn a_tiff_that_keeps_smaller_copies_of_itself_has_the_size_of_its_first_directory() {
  let mut first = page(4000, 3000, 2, 3);
  first.push((
    330,
    TiffValue::Directories(vec![copy(true, 2000, 1500, 2), copy(true, 1000, 750, 2)]),
  ));
  let found = specs(&tiff(false, &[first]));
  assert_eq!(found.media_type, MediaType::Tiff);
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height, facts.frames), (4000, 3000, 1));
}

/// Where a directory of six entries that starts at offset 8 says the next one is.
const NEXT_OF_THE_FIRST: usize = 8 + 2 + 6 * 12;

#[test]
fn directories_that_lead_back_to_one_already_counted_are_counted_once() {
  // The only directory names itself as the next.
  let mut file = tiff(true, &[page(640, 480, 2, 3)]);
  file[NEXT_OF_THE_FIRST..NEXT_OF_THE_FIRST + 4].copy_from_slice(&8u32.to_le_bytes());
  let found = specs(&file);
  assert_eq!(
    found.warnings,
    ["its directories lead back to one already counted, where the count of its pages stops"]
  );
  let facts = image_facts(found);
  assert_eq!((facts.frames, detail(&facts).pages), (1, 1));

  // Two pages, the second of which names the first as the next.
  let mut file = tiff(true, &[page(640, 480, 2, 3), page(640, 480, 2, 3)]);
  let second = usize::try_from(u32::from_le_bytes(
    *file[NEXT_OF_THE_FIRST..].first_chunk().unwrap(),
  ))
  .unwrap();
  let its_next = second + 2 + 6 * 12;
  file[its_next..its_next + 4].copy_from_slice(&8u32.to_le_bytes());
  let found = specs(&file);
  assert_eq!(found.warnings.len(), 1);
  let facts = image_facts(found);
  assert_eq!((facts.frames, detail(&facts).pages), (1, 2));
}

#[test]
fn a_value_that_lies_past_the_end_of_the_file_is_left_out_and_the_rest_is_kept() {
  // The name of the program that wrote the file is the last thing in it, and the file was cut.
  let mut entries = page(640, 480, 2, 3);
  entries.push((305, TiffValue::Text("A program with a name of some length")));
  let file = tiff(true, &[entries]);
  let found = specs(&file[..file.len() - 8]);
  assert_eq!(
    found.warnings,
    ["a value of its first directory lies past the end of the file and is left out"]
  );
  let facts = image_facts(found);
  assert_eq!((facts.width, facts.height), (640, 480));
  assert_eq!(detail(&facts).software, None);
  assert_eq!(detail(&facts).compression, Some(5));
}
