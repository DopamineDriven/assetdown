use super::*;

#[test]
fn osd_below_threshold_is_ignored() {
  assert_eq!(pick_first_rotation(Some((90, 2.9))), None);
}

#[test]
fn osd_seeds_the_first_pass() {
  assert_eq!(pick_first_rotation(Some((90, 6.0))), Some(90));
  assert_eq!(pick_first_rotation(Some((0, 10.0))), Some(0));
}

#[test]
fn no_osd_means_no_seed() {
  assert_eq!(pick_first_rotation(None), None);
}

#[test]
fn convincing_upright_reads_skip_the_detector() {
  assert!(upright_is_convincing(182, 95.6), "a real upright page");
  assert!(
    !upright_is_convincing(24, 62.4),
    "the synthetic page read upside down"
  );
  assert!(
    !upright_is_convincing(449, 41.4),
    "a real page read sideways"
  );
  assert!(
    !upright_is_convincing(0, 100.0),
    "no words is never convincing"
  );
}

#[test]
fn osd_orientation_becomes_a_clockwise_correction() {
  // Tesseract reports how the text currently lies; the CLI prints "Orientation 270 / Rotate 90".
  assert_eq!(osd_correction(270), 90);
  assert_eq!(osd_correction(90), 270);
  assert_eq!(osd_correction(180), 180);
  assert_eq!(osd_correction(0), 0);
}

#[test]
fn unrotating_a_pixel_box_undoes_the_raster_turn() {
  // an 800×200 raster turned 90° clockwise is 200×800; a box in the turned raster maps back
  assert_eq!(
    unrotate_pixel_box([140, 10, 180, 110], 90, 200, 800),
    [10, 20, 110, 60]
  );
  // 270° clockwise (= 90° counter-clockwise): the same original box lands elsewhere
  assert_eq!(
    unrotate_pixel_box([20, 690, 60, 790], 270, 200, 800),
    [10, 20, 110, 60]
  );
  // 180° keeps the dimensions
  assert_eq!(
    unrotate_pixel_box([690, 140, 790, 180], 180, 800, 200),
    [10, 20, 110, 60]
  );
  assert_eq!(
    unrotate_pixel_box([10, 20, 110, 60], 0, 800, 200),
    [10, 20, 110, 60]
  );
}
