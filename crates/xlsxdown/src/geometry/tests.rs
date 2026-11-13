use super::*;
use crate::workbook::{WorkbookContext, parse_sheet};

fn sheet_of(xml: &[u8], styles: &Styles) -> Sheet {
  parse_sheet(
    xml,
    "S",
    0,
    false,
    WorkbookContext {
      shared: &[],
      styles,
      date1904: false,
    },
    None,
  )
  .unwrap()
}

#[test]
fn column_widths_follow_the_spreadsheetml_pixel_formula() {
  assert_eq!(column_width_points(9.140625), 48.0);
  assert_eq!(column_width_points(20.0), 105.0);
  assert_eq!(column_width_points(12.0), 63.0);
  assert_eq!(column_width_points(14.0), 73.5);
}

#[test]
fn text_taller_than_its_row_grows_the_row() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Above</t></is></c><c r="B1" t="inlineStr"><is><t>Right</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Line one&#10;Line two</t></is></c><c r="B2" t="inlineStr"><is><t>Beside</t></is></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Below</t></is></c></row></sheetData></worksheet>"##;
  let styles = Styles::default();
  let sheet = sheet_of(xml, &styles);
  let built = build(&sheet, &styles, page_range(&sheet).unwrap(), false);
  // Row 2 grows from 15 pt to two Calibri 11 lines (2 × 13.2 pt), like Excel's auto-fit.
  assert!(
    (built.frame.height - (15.0 + 26.4 + 15.0)).abs() < 1e-9,
    "{}",
    built.frame.height
  );
  assert_eq!(built.grown_rows, 1);
  assert_eq!(built.prepared.len(), 5);
  let two_lines = built
    .prepared
    .iter()
    .find(|p| (p.row, p.col) == (1, 0))
    .unwrap();
  assert_eq!(two_lines.fragments, ["Line one", "Line two"]);
  let geometry = sheet_geometry(&sheet, &styles, false);
  assert_eq!(
    geometry.rows,
    vec![SheetRow {
      index: 1,
      height: 26.4,
      hidden: false
    }],
    "the grown row is the one that differs from the default"
  );
}

#[test]
fn a_reference_past_the_grid_does_not_grow_the_frame() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="A999999999"><v>2</v></c></row></sheetData></worksheet>"##;
  let styles = Styles::default();
  let sheet = sheet_of(xml, &styles);
  let built = build(&sheet, &styles, page_range(&sheet).unwrap(), false);
  assert_eq!((built.frame.width, built.frame.height), (48.0, 15.0));
  assert_eq!(built.prepared.len(), 1);
  assert!(
    sheet.warnings.iter().any(|w| w.contains("malformed")),
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn the_print_area_is_the_page_and_the_content_is_the_sheet() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="2"><c r="B2"><v>1</v></c></row><row r="5"><c r="D5"><v>2</v></c></row></sheetData></worksheet>"##;
  let styles = Styles::default();
  let mut sheet = sheet_of(xml, &styles);
  assert_eq!(page_range(&sheet).map(|r| r.a1()).as_deref(), Some("B2:D5"));
  assert_eq!(
    sheet_range(&sheet).map(|r| r.a1()).as_deref(),
    Some("A1:D5")
  );
  sheet.print_area = CellRange::parse("A1:B3");
  assert_eq!(page_range(&sheet).map(|r| r.a1()).as_deref(), Some("A1:B3"));
  let built = build(&sheet, &styles, page_range(&sheet).unwrap(), false);
  assert!(built.frame.contains(1, 1));
  assert!(!built.frame.contains(4, 3), "D5 is off the page");
  let empty = sheet_of(
    br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"##,
    &styles,
  );
  assert_eq!(page_range(&empty), None);
  assert_eq!(sheet_geometry(&empty, &styles, true).boxes.len(), 0);
}

#[test]
fn sheet_geometry_lists_the_exceptions_and_boxes_every_cell_in_the_a1_frame() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cols><col min="1" max="1" width="20"/></cols><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row><row r="2" ht="30"><c r="B2" t="inlineStr"><is><t>y</t></is></c></row></sheetData><mergeCells count="1"><mergeCell ref="A1:C1"/></mergeCells></worksheet>"##;
  let styles = Styles::default();
  let sheet = sheet_of(xml, &styles);
  let g = sheet_geometry(&sheet, &styles, true);
  // A 20 chars = 105 pt, B and C default 48 pt; the merge A1:C1 extends the frame to column C.
  assert_eq!((g.width, g.height), (201.0, 45.0));
  assert_eq!((g.default_column_width, g.default_row_height), (48.0, 15.0));
  assert_eq!(
    g.columns,
    vec![SheetColumn {
      index: 0,
      width: 105.0,
      hidden: false
    }]
  );
  assert_eq!(
    g.rows,
    vec![SheetRow {
      index: 1,
      height: 30.0,
      hidden: false
    }]
  );
  assert_eq!(
    g.boxes[&(0, 0)],
    CellBox {
      x0: 0.0,
      y0: 30.0,
      x1: 201.0,
      y1: 45.0
    },
    "a merged anchor spans its range"
  );
  assert_eq!(
    g.boxes[&(1, 1)],
    CellBox {
      x0: 105.0,
      y0: 0.0,
      x1: 153.0,
      y1: 30.0
    }
  );
}
