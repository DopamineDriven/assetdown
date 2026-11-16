use super::*;
use crate::testing::{Setup, document, p};

#[test]
fn columns_are_lettered_as_a_sheet_letters_them() {
  let letters: Vec<String> = [0, 25, 26, 27, 701, 702].map(column_letters).to_vec();
  assert_eq!(letters, ["A", "Z", "AA", "AB", "ZZ", "AAA"]);
  assert_eq!(cell_ref(0, 0), "A1");
  assert_eq!(cell_ref(11, 2), "C12");
}

#[test]
fn every_cell_is_at_its_row_and_column_and_a_merged_one_names_its_range() {
  let cell =
    |properties: &str, inner: &str| format!("<w:tc><w:tcPr>{properties}</w:tcPr>{inner}</w:tc>");
  let inner_table = format!(
    "<w:tbl><w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>",
    p("in"),
    p("side")
  );
  let body = format!(
    r#"<w:tbl><w:tblGrid><w:gridCol w:w="1440"/><w:gridCol w:w="1440"/><w:gridCol w:w="2880"/></w:tblGrid>
       <w:tr><w:trPr><w:tblHeader/></w:trPr>{}{}</w:tr>
       <w:tr>{}{}{}</w:tr>
       <w:tr>{}{}{}</w:tr></w:tbl>{}<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>"#,
    cell(r#"<w:gridSpan w:val="2"/>"#, &p("Item")),
    cell("", &p("Price")),
    cell(r#"<w:vMerge w:val="restart"/>"#, &p("Widget")),
    cell("", &[p("blue"), p("large")].concat()),
    cell("", &p("10.00")),
    cell("<w:vMerge/>", ""),
    cell("", &inner_table),
    cell("", ""),
    p("between"),
    p("second table"),
  );
  let tables = document(&body, Setup::default()).tables();
  let summary: Vec<(u32, Option<u32>, u32, u32, u32)> = tables
    .iter()
    .map(|t| {
      (
        t.index,
        t.parent,
        t.row_count,
        t.column_count,
        t.header_rows,
      )
    })
    .collect();
  // The table inside a cell follows the table that holds it.
  assert_eq!(
    summary,
    [
      (0, None, 3, 3, 1),
      (1, Some(0), 1, 2, 0),
      (2, None, 1, 1, 0)
    ]
  );
  let first = &tables[0];
  assert_eq!(first.columns, [72.0, 72.0, 144.0]);
  assert_eq!(first.merges, ["A1:B1", "A2:A3"]);
  let cells: Vec<(String, u32, u32, &str)> = first
    .cells
    .iter()
    .map(|c| {
      (
        cell_ref(c.row, c.column),
        c.col_span,
        c.row_span,
        c.text.as_str(),
      )
    })
    .collect();
  assert_eq!(
    cells,
    [
      ("A1".to_owned(), 2, 1, "Item"),
      ("C1".to_owned(), 1, 1, "Price"),
      ("A2".to_owned(), 1, 2, "Widget"),
      // A cell's paragraphs are its lines.
      ("B2".to_owned(), 1, 1, "blue\nlarge"),
      ("C2".to_owned(), 1, 1, "10.00"),
      // The cell that A2 continues into is not listed; a table in a cell is also its text.
      ("B3".to_owned(), 1, 1, "in\tside"),
      ("C3".to_owned(), 1, 1, ""),
    ]
  );
  assert_eq!(tables[1].cells[1].text, "side");
}
