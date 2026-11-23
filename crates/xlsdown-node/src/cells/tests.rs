use super::*;
use crate::testing::workbook;
use crate::types::SheetCell;

fn options(hidden_sheets: bool) -> SpreadsheetOptions {
  SpreadsheetOptions {
    sheets: None,
    hidden_sheets: Some(hidden_sheets),
    hidden_cells: Some(true),
  }
}

#[test]
fn the_visible_sheets_come_back_with_each_value_as_its_own_variant() {
  let sheets = cells_of(&workbook(), &options(false));
  assert_eq!(sheets.len(), 1);
  let sheet = &sheets[0];
  assert_eq!((sheet.sheet.as_str(), sheet.cells.len()), ("Costs", 4));
  match &sheet.cells[0] {
    SheetCell::Text {
      r#ref, value, text, ..
    } => assert_eq!(
      (r#ref.as_str(), value.as_str(), text.as_str()),
      ("A1", "Total", "Total")
    ),
    other => panic!("{other:?}"),
  }
  match &sheet.cells[1] {
    SheetCell::Date {
      r#ref,
      value,
      text,
      raw,
      ..
    } => assert_eq!(
      (r#ref.as_str(), value.as_str(), text.as_str(), raw.as_str()),
      ("B1", "2026-11-23", "11/23/2026", "46349")
    ),
    other => panic!("{other:?}"),
  }
  match &sheet.cells[2] {
    SheetCell::Number { value, text, .. } => {
      assert_eq!((*value, text.as_str()), (1234.5, "1,234.50"));
    }
    other => panic!("{other:?}"),
  }
}

#[test]
fn a_formula_s_text_is_empty_and_a_cell_that_follows_one_names_the_cell_that_holds_it() {
  let sheets = cells_of(&workbook(), &options(false));
  match &sheets[0].cells[2] {
    SheetCell::Number {
      formula,
      shared_formula,
      cached,
      ..
    } => assert_eq!(
      (formula.as_deref(), shared_formula.as_deref(), *cached),
      (Some(""), None, true)
    ),
    other => panic!("{other:?}"),
  }
  match &sheets[0].cells[3] {
    SheetCell::Number {
      formula,
      shared_formula,
      ..
    } => assert_eq!(
      (formula.as_deref(), shared_formula.as_deref()),
      (None, Some("B2"))
    ),
    other => panic!("{other:?}"),
  }
}

#[test]
fn a_hidden_sheet_comes_back_when_it_is_asked_for() {
  let sheets = cells_of(&workbook(), &options(true));
  let names: Vec<&str> = sheets.iter().map(|sheet| sheet.sheet.as_str()).collect();
  assert_eq!(names, ["Costs", "Notes"]);
  assert!(sheets[1].hidden);
}
