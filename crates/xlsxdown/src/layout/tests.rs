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

fn layout_of(xml: &[u8]) -> SheetLayout {
  let styles = Styles::default();
  sheet_layout(&sheet_of(xml, &styles), &styles, 1, false)
}

#[test]
fn text_taller_than_its_row_grows_the_row_and_never_joins_the_row_above() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Above</t></is></c><c r="B1" t="inlineStr"><is><t>Right</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Line one&#10;Line two</t></is></c><c r="B2" t="inlineStr"><is><t>Beside</t></is></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Below</t></is></c></row></sheetData></worksheet>"##;
  let page = layout_of(xml).page;
  assert_eq!(
    page
      .lines
      .iter()
      .map(|l| l.text.as_str())
      .collect::<Vec<_>>(),
    ["Above Right", "Line one", "Line two Beside", "Below"]
  );
  // Row 2 grows from 15 pt to two Calibri 11 lines (2 × 13.2 pt), like Excel's auto-fit.
  assert!(
    (page.height - (15.0 + 26.4 + 15.0)).abs() < 1e-9,
    "{}",
    page.height
  );
  assert!(
    page.words.iter().any(|w| w.text == "two"),
    "no word is dropped"
  );
  assert!(
    page
      .words
      .iter()
      .all(|w| w.y0 >= 0.0 && w.y1 <= page.height)
  );
  assert!(
    page.warnings.iter().any(|w| w.contains("grown")),
    "{:?}",
    page.warnings
  );
}

#[test]
fn a_reference_past_the_grid_does_not_grow_the_page() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="A999999999"><v>2</v></c></row></sheetData></worksheet>"##;
  let page = layout_of(xml).page;
  assert_eq!((page.width, page.height), (48.0, 15.0));
  assert_eq!(page.words.len(), 1);
  assert!(
    page.warnings.iter().any(|w| w.contains("malformed")),
    "{:?}",
    page.warnings
  );
}

#[test]
fn every_word_names_the_cell_it_was_laid_out_from() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Invoice total</t></is></c><c r="B1"><v>42</v></c></row><row r="2"><c r="B2" t="inlineStr"><is><t>Paid</t></is></c></row></sheetData></worksheet>"##;
  let layout = layout_of(xml);
  assert_eq!(layout.sheet, "S");
  assert_eq!(layout.page.source, TextSource::Native);
  let by_cell: Vec<(&str, &str)> = layout
    .page
    .words
    .iter()
    .zip(&layout.cells)
    .map(|(word, cell)| (word.text.as_str(), cell.as_str()))
    .collect();
  assert_eq!(
    by_cell,
    [
      ("Invoice", "A1"),
      ("total", "A1"),
      ("42", "B1"),
      ("Paid", "B2")
    ]
  );
}

#[test]
fn an_empty_sheet_is_a_page_with_no_size_and_a_warning() {
  let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"##;
  let layout = layout_of(xml);
  assert_eq!((layout.page.width, layout.page.height), (0.0, 0.0));
  assert!(layout.page.words.is_empty() && layout.cells.is_empty());
  assert!(
    layout
      .page
      .warnings
      .iter()
      .any(|w| w.contains("no content"))
  );
}
