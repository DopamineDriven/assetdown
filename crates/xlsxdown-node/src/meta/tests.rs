use xlsxdown::DocProps;

use super::*;

fn form() -> Workbook {
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/__test__/fixtures/form.xlsx");
  let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}"));
  Workbook::open(bytes).unwrap()
}

#[test]
fn the_properties_and_the_counts_are_joined_into_one_object() {
  // The fixture states no properties, so they are set here as a file would state them.
  let mut workbook = form();
  workbook.props = DocProps {
    title: Some("Order form".to_string()),
    creator: Some("A. Clerk".to_string()),
    created: Some("2026-11-15T09:30:00Z".to_string()),
    application: Some("Microsoft Excel".to_string()),
    ..DocProps::default()
  };
  let meta = metadata_of(&workbook);
  assert_eq!(meta.title.as_deref(), Some("Order form"));
  // `dc:creator` is the author.
  assert_eq!(meta.author.as_deref(), Some("A. Clerk"));
  assert_eq!(meta.created.as_deref(), Some("2026-11-15T09:30:00Z"));
  assert_eq!(meta.application.as_deref(), Some("Microsoft Excel"));
  assert_eq!(
    (meta.subject, meta.modified, meta.company),
    (None, None, None)
  );
  assert_eq!((meta.sheet_count, meta.hidden_sheet_count), (2, 1));
  assert!(!meta.date1904);
}

#[test]
fn ranges_are_given_in_a1_form_and_an_empty_sheet_has_no_used_range() {
  let mut workbook = form();
  let meta = metadata_of(&workbook);
  let sheet = &meta.sheets[0];
  assert_eq!(sheet.name, "Form");
  assert_eq!(sheet.dimension.as_deref(), Some("A1:H30"));
  let used = sheet.used_range.as_deref().expect("the form holds cells");
  let range = CellRange::parse(used).expect("an A1 range");
  assert_eq!(
    (sheet.row_count, sheet.column_count),
    (range.r1 - range.r0 + 1, range.c1 - range.c0 + 1)
  );

  workbook.sheets[1].cells.clear();
  let emptied = metadata_of(&workbook);
  assert_eq!(emptied.sheets[1].used_range, None);
  assert_eq!(
    (emptied.sheets[1].row_count, emptied.sheets[1].cell_count),
    (0, 0)
  );
}
