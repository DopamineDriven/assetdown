use super::*;

#[test]
fn source_ppi_comes_from_the_painted_size_and_falls_back_to_a_scan_default() {
  // A 1728 px wide fax raster painted 8.64 in wide (622.08 pt) is a 200 dpi scan.
  assert_eq!(source_ppi(1728, 622.08), 200);
  // 2480 px over an A4 width of 595.28 pt is 300 dpi.
  assert_eq!(source_ppi(2480, 595.28), 300);
  // No usable placement: Tesseract's own guess of 70 dpi is wrong for scans; assume 300.
  assert_eq!(source_ppi(1728, 0.0), DEFAULT_SOURCE_PPI);
  assert_eq!(source_ppi(1728, f64::NAN), DEFAULT_SOURCE_PPI);
  assert_eq!(source_ppi(0, 622.08), DEFAULT_SOURCE_PPI);
  // Tiny or giant placements clamp to the range Tesseract handles.
  assert_eq!(source_ppi(10_000, 10.0), MAX_SOURCE_PPI);
  assert_eq!(source_ppi(100, 10_000.0), MIN_SOURCE_PPI);
}

fn white(width: u32, height: u32) -> image::DynamicImage {
  image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(
    width,
    height,
    image::Luma([255]),
  ))
}

#[test]
fn a_picture_with_too_few_pixels_to_the_inch_is_brought_up_to_the_working_resolution() {
  // 850 pixels across a page 612 points wide is 100 to the inch: doubled, it is 200.
  let larger = enlarged(&white(850, 1169), 612.0).unwrap();
  assert_eq!((larger.width(), larger.height()), (1700, 2338));
  assert!(matches!(larger, image::DynamicImage::ImageLuma8(_)));
  // 72 to the inch is brought up by the same rule, to the nearest pixel.
  let small = enlarged(&white(612, 792), 612.0).unwrap();
  assert_eq!((small.width(), small.height()), (1700, 2200));
}

#[test]
fn a_picture_with_enough_pixels_to_the_inch_is_left_as_it_is() {
  assert!(enlarged(&white(1700, 2200), 612.0).is_none());
  assert!(enlarged(&white(2550, 3300), 612.0).is_none());
}

#[test]
fn a_picture_whose_page_has_no_width_is_left_as_it_is() {
  assert!(enlarged(&white(850, 1169), 0.0).is_none());
  assert!(enlarged(&white(850, 1169), f64::NAN).is_none());
  assert!(enlarged(&white(0, 0), 612.0).is_none());
}

#[test]
fn a_picture_within_a_twentieth_of_the_working_resolution_is_left_as_it_is() {
  // An A4 page rendered at 200 to the inch comes to 1,653 pixels across 595.28 points: a
  // rounding short of 200, and nothing to gain from making it again.
  assert!(enlarged(&white(1653, 2339), 595.28).is_none());
  // 1,600 pixels across a letter page is 188 to the inch, which is short enough to matter.
  let larger = enlarged(&white(1600, 2071), 612.0).unwrap();
  assert_eq!((larger.width(), larger.height()), (1700, 2200));
}
