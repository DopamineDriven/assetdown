use super::{PaperSize, SIZES, TOLERANCE, paper_neighbors, paper_size};

#[test]
fn every_standard_size_is_itself_in_both_orientations() {
  for (size, short, long) in SIZES {
    assert!(short < long, "{size:?} is listed portrait");
    assert_eq!(paper_size(short, long), size, "{size:?} portrait");
    assert_eq!(paper_size(long, short), size, "{size:?} landscape");
  }
}

#[test]
fn a_size_knows_its_own_dimensions_and_unknown_has_none() {
  for (size, short, long) in SIZES {
    assert_eq!(size.dimensions(), Some((short, long)), "{size:?}");
  }
  assert_eq!(PaperSize::Letter.dimensions(), Some((612.0, 792.0)));
  assert_eq!(PaperSize::Unknown.dimensions(), None);
}

#[test]
fn a_scan_a_little_off_the_exact_size_still_matches() {
  // A letter page scanned at 200 dpi: 1704 pixels wide is 613.44 points.
  assert_eq!(paper_size(613.44, 792.000_03), PaperSize::Letter);
  // ISO sizes written with their unrounded millimeters.
  assert_eq!(paper_size(595.276, 841.89), PaperSize::A4);
  assert_eq!(paper_size(1190.55, 841.89), PaperSize::A3);
}

#[test]
fn a_page_further_than_the_tolerance_from_every_size_is_unknown() {
  let just_inside = 595.0 * (1.0 + TOLERANCE - 0.001);
  let just_outside = 595.0 * (1.0 + TOLERANCE + 0.001);
  assert_eq!(paper_size(just_inside, 842.0), PaperSize::A4);
  assert_eq!(paper_size(just_outside, 842.0), PaperSize::Unknown);
  // Each side is judged on its own: the right width with the wrong height is not a match.
  assert_eq!(paper_size(612.0, 900.0), PaperSize::Unknown);
  assert_eq!(paper_size(500.0, 500.0), PaperSize::Unknown);
}

#[test]
fn a_page_between_two_sizes_goes_to_the_nearer() {
  // A5 is 420 by 595 and Statement is 396 by 612.
  assert_eq!(paper_size(417.0, 597.0), PaperSize::A5);
  assert_eq!(paper_size(399.0, 610.0), PaperSize::Statement);
  // Letter and Legal share a width and differ in height.
  assert_eq!(paper_size(612.0, 1000.0), PaperSize::Legal);
}

#[test]
fn a_page_without_a_positive_finite_size_is_unknown() {
  assert_eq!(paper_size(0.0, 0.0), PaperSize::Unknown);
  assert_eq!(paper_size(612.0, 0.0), PaperSize::Unknown);
  assert_eq!(paper_size(-612.0, 792.0), PaperSize::Unknown);
  assert_eq!(paper_size(f64::NAN, 792.0), PaperSize::Unknown);
  assert_eq!(paper_size(612.0, f64::INFINITY), PaperSize::Unknown);
}

#[test]
fn an_unknown_page_lies_between_the_sizes_on_either_side_by_area() {
  // A letter-width page cropped short: too far from Letter to be called one.
  assert_eq!(paper_size(610.0, 760.0), PaperSize::Unknown);
  assert_eq!(
    paper_neighbors(610.0, 760.0),
    (Some(PaperSize::Executive), Some(PaperSize::Letter))
  );
  assert_eq!(
    paper_neighbors(300.0, 500.0),
    (Some(PaperSize::A6), Some(PaperSize::B6))
  );
  // Orientation does not matter.
  assert_eq!(paper_neighbors(500.0, 300.0), paper_neighbors(300.0, 500.0));
}

#[test]
fn a_page_beyond_an_end_of_the_table_has_a_neighbor_on_one_side_only() {
  assert_eq!(paper_neighbors(50.0, 80.0), (None, Some(PaperSize::A10)));
  assert_eq!(paper_neighbors(3000.0, 5000.0), (Some(PaperSize::B0), None));
  assert_eq!(paper_neighbors(0.0, 0.0), (None, None));
  assert_eq!(paper_neighbors(f64::NAN, 792.0), (None, None));
}

#[test]
fn a_page_the_size_of_a_standard_has_that_standard_below_it() {
  assert_eq!(
    paper_neighbors(612.0, 792.0),
    (Some(PaperSize::Letter), Some(PaperSize::A4))
  );
}
