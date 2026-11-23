use xlsxdown::DocProps;

use super::*;
use crate::testing::workbook;

#[test]
fn the_properties_and_the_counts_are_joined_into_one_object() {
  let mut workbook = workbook();
  workbook.props = DocProps {
    title: Some("Order form".to_string()),
    creator: Some("A. Clerk".to_string()),
    ..DocProps::default()
  };
  let meta = metadata_of(&workbook);
  assert_eq!(meta.title.as_deref(), Some("Order form"));
  // The creator is the author.
  assert_eq!(meta.author.as_deref(), Some("A. Clerk"));
  assert_eq!((meta.subject, meta.company), (None, None));
  assert_eq!((meta.sheet_count, meta.hidden_sheet_count), (2, 1));
  assert_eq!((meta.cell_count, meta.formula_count), (5, 2));
  assert_eq!(meta.sheets[0].used_range.as_deref(), Some("A1:B3"));
  assert!(!meta.date1904);
}

#[test]
fn what_kind_of_file_the_workbook_came_from_is_passed_on() {
  let mut workbook = workbook();
  let plain = metadata_of(&workbook);
  assert!(!plain.has_macros && !plain.is_template);
  workbook.has_macros = true;
  let enabled = metadata_of(&workbook);
  assert!(enabled.has_macros && !enabled.is_template);
}
