use super::*;

#[test]
fn the_four_corners_of_the_grid_parse_and_one_step_past_each_edge_does_not() {
  assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
  assert_eq!(parse_cell_ref("XFD1"), Some((0, MAX_COLS - 1)));
  assert_eq!(parse_cell_ref("A1048576"), Some((MAX_ROWS - 1, 0)));
  assert_eq!(
    parse_cell_ref("XFD1048576"),
    Some((MAX_ROWS - 1, MAX_COLS - 1))
  );
  assert_eq!(parse_cell_ref("XFE1"), None, "one column past XFD");
  assert_eq!(parse_cell_ref("A1048577"), None, "one row past the last");
  assert_eq!(parse_cell_ref("A0"), None, "rows count from 1");
  assert_eq!(parse_cell_ref("ZZZZZZZZ1"), None, "a column that overflows");
  assert_eq!(parse_cell_ref("A99999999999"), None, "a row that overflows");
}

#[test]
fn absolute_markers_and_lower_case_name_the_same_cell() {
  for spelling in ["H12", "$H12", "H$12", "$H$12", "h12", "$h$12"] {
    assert_eq!(parse_cell_ref(spelling), Some((11, 7)), "{spelling}");
  }
  assert_eq!(parse_cell_ref("AA10"), Some((9, 26)));
  assert_eq!(parse_cell_ref("H012"), Some((11, 7)), "leading zeros");
}

#[test]
fn what_is_not_a_cell_reference_is_none() {
  for text in [
    "",
    "H",
    "12",
    "12H",
    "H12K",
    "H 12",
    "H-1",
    "H+1",
    "H1.5",
    "$",
    "$$H1",
    "H1$",
    "H$$1",
    "Ä1",
    "H1:K2",
    "Sheet1!H1",
  ] {
    assert_eq!(parse_cell_ref(text), None, "{text:?}");
  }
}

#[test]
fn every_column_survives_a_round_trip() {
  for column in 0..MAX_COLS {
    let text = cell_ref(6, column);
    assert_eq!(parse_cell_ref(&text), Some((6, column)), "{text}");
  }
  assert_eq!(cell_ref(0, 0), "A1");
  assert_eq!(cell_ref(1, 25), "Z2");
  assert_eq!(cell_ref(1, 26), "AA2");
  assert_eq!(cell_ref(1, 27), "AB2");
  assert_eq!(cell_ref(MAX_ROWS - 1, MAX_COLS - 1), "XFD1048576");
}

#[test]
fn ranges_are_normalized_and_a_single_cell_is_a_range_of_one() {
  let expected = CellRange {
    r0: 1,
    c0: 1,
    r1: 4,
    c1: 3,
  };
  for spelling in ["B2:D5", "D5:B2", "B5:D2", "D2:B5", "$B$2:$D$5"] {
    assert_eq!(CellRange::parse(spelling), Some(expected), "{spelling}");
  }
  assert_eq!(expected.a1(), "B2:D5");
  assert_eq!(
    CellRange::parse("F7").map(|r| r.a1()).as_deref(),
    Some("F7:F7")
  );
}

#[test]
fn what_is_not_a_rectangle_of_cells_is_none() {
  for text in [
    "", ":", "B2:", ":D5", "B2:D5:F7", "A:A", "1:1", "B2;D5", "B2 : D5",
  ] {
    assert_eq!(CellRange::parse(text), None, "{text:?}");
  }
}
