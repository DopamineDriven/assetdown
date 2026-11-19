use super::*;
use crate::testing::{TiffValue, exif_block, tiff};

#[test]
fn a_camera_s_notes_are_read_from_their_three_directories_in_either_byte_order() {
  for little in [true, false] {
    let exif = from_bytes(&exif_block(little)).unwrap();
    assert_eq!(exif.orientation, Some(6));
    assert_eq!(
      (exif.make.as_deref(), exif.model.as_deref()),
      (Some("Leica Camera AG"), Some("M11"))
    );
    assert_eq!(exif.lens_model.as_deref(), Some("Summilux-M 1:1.4/35"));
    // As the camera wrote it, with the offset it recorded.
    assert_eq!(
      exif.date_time_original.as_deref(),
      Some("2026-11-17T23:36:27-05:00")
    );
    assert_eq!(exif.color_space, Some(1));
    let (latitude, longitude) = exif.position.unwrap();
    // 33 degrees 51 minutes 24.84 seconds south, 70 degrees 39 minutes 36 seconds west.
    assert!((latitude + 33.8569).abs() < 0.0001, "{latitude}");
    assert!((longitude + 70.66).abs() < 0.0001, "{longitude}");
  }
}

#[test]
fn a_date_with_no_offset_is_given_none_and_a_date_of_zeros_is_no_date() {
  let taken = |written: &'static str| {
    let block = tiff(
      true,
      &[vec![(
        0x8769,
        TiffValue::Directory(vec![(0x9003, TiffValue::Text(written))]),
      )]],
    );
    from_bytes(&block).unwrap().date_time_original
  };
  assert_eq!(
    taken("2026:11:17 23:36:27").as_deref(),
    Some("2026-11-17T23:36:27")
  );
  assert_eq!(taken("0000:00:00 00:00:00"), None);
  assert_eq!(taken("    :  :     :  :  "), None);
}

#[test]
fn a_block_that_says_little_says_only_that() {
  let exif = from_bytes(&tiff(false, &[vec![(0x0112, TiffValue::Short(vec![1]))]])).unwrap();
  assert_eq!(
    exif,
    Exif {
      orientation: Some(1),
      ..Exif::default()
    }
  );
  // A turn that is not one of the eight there are.
  let odd = from_bytes(&tiff(true, &[vec![(0x0112, TiffValue::Short(vec![9]))]])).unwrap();
  assert_eq!(odd.orientation, None);
  assert_eq!(from_bytes(b"not a block"), None);
}

#[test]
fn a_pointer_left_dangling_loses_what_it_pointed_at_and_nothing_else() {
  // A program that rewrote the picture left the place pointing nowhere: past the end of the
  // block, or at zero. How the picture is turned and who made it are still there.
  for nowhere in [90_000, 0] {
    let block = tiff(
      true,
      &[vec![
        (0x010F, TiffValue::Text("Leica Camera AG")),
        (0x0112, TiffValue::Short(vec![6])),
        (0x8825, TiffValue::Long(vec![nowhere])),
      ]],
    );
    let exif = from_bytes(&block).unwrap();
    assert_eq!(exif.orientation, Some(6), "{nowhere}");
    assert_eq!(exif.make.as_deref(), Some("Leica Camera AG"), "{nowhere}");
    assert_eq!(exif.position, None, "{nowhere}");
  }
}
