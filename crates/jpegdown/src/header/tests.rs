use super::*;
use crate::testing::{Exif, exif, file, frame, jfif, segment};

#[test]
fn a_frame_states_the_size_the_depth_and_how_many_components_a_pixel_has() {
  let header = read(&file(&[frame(0xC0, 8, 1700, 2200, 3)])).unwrap();
  assert_eq!((header.width, header.height), (1700, 2200));
  assert_eq!((header.precision, header.components), (8, 3));
  assert_eq!(header.coding, Coding::Baseline);
  assert_eq!(header.resolution, None);
  assert_eq!(header.orientation, 1);
  assert_eq!((header.software, header.date_time), (None, None));
}

#[test]
fn the_coding_is_named_from_the_marker_of_the_frame() {
  let named = |marker: u8| read(&file(&[frame(marker, 8, 4, 4, 1)])).unwrap().coding;
  assert_eq!(named(0xC0), Coding::Baseline);
  assert_eq!(named(0xC1), Coding::Extended);
  assert_eq!(named(0xC2), Coding::Progressive);
  assert_eq!(named(0xC3), Coding::Lossless);
  assert_eq!(named(0xC9), Coding::Other(0xC9));
}

#[test]
fn a_density_is_a_resolution_when_it_is_in_a_real_unit() {
  let with = |unit, across, down| {
    read(&file(&[jfif(unit, across, down), frame(0xC0, 8, 4, 4, 1)]))
      .unwrap()
      .resolution
  };
  assert_eq!(with(1, 200, 100), Some(Resolution { x: 200.0, y: 100.0 }));
  // 118 dots to the centimeter is 299.72 to the inch.
  let metric = with(2, 118, 118).unwrap();
  assert!((metric.x - 299.72).abs() < 1e-9 && metric.x == metric.y);
  // With no unit the two numbers are the shape of a pixel and say nothing of size.
  assert_eq!(with(0, 1, 1), None);
  assert_eq!(with(1, 0, 300), None);
}

#[test]
fn exif_gives_the_orientation_the_resolution_the_program_and_the_date_in_either_byte_order() {
  for little in [true, false] {
    let said = exif(
      little,
      &[
        (0x0112, Exif::Short(6)),
        (0x011A, Exif::Fraction(3000, 10)),
        (0x011B, Exif::Fraction(300, 1)),
        (0x0128, Exif::Short(2)),
        (0x0131, Exif::Text("a scanner")),
        (0x0132, Exif::Text("2015:03:04 10:22:33")),
      ],
    );
    let header = read(&file(&[said, frame(0xC2, 8, 2200, 1700, 3)])).unwrap();
    assert_eq!(header.orientation, 6);
    assert_eq!(header.resolution, Some(Resolution { x: 300.0, y: 300.0 }));
    assert_eq!(header.software.as_deref(), Some("a scanner"));
    // As the file records it: the format has no place for a time zone.
    assert_eq!(header.date_time.as_deref(), Some("2015-03-04T10:22:33"));
  }
}

#[test]
fn the_jfif_density_is_taken_before_exifs_and_exifs_when_jfif_has_no_unit() {
  let said = |unit| {
    let exif = exif(
      true,
      &[
        (0x011A, Exif::Fraction(72, 1)),
        (0x011B, Exif::Fraction(72, 1)),
        (0x0128, Exif::Short(2)),
      ],
    );
    read(&file(&[
      jfif(unit, 300, 300),
      exif,
      frame(0xC0, 8, 4, 4, 1),
    ]))
    .unwrap()
    .resolution
  };
  assert_eq!(said(1), Some(Resolution { x: 300.0, y: 300.0 }));
  assert_eq!(said(0), Some(Resolution { x: 72.0, y: 72.0 }));
}

#[test]
fn an_orientation_that_is_not_one_of_the_eight_is_taken_as_upright() {
  let header = read(&file(&[
    exif(true, &[(0x0112, Exif::Short(9))]),
    frame(0xC0, 8, 4, 4, 1),
  ]))
  .unwrap();
  assert_eq!(header.orientation, 1);
}

#[test]
fn bytes_that_are_not_a_jpeg_are_refused_by_name() {
  assert_eq!(
    read(b"%PDF-1.7\n"),
    Err(JpegError::UnsupportedFormat(
      "the file does not open as a JPEG does".to_string()
    ))
  );
  assert!(matches!(read(&[]), Err(JpegError::UnsupportedFormat(_))));
}

#[test]
fn a_file_with_no_frame_before_its_picture_is_damaged() {
  assert_eq!(
    read(&file(&[jfif(1, 300, 300)])),
    Err(JpegError::Malformed(
      "the file's picture begins before anything states its size".to_string()
    ))
  );
}

#[test]
fn a_segment_that_runs_past_the_end_of_the_file_is_damaged() {
  let mut cut = file(&[frame(0xC0, 8, 4, 4, 1)]);
  cut.truncate(9);
  assert_eq!(
    read(&cut),
    Err(JpegError::Malformed(
      "a segment runs past the end of the file".to_string()
    ))
  );
}

#[test]
fn a_frame_that_states_no_size_is_damaged() {
  assert_eq!(
    read(&file(&[frame(0xC0, 8, 0, 4, 1)])),
    Err(JpegError::Malformed(
      "the frame states a picture with no width or no height".to_string()
    ))
  );
}

#[test]
fn damaged_exif_is_passed_over_and_the_frame_still_read() {
  // The directory is said to start far past the end of the segment.
  let mut bad = exif(true, &[(0x0112, Exif::Short(6))]);
  bad[14..18].copy_from_slice(&4_000_000u32.to_le_bytes());
  let header = read(&file(&[bad, frame(0xC0, 8, 4, 4, 1)])).unwrap();
  assert_eq!((header.width, header.orientation), (4, 1));
}

#[test]
fn no_more_segments_are_walked_than_a_file_could_sensibly_have() {
  let many: Vec<Vec<u8>> = (0..=MOST_SEGMENTS).map(|_| segment(0xFE, b"x")).collect();
  assert_eq!(
    read(&file(&many)),
    Err(JpegError::LimitExceeded(
      "the file has more than 10000 segments before its picture".to_string()
    ))
  );
}
