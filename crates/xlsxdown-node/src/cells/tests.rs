use super::*;

fn form() -> Workbook {
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/__test__/fixtures/form.xlsx");
  let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}"));
  Workbook::open(bytes).unwrap()
}

fn options(sheets: Option<&[&str]>, hidden_sheets: bool, hidden_cells: bool) -> SpreadsheetOptions {
  SpreadsheetOptions {
    sheets: sheets.map(|names| names.iter().map(|name| name.to_string()).collect()),
    hidden_sheets: Some(hidden_sheets),
    hidden_cells: Some(hidden_cells),
  }
}

fn reference(cell: &SheetCell) -> &str {
  match cell {
    SheetCell::Text { r#ref, .. }
    | SheetCell::Number { r#ref, .. }
    | SheetCell::Boolean { r#ref, .. }
    | SheetCell::Error { r#ref, .. }
    | SheetCell::Date { r#ref, .. }
    | SheetCell::Time { r#ref, .. }
    | SheetCell::Blank { r#ref, .. } => r#ref,
  }
}

fn find<'a>(sheet: &'a SheetCells, wanted: &str) -> Option<&'a SheetCell> {
  sheet.cells.iter().find(|cell| reference(cell) == wanted)
}

fn at<'a>(sheet: &'a SheetCells, wanted: &str) -> &'a SheetCell {
  find(sheet, wanted).unwrap_or_else(|| panic!("{wanted} is not among the cells"))
}

#[test]
fn whole_numbers_javascript_holds_exactly_are_integers() {
  assert_eq!(number_subtype(3.0), NumberSubtype::Integer);
  assert_eq!(number_subtype(-3.0), NumberSubtype::Integer);
  assert_eq!(number_subtype(0.0), NumberSubtype::Integer);
  assert_eq!(number_subtype(0.5), NumberSubtype::Float);
  assert_eq!(number_subtype(MAX_EXACT_INTEGER), NumberSubtype::Integer);
  assert_eq!(number_subtype(-MAX_EXACT_INTEGER), NumberSubtype::Integer);
  assert_eq!(
    number_subtype(MAX_EXACT_INTEGER * 2.0),
    NumberSubtype::Float,
    "whole, but past what a JavaScript number holds exactly"
  );
  assert_eq!(number_subtype(1e300), NumberSubtype::Float);
}

#[test]
fn by_default_the_visible_sheets_come_back_with_their_facts() {
  let sheets = cells_of(&form(), &SpreadsheetOptions::default());
  assert_eq!(sheets.len(), 1);
  let sheet = &sheets[0];
  assert_eq!(
    (sheet.sheet.as_str(), sheet.index, sheet.hidden),
    ("Form", 0, false)
  );
  assert_eq!(sheet.dimension.as_deref(), Some("A1:H30"));
  assert_eq!(sheet.print_area.as_deref(), Some("A1:F12"));
  assert_eq!(sheet.merges, ["A1:F1", "A11:A12"]);
  assert_eq!((sheet.width, sheet.height), (385.5, 345.0));
  assert_eq!(
    (sheet.default_column_width, sheet.default_row_height),
    (48.0, 15.0)
  );
  let columns: Vec<(u32, f64, bool)> = sheet
    .columns
    .iter()
    .map(|c| (c.index, c.width, c.hidden))
    .collect();
  assert_eq!(
    columns,
    [(0, 105.0, false), (1, 63.0, false), (2, 73.5, false)]
  );
  let rows: Vec<(u32, f64, bool)> = sheet
    .rows
    .iter()
    .map(|r| (r.index, r.height, r.hidden))
    .collect();
  assert_eq!(rows, [(9, 15.0, true), (10, 30.0, false)]);
  assert!(
    sheet.warnings.iter().any(|w| w.contains("serial 60")),
    "{:?}",
    sheet.warnings
  );
}

#[test]
fn each_kind_of_value_becomes_its_own_variant() {
  let sheets = cells_of(&form(), &SpreadsheetOptions::default());
  let sheet = &sheets[0];
  assert!(matches!(
    at(sheet, "A1"),
    SheetCell::Text { value, text, merge: Some(merge), row: 0, col: 0, .. }
      if value == "Maintenance Reimbursement Form" && text == value && merge == "A1:F1"
  ));
  assert!(matches!(
    at(sheet, "C3"),
    SheetCell::Number {
      value,
      subtype: NumberSubtype::Float,
      currency_symbol: None,
      raw,
      format: Some(format),
      formula: None,
      cached: true,
      ..
    } if *value == 142.5 && raw == "142.5" && format == "#,##0.00\"/hr\""
  ));
  assert!(matches!(
    at(sheet, "C4"),
    SheetCell::Number {
      subtype: NumberSubtype::Integer,
      ..
    }
  ));
  assert!(matches!(
    at(sheet, "C8"),
    SheetCell::Number { subtype: NumberSubtype::Integer, currency_symbol: Some(symbol), .. }
      if symbol == "$"
  ));
  assert!(matches!(
    at(sheet, "C9"),
    SheetCell::Number {
      subtype: NumberSubtype::Float,
      ..
    }
  ));
  assert!(matches!(at(sheet, "C6"), SheetCell::Date { .. }));
  assert!(matches!(
    at(sheet, "D20"),
    SheetCell::Error { value, text, .. } if value == "#DIV/0!" && text == "#DIV/0!"
  ));
  assert!(matches!(
    at(sheet, "E21"),
    SheetCell::Boolean { value: true, text, .. } if text == "TRUE"
  ));
  assert!(
    find(sheet, "G20").is_none(),
    "a styled empty cell is not a cell"
  );
}

#[test]
fn formula_cells_carry_their_formula_and_followers_name_their_master() {
  let sheets = cells_of(&form(), &SpreadsheetOptions::default());
  let sheet = &sheets[0];
  assert!(matches!(
    at(sheet, "C5"),
    SheetCell::Number { value, formula: Some(formula), shared_formula: None, cached: true, .. }
      if *value == 427.5 && formula == "C3*C4"
  ));
  assert!(matches!(
    at(sheet, "B16"),
    SheetCell::Number { formula: Some(formula), shared_formula: None, .. } if formula == "C3*2"
  ));
  assert!(matches!(
    at(sheet, "B17"),
    SheetCell::Text { formula: Some(formula), shared_formula: Some(master), cached: true, .. }
      if formula == "C4*2" && master == "B16"
  ));
  assert!(matches!(
    at(sheet, "B18"),
    SheetCell::Blank { formula: Some(formula), shared_formula: Some(master), cached: false, .. }
      if formula == "C5*2" && master == "B16"
  ));
}

#[test]
fn every_cell_has_a_rectangle_and_a_merged_anchor_spans_its_range() {
  let sheets = cells_of(&form(), &SpreadsheetOptions::default());
  let sheet = &sheets[0];
  let rectangle = |wanted: &str| match at(sheet, wanted) {
    SheetCell::Text { r#box, .. }
    | SheetCell::Number { r#box, .. }
    | SheetCell::Boolean { r#box, .. }
    | SheetCell::Error { r#box, .. }
    | SheetCell::Date { r#box, .. }
    | SheetCell::Time { r#box, .. }
    | SheetCell::Blank { r#box, .. } => *r#box,
  };
  let expected = |x0, y0, x1, y1| CellBox { x0, y0, x1, y1 };
  assert_eq!(rectangle("C3"), expected(168.0, 300.0, 241.5, 315.0));
  assert_eq!(rectangle("A1"), expected(0.0, 330.0, 385.5, 345.0));
  assert_eq!(rectangle("A11"), expected(0.0, 150.0, 105.0, 195.0));
  for cell in &sheet.cells {
    let b = rectangle(reference(cell));
    assert!(
      b.x0 <= b.x1 && b.y0 < b.y1 && b.x1 <= sheet.width && b.y1 <= sheet.height,
      "{}: {b:?}",
      reference(cell)
    );
  }
}

#[test]
fn hidden_cells_are_marked_by_default_and_left_out_on_request() {
  let workbook = form();
  let with = cells_of(&workbook, &SpreadsheetOptions::default());
  assert!(matches!(
    at(&with[0], "B10"),
    SheetCell::Text { hidden: true, text, .. } if text == "hidden value"
  ));
  let without = cells_of(&workbook, &options(None, false, false));
  assert!(find(&without[0], "B10").is_none());
  assert_eq!(
    without[0].height, 330.0,
    "a hidden row has no extent when hidden cells are excluded"
  );
  assert_eq!(with[0].cells.len(), without[0].cells.len() + 1);
}

#[test]
fn sheets_are_chosen_by_name_or_by_visibility() {
  let workbook = form();
  let names = |opts: &SpreadsheetOptions| -> Vec<String> {
    cells_of(&workbook, opts)
      .into_iter()
      .map(|sheet| sheet.sheet)
      .collect()
  };
  assert_eq!(names(&SpreadsheetOptions::default()), ["Form"]);
  assert_eq!(names(&options(None, true, true)), ["Form", "Notes"]);
  assert_eq!(
    names(&options(Some(&["Notes"]), false, true)),
    ["Notes"],
    "a hidden sheet asked for by name"
  );
  assert!(names(&options(Some(&["Nope"]), true, true)).is_empty());
  let notes = &cells_of(&workbook, &options(Some(&["Notes"]), false, true))[0];
  assert!(notes.hidden);
  assert_eq!(notes.index, 1);
  let texts: Vec<&str> = notes
    .cells
    .iter()
    .map(|cell| match cell {
      SheetCell::Text { text, .. } => text.as_str(),
      other => panic!("unexpected cell {other:?}"),
    })
    .collect();
  assert_eq!(texts, ["secret"]);
}
