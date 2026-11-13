//! Whole files through the public API: the fixtures pdfdown's spreadsheet suite reads, with the
//! same expectations stated against the cell model.

use xlsxdown::{
  Cell, CellBox, CellRange, CellValue, Sheet, SheetColumn, SheetRow, SpreadsheetError, Workbook,
  parse_cell_ref, sheet_geometry,
};

fn fixture(name: &str) -> Vec<u8> {
  let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
  std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn open(name: &str) -> Workbook {
  Workbook::open(fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn cell<'a>(sheet: &'a Sheet, reference: &str) -> Option<&'a Cell> {
  sheet.cells.get(&parse_cell_ref(reference).unwrap())
}

fn at<'a>(sheet: &'a Sheet, reference: &str) -> &'a Cell {
  cell(sheet, reference).unwrap_or_else(|| panic!("{reference} is not a cell"))
}

fn a1(range: Option<CellRange>) -> Option<String> {
  range.map(|range| range.a1())
}

#[test]
fn the_form_lists_its_sheets_with_their_dimension_print_area_and_merges() {
  let workbook = open("form.xlsx");
  assert!(!workbook.date1904);
  let sheets: Vec<(&str, u32, bool)> = workbook
    .sheets
    .iter()
    .map(|s| (s.name.as_str(), s.index, s.hidden))
    .collect();
  assert_eq!(sheets, [("Form", 0, false), ("Notes", 1, true)]);
  let form = &workbook.sheets[0];
  assert_eq!(a1(form.dimension).as_deref(), Some("A1:H30"));
  assert_eq!(a1(form.print_area).as_deref(), Some("A1:F12"));
  let merges: Vec<String> = form.merges.iter().map(CellRange::a1).collect();
  assert_eq!(merges, ["A1:F1", "A11:A12"]);
  let notes: Vec<&str> = workbook.sheets[1]
    .cells
    .values()
    .map(|c| c.text.as_str())
    .collect();
  assert_eq!(notes, ["secret"]);
}

#[test]
fn sheets_are_selected_by_name_or_by_visibility() {
  let workbook = open("form.xlsx");
  let names = |selected: Vec<&Sheet>| -> Vec<String> {
    selected.into_iter().map(|s| s.name.clone()).collect()
  };
  assert_eq!(names(workbook.selected_sheets(None, false)), ["Form"]);
  assert_eq!(
    names(workbook.selected_sheets(None, true)),
    ["Form", "Notes"]
  );
  let wanted = ["Notes".to_string()];
  assert_eq!(
    names(workbook.selected_sheets(Some(&wanted), false)),
    ["Notes"],
    "a sheet asked for by name is returned even when hidden"
  );
  let reversed = ["Notes".to_string(), "Form".to_string()];
  assert_eq!(
    names(workbook.selected_sheets(Some(&reversed), false)),
    ["Form", "Notes"],
    "workbook order, not the order asked for"
  );
  let unknown = ["Nope".to_string()];
  assert!(workbook.selected_sheets(Some(&unknown), true).is_empty());
}

#[test]
fn cells_are_typed_and_keep_their_stored_spelling_and_display_text() {
  let workbook = open("form.xlsx");
  let form = &workbook.sheets[0];

  let title = at(form, "A1");
  assert_eq!(
    title.value,
    CellValue::Text("Maintenance Reimbursement Form".to_string())
  );
  assert_eq!(title.text, "Maintenance Reimbursement Form");
  assert_eq!(a1(title.merge).as_deref(), Some("A1:F1"));

  let rate = at(form, "C3");
  assert_eq!(rate.value, CellValue::Number(142.5));
  assert_eq!(rate.raw, "142.5");
  assert_eq!(rate.format.as_deref(), Some("#,##0.00\"/hr\""));
  assert_eq!(rate.formula, None);
  assert!(rate.cached);
  assert_eq!(rate.currency_symbol, None);

  let padded = at(form, "C7");
  assert_eq!(padded.value, CellValue::Text("000123".to_string()));
  assert_eq!(
    (padded.text.as_str(), padded.raw.as_str()),
    ("000123", "000123")
  );

  let error = at(form, "D20");
  assert_eq!(error.value, CellValue::Error("#DIV/0!".to_string()));
  assert_eq!(error.text, "#DIV/0!");

  let boolean = at(form, "E21");
  assert_eq!(boolean.value, CellValue::Boolean(true));
  assert_eq!(boolean.text, "TRUE");

  assert_eq!(at(form, "C8").currency_symbol.as_deref(), Some("$"));
  assert!(matches!(at(form, "C6").value, CellValue::Date(_)));
  assert_eq!(at(form, "A11").text, "Line one\nLine two");
  assert_eq!(a1(at(form, "A11").merge).as_deref(), Some("A11:A12"));
  assert!(
    cell(form, "G20").is_none(),
    "a styled empty cell is not a cell"
  );
  assert!(
    form
      .warnings
      .iter()
      .any(|w| w.contains("C22") && w.contains("serial 60")),
    "{:?}",
    form.warnings
  );
}

#[test]
fn every_formula_cell_has_its_formula_and_followers_also_name_their_master() {
  let workbook = open("form.xlsx");
  let form = &workbook.sheets[0];

  let direct = at(form, "C5");
  assert_eq!(direct.value, CellValue::Number(427.5));
  assert_eq!(direct.formula.as_deref(), Some("C3*C4"));
  assert!(direct.cached);

  let master = at(form, "B16");
  assert_eq!(master.formula.as_deref(), Some("C3*2"));
  assert_eq!(master.shared_master, None);
  assert_eq!(master.value, CellValue::Number(285.0));

  let master_position = parse_cell_ref("B16");
  let follower = at(form, "B17");
  assert_eq!(follower.value, CellValue::Text(String::new()));
  assert_eq!(
    follower.formula.as_deref(),
    Some("C4*2"),
    "the master's C3*2, one row down"
  );
  assert_eq!(follower.shared_master, master_position);
  assert!(follower.cached);

  let uncached = at(form, "B18");
  assert_eq!(uncached.value, CellValue::Blank);
  assert_eq!(uncached.formula.as_deref(), Some("C5*2"));
  assert_eq!(uncached.shared_master, master_position);
  assert!(!uncached.cached);
}

#[test]
fn hidden_rows_are_marked_and_geometry_follows_the_callers_choice() {
  let workbook = open("form.xlsx");
  let form = &workbook.sheets[0];
  assert_eq!(at(form, "B10").text, "hidden value");
  assert!(form.row_hidden(9));
  assert!(!form.row_hidden(10));

  // The sheet frame runs from A1 to the last non-empty cell or merged range (A1:F22 here), y up.
  let all = sheet_geometry(form, &workbook.styles, true);
  assert_eq!((all.width, all.height), (385.5, 345.0));
  assert_eq!(
    (all.default_column_width, all.default_row_height),
    (48.0, 15.0)
  );
  let column = |index, width| SheetColumn {
    index,
    width,
    hidden: false,
  };
  assert_eq!(
    all.columns,
    [column(0, 105.0), column(1, 63.0), column(2, 73.5)]
  );
  let row = |index, height, hidden| SheetRow {
    index,
    height,
    hidden,
  };
  assert_eq!(all.rows, [row(9, 15.0, true), row(10, 30.0, false)]);
  let rectangle = |reference: &str| all.boxes[&parse_cell_ref(reference).unwrap()];
  let expected = |x0, y0, x1, y1| CellBox { x0, y0, x1, y1 };
  assert_eq!(rectangle("C3"), expected(168.0, 300.0, 241.5, 315.0));
  assert_eq!(
    rectangle("A1"),
    expected(0.0, 330.0, 385.5, 345.0),
    "a merged anchor spans its range"
  );
  assert_eq!(rectangle("A11"), expected(0.0, 150.0, 105.0, 195.0));
  assert_eq!(all.boxes.len(), form.cells.len(), "every cell has a box");
  assert!(
    all
      .boxes
      .values()
      .all(|b| b.x0 <= b.x1 && b.y0 < b.y1 && b.x1 <= all.width && b.y1 <= all.height)
  );

  let visible = sheet_geometry(form, &workbook.styles, false);
  assert_eq!(
    visible.height, 330.0,
    "a hidden row has no extent when hidden cells are excluded"
  );
  assert_eq!(visible.rows, [row(9, 0.0, true), row(10, 30.0, false)]);
}

#[test]
fn a_workbook_without_shared_strings_or_styles_still_resolves_every_cell() {
  let workbook = open("minimal.xlsx");
  assert_eq!(workbook.sheets.len(), 1);
  let sheet = &workbook.sheets[0];
  assert_eq!(sheet.cells.len(), 4);
  assert_eq!(
    sheet_geometry(sheet, &workbook.styles, true).columns,
    [],
    "no column differs from the default"
  );
}

#[test]
fn the_1904_date_system_is_read_from_the_workbook() {
  let workbook = open("date1904.xlsx");
  assert!(workbook.date1904);
  let dates: Vec<&str> = workbook
    .sheets
    .iter()
    .flat_map(|s| s.cells.values())
    .filter_map(|c| match &c.value {
      CellValue::Date(iso) => Some(iso.as_str()),
      _ => None,
    })
    .collect();
  assert!(!dates.is_empty(), "the fixture holds at least one date");
  assert!(dates.iter().all(|&iso| iso >= "1904-01-01"), "{dates:?}");
}

#[test]
fn packages_that_are_not_plain_workbooks_are_unsupported_and_say_what_they_are() {
  let detail = |name: &str| match Workbook::open(fixture(name)) {
    Err(SpreadsheetError::UnsupportedFormat(detail)) => detail,
    other => panic!("{name}: {other:?}"),
  };
  assert_eq!(
    detail("macro.xlsm"),
    "macro-enabled workbook (not supported in this release)"
  );
  assert_eq!(
    detail("strict.xlsx"),
    "Strict OOXML workbook (not supported in this release)"
  );
  assert_eq!(detail("not-a-workbook.docx"), "Word document package");
  assert_eq!(
    Workbook::open(fixture("macro.xlsm"))
      .unwrap_err()
      .to_string(),
    "unsupported format: macro-enabled workbook (not supported in this release)"
  );
}

#[test]
fn what_is_not_a_zip_is_malformed_and_what_is_too_large_is_a_limit_error() {
  assert!(matches!(
    Workbook::open(b"hello".to_vec()),
    Err(SpreadsheetError::Malformed(_))
  ));
  let mut oversized = vec![0u8; 65 * 1024 * 1024];
  oversized[..4].copy_from_slice(b"PK\x03\x04");
  let error = Workbook::open(oversized).unwrap_err();
  assert!(
    matches!(&error, SpreadsheetError::LimitExceeded(d) if d.starts_with("the package is ")),
    "{error}"
  );
  assert!(
    error
      .to_string()
      .starts_with("limit exceeded: the package is ")
  );
}
