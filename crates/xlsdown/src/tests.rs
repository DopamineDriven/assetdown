use xlsxdown::{CellRange, CellValue, Horizontal, Sheet, SpreadsheetError, Vertical, Workbook};

use super::*;
use crate::testing::*;

fn opened(book: Book) -> Workbook {
  open(&book.file()).unwrap()
}

fn value(sheet: &Sheet, row: u32, col: u32) -> &CellValue {
  &sheet
    .cells
    .get(&(row, col))
    .unwrap_or_else(|| panic!("no cell at row {row}, column {col}"))
    .value
}

fn shown(sheet: &Sheet, row: u32, col: u32) -> &str {
  &sheet.cells[&(row, col)].text
}

fn unsupported(file: &[u8]) -> String {
  match open(file) {
    Err(SpreadsheetError::UnsupportedFormat(detail)) => detail,
    other => panic!("{other:?}"),
  }
}

#[test]
fn numbers_are_read_from_each_of_the_ways_a_sheet_keeps_them() {
  // The short way keeps a whole number or the top of a double, either one a hundredth of what
  // the cell holds when the lowest bit is set.
  let whole = |n: i32| (n as u32) << 2 | 2;
  let hundredths = |n: i32| (n as u32) << 2 | 3;
  let double = u32::try_from(1.5f64.to_bits() >> 32).unwrap();
  let book = Book::new().sheet(
    "Costs",
    &[
      number(0, 0, 0, 427.5),
      rk(0, 1, 0, whole(-7)),
      rk(0, 2, 0, hundredths(1999)),
      rk(0, 3, 0, double),
      rk(0, 4, 0, double | 1),
      mulrk(1, 2, &[(0, whole(1)), (0, whole(2)), (0, hundredths(250))]),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  let numbers: Vec<f64> = sheet
    .cells
    .values()
    .map(|cell| match cell.value {
      CellValue::Number(n) => n,
      ref other => panic!("{other:?}"),
    })
    .collect();
  assert_eq!(numbers, [427.5, -7.0, 19.99, 1.5, 0.015, 1.0, 2.0, 2.5]);
  assert_eq!(
    sheet.cells.keys().copied().collect::<Vec<_>>(),
    [
      (0, 0),
      (0, 1),
      (0, 2),
      (0, 3),
      (0, 4),
      (1, 2),
      (1, 3),
      (1, 4)
    ]
  );
  assert_eq!(shown(sheet, 0, 2), "19.99");
  assert_eq!(sheet.cells[&(0, 0)].raw, "427.5");
  assert!(
    sheet
      .cells
      .values()
      .all(|cell| cell.cached && cell.formula.is_none())
  );
}

#[test]
fn text_booleans_and_errors_are_read_and_a_cell_with_nothing_in_it_is_no_cell() {
  let book = Book::new()
    .global(sst(&["Total", "Zo\u{eb}", "\u{65e5}\u{672c}"]))
    .sheet(
      "S",
      &[
        label_sst(0, 0, 0, 0),
        label_sst(0, 1, 0, 1),
        label_sst(0, 2, 0, 2),
        label(1, 0, 0, "kept in the cell"),
        boolean(2, 0, 0, true),
        boolean(2, 1, 0, false),
        error(2, 2, 0, 0x07),
        error(2, 3, 0, 0x2A),
        blank(3, 0, 0),
      ],
    );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(value(sheet, 0, 0), &CellValue::Text("Total".to_string()));
  assert_eq!(shown(sheet, 0, 1), "Zo\u{eb}");
  assert_eq!(shown(sheet, 0, 2), "\u{65e5}\u{672c}");
  assert_eq!(shown(sheet, 1, 0), "kept in the cell");
  assert_eq!(value(sheet, 2, 0), &CellValue::Boolean(true));
  assert_eq!(shown(sheet, 2, 1), "FALSE");
  assert_eq!(value(sheet, 2, 2), &CellValue::Error("#DIV/0!".to_string()));
  assert_eq!(shown(sheet, 2, 3), "#N/A");
  assert_eq!(sheet.cells.len(), 8, "the blank cell is not one");
}

#[test]
fn a_shared_string_index_past_the_table_is_left_out_and_said_to_be() {
  let book = Book::new()
    .global(sst(&["only"]))
    .sheet("S", &[label_sst(0, 0, 0, 0), label_sst(0, 1, 0, 5)]);
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(sheet.cells.len(), 1);
  assert_eq!(
    sheet.warnings,
    ["B1: shared string index 5 is out of range; skipped"]
  );
}

#[test]
fn a_shared_string_keeps_only_its_characters_whatever_else_it_carries() {
  // A string with two runs of formatting after it, and one with a note on how it is read.
  let mut data = [2u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
  data.extend(5u16.to_le_bytes());
  data.push(0x08);
  data.extend(2u16.to_le_bytes());
  data.extend(b"Hello");
  data.extend([0xAA; 8]);
  data.extend(2u16.to_le_bytes());
  data.push(0x04 | 0x01);
  data.extend(3u32.to_le_bytes());
  data.extend("\u{65e5}\u{672c}".encode_utf16().flat_map(u16::to_le_bytes));
  data.extend([0xBB; 3]);
  let book = Book::new()
    .global(record(0x00FC, &data))
    .sheet("S", &[label_sst(0, 0, 0, 0), label_sst(0, 1, 0, 1)]);
  let workbook = opened(book);
  assert_eq!(shown(&workbook.sheets[0], 0, 0), "Hello");
  assert_eq!(shown(&workbook.sheets[0], 0, 1), "\u{65e5}\u{672c}");
}

#[test]
fn a_shared_string_that_runs_on_into_the_next_record_may_change_how_it_is_kept() {
  // Eleven characters: five of them one byte each, and after the break six of two bytes each.
  let mut data = [2u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
  data.extend(text("ab"));
  data.extend(11u16.to_le_bytes());
  data.push(0);
  data.extend([b'h', 0xE9, b'l', b'l', b'o']);
  let mut more = vec![1u8];
  more.extend(" w\u{f6}rld".encode_utf16().flat_map(u16::to_le_bytes));
  let book = Book::new()
    .global(record(0x00FC, &data))
    .global(record(0x003C, &more))
    .sheet("S", &[label_sst(0, 0, 0, 0), label_sst(0, 1, 0, 1)]);
  let workbook = opened(book);
  assert_eq!(shown(&workbook.sheets[0], 0, 0), "ab");
  assert_eq!(shown(&workbook.sheets[0], 0, 1), "h\u{e9}llo w\u{f6}rld");
}

#[test]
fn a_number_is_typed_and_shown_through_the_format_its_style_selects() {
  let book = Book::new()
    .global(format(164, "0.0\" kg\""))
    .global(xf(0, 0, 0, 0))
    .global(xf(0, 14, 0, 0))
    .global(xf(0, 4, 0, 0))
    .global(xf(0, 164, 0, 0))
    .sheet(
      "S",
      &[
        number(0, 0, 1, 46349.0),
        number(0, 1, 2, 1234.5),
        number(0, 2, 3, 2.25),
        number(0, 3, 0, 46349.0),
      ],
    );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(
    value(sheet, 0, 0),
    &CellValue::Date("2026-11-23".to_string())
  );
  assert_eq!(shown(sheet, 0, 0), "11/23/2026");
  assert_eq!(sheet.cells[&(0, 0)].raw, "46349");
  assert_eq!(shown(sheet, 0, 1), "1,234.50");
  assert_eq!(shown(sheet, 0, 2), "2.3 kg");
  assert_eq!(sheet.cells[&(0, 2)].format.as_deref(), Some("0.0\" kg\""));
  assert_eq!(value(sheet, 0, 3), &CellValue::Number(46349.0));
  assert!(!workbook.date1904);
}

#[test]
fn dates_count_from_1904_when_the_workbook_says_so() {
  let book = Book::new()
    .global(record(0x0022, &1u16.to_le_bytes()))
    .global(xf(0, 14, 0, 0))
    .sheet("S", &[number(0, 0, 0, 0.0)]);
  let workbook = opened(book);
  assert!(workbook.date1904);
  assert_eq!(
    value(&workbook.sheets[0], 0, 0),
    &CellValue::Date("1904-01-01".to_string())
  );
}

#[test]
fn a_formula_gives_the_value_it_was_left_with_and_says_it_is_one() {
  let sum = [0x1E, 1, 0, 0x1E, 2, 0, 0x03];
  let book = Book::new().sheet(
    "S",
    &[
      formula(0, 0, 0, 3.0f64.to_le_bytes(), &sum),
      formula(0, 1, 0, result_of(0, 0), &sum),
      // What defines a shared formula stands between the formula and the text it gave.
      record(0x04BC, &[0u8; 10]),
      string("joined up"),
      formula(0, 2, 0, result_of(1, 1), &sum),
      formula(0, 3, 0, result_of(2, 0x17), &sum),
      formula(0, 4, 0, result_of(3, 0), &sum),
      number(1, 0, 0, 9.0),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(value(sheet, 0, 0), &CellValue::Number(3.0));
  assert_eq!(
    value(sheet, 0, 1),
    &CellValue::Text("joined up".to_string())
  );
  assert_eq!(value(sheet, 0, 2), &CellValue::Boolean(true));
  assert_eq!(value(sheet, 0, 3), &CellValue::Error("#REF!".to_string()));
  assert_eq!(value(sheet, 0, 4), &CellValue::Text(String::new()));
  for col in 0..5 {
    let cell = &sheet.cells[&(0, col)];
    assert_eq!(cell.formula.as_deref(), Some("1+2"), "column {col}");
    assert!(cell.cached);
  }
  assert_eq!(sheet.cells[&(1, 0)].formula, None);
  assert_eq!(workbook.metadata().formula_count, 5);
}

#[test]
fn a_formula_whose_text_never_comes_is_a_cell_with_no_value() {
  let book = Book::new().sheet(
    "S",
    &[
      formula(0, 0, 0, result_of(0, 0), &[0x1E, 1, 0]),
      number(0, 1, 0, 2.0),
    ],
  );
  let workbook = opened(book);
  let cell = &workbook.sheets[0].cells[&(0, 0)];
  assert_eq!(cell.value, CellValue::Blank);
  assert_eq!((cell.formula.as_deref(), cell.cached), (Some("1"), false));
  assert_eq!(value(&workbook.sheets[0], 0, 1), &CellValue::Number(2.0));
}

/// The tokens of the formula the tests share: the cell two columns to the left of the one
/// that uses it, times the cell one column to the left.
fn two_left_times_one_left() -> Vec<u8> {
  let step = |columns: u8| [0x4C, 0, 0, columns, 0xC0];
  [&step(0xFE)[..], &step(0xFF), &[0x05]].concat()
}

#[test]
fn each_cell_that_shares_a_formula_has_it_written_for_itself_and_names_the_cell_that_holds_it() {
  let book = Book::new().sheet(
    "S",
    &[
      formula(1, 2, 0, 6.0f64.to_le_bytes(), &follows(1, 2)),
      shared_formula((1, 3), (2, 2), &two_left_times_one_left()),
      formula(2, 2, 0, 12.0f64.to_le_bytes(), &follows(1, 2)),
      formula(3, 2, 0, 20.0f64.to_le_bytes(), &follows(1, 2)),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  let found: Vec<_> = sheet
    .cells
    .values()
    .map(|cell| (cell.formula.as_deref(), cell.shared_master))
    .collect();
  assert_eq!(
    found,
    [
      (Some("A2*B2"), None),
      (Some("A3*B3"), Some((1, 2))),
      (Some("A4*B4"), Some((1, 2)))
    ]
  );
  assert_eq!(value(sheet, 3, 2), &CellValue::Number(20.0));
  assert_eq!(workbook.metadata().formula_count, 3);
  assert!(sheet.warnings.is_empty(), "{:?}", sheet.warnings);
}

#[test]
fn a_formula_that_fills_a_range_of_cells_together_is_written_on_each_of_them() {
  // A1:A2 times two, in D1 and D2.
  let tokens = [
    &[0x25, 0, 0, 1, 0, 0x00, 0xC0, 0x00, 0xC0][..],
    &[0x1E, 2, 0, 0x05],
  ]
  .concat();
  let book = Book::new().sheet(
    "S",
    &[
      formula(0, 3, 0, 2.0f64.to_le_bytes(), &follows(0, 3)),
      array_formula((0, 1), (3, 3), &tokens),
      formula(1, 3, 0, 4.0f64.to_le_bytes(), &follows(0, 3)),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  let first = &sheet.cells[&(0, 3)];
  assert_eq!(
    (first.formula.as_deref(), first.shared_master),
    (Some("A1:A2*2"), None)
  );
  // The same formula, and not one moved to the cell: the range is filled as one.
  let second = &sheet.cells[&(1, 3)];
  assert_eq!(
    (second.formula.as_deref(), second.shared_master),
    (Some("A1:A2*2"), Some((0, 3)))
  );
}

#[test]
fn a_formula_names_the_sheets_the_names_and_the_functions_the_workbook_lists() {
  let rate = [&[0x23][..], &1u32.to_le_bytes()].concat();
  // The first cell of the second sheet, times a name, and a function an add-in gives.
  let other_sheet = [&[0x3A][..], &1u16.to_le_bytes(), &[0, 0, 0x00, 0xC0]].concat();
  let month_end = [
    &[0x39][..],
    &2u16.to_le_bytes(),
    &1u32.to_le_bytes(),
    &[0x44, 0, 0, 0x00, 0xC0],
    &[0x1E, 1, 0],
    &[0x42, 3, 0xFF, 0x00],
  ]
  .concat();
  let book = Book::new()
    .global(own_book(2))
    .global(add_in_book("EOMONTH"))
    .global(sheet_spans(&[(0, 0, 0), (0, 1, 1), (1, -2, -2)]))
    .global(name(None, "Rate", 0, &[0x1E, 5, 0]))
    .sheet(
      "Costs",
      &[
        formula(
          0,
          0,
          0,
          10.0f64.to_le_bytes(),
          &[other_sheet, rate, vec![0x05]].concat(),
        ),
        formula(1, 0, 0, 46357.0f64.to_le_bytes(), &month_end),
      ],
    )
    .sheet("Unit prices", &[number(0, 0, 0, 2.0)]);
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(
    sheet.cells[&(0, 0)].formula.as_deref(),
    Some("'Unit prices'!A1*Rate")
  );
  assert_eq!(
    sheet.cells[&(1, 0)].formula.as_deref(),
    Some("EOMONTH(A1,1)")
  );
  assert!(sheet.warnings.is_empty(), "{:?}", sheet.warnings);
}

#[test]
fn a_formula_that_cannot_be_written_out_has_its_text_empty_and_the_sheet_says_how_many() {
  let book = Book::new().sheet(
    "S",
    &[
      // A token this release does not know, and a table of what-ifs.
      formula(0, 0, 0, 1.0f64.to_le_bytes(), &[0x1A, 0, 0]),
      formula(0, 1, 0, 2.0f64.to_le_bytes(), &[0x02, 0, 0, 1, 0]),
      formula(0, 2, 0, 3.0f64.to_le_bytes(), &[0x1E, 3, 0]),
      // A formula that says it is shared, with nothing that defines it, and a cell after it.
      formula(1, 0, 0, 4.0f64.to_le_bytes(), &follows(1, 0)),
      formula(2, 0, 0, 5.0f64.to_le_bytes(), &follows(1, 0)),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  let text = |row: u32, col: u32| sheet.cells[&(row, col)].formula.as_deref();
  assert_eq!(
    (text(0, 0), text(0, 1), text(0, 2)),
    (Some(""), Some(""), Some("3"))
  );
  assert_eq!(text(1, 0), Some(""));
  // A cell that follows it names it, and has no text.
  let follower = &sheet.cells[&(2, 0)];
  assert_eq!(
    (follower.formula.as_deref(), follower.shared_master),
    (None, Some((1, 0)))
  );
  // The values are there whatever the text.
  assert_eq!(value(sheet, 0, 0), &CellValue::Number(1.0));
  assert_eq!(
    sheet.warnings,
    [
      "3 formula(s) hold what this release does not write out; their text is empty",
      "1 cell(s) follow a shared formula that was not written out for them; they name their master only"
    ]
  );
  assert_eq!(workbook.metadata().formula_count, 5);
}

/// The tokens of a formula that joins `count` pieces of text of 255 characters each.
fn long_text(count: usize) -> Vec<u8> {
  let piece: Vec<u8> = [&[0x17, 255, 0][..], &[b'x'; 255]].concat();
  let mut tokens = piece.clone();
  for _ in 1..count {
    tokens.extend(&piece);
    tokens.push(0x08);
  }
  tokens
}

#[test]
fn a_shared_formula_longer_than_a_formula_may_be_is_not_written_for_the_cells_that_follow_it() {
  // Thirty-three pieces come to more than 8,192 characters.
  let book = Book::new().sheet(
    "S",
    &[
      formula(0, 0, 0, result_of(3, 0), &follows(0, 0)),
      shared_formula((0, 1), (0, 0), &long_text(33)),
      formula(1, 0, 0, result_of(3, 0), &follows(0, 0)),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert!(
    sheet.cells[&(0, 0)]
      .formula
      .as_ref()
      .is_some_and(|text| text.len() > 8192)
  );
  let follower = &sheet.cells[&(1, 0)];
  assert_eq!(
    (follower.formula.as_deref(), follower.shared_master),
    (None, Some((0, 0)))
  );
  assert_eq!(
    sheet.warnings,
    [
      "1 cell(s) follow a shared formula that was not written out for them; they name their master only"
    ]
  );
}

#[test]
fn shared_formulas_are_written_out_until_the_workbook_s_budget_for_them_is_spent() {
  // Each follower's text is 2,061 bytes, and the budget is 64 MiB: 32,561 of them fit.
  let tokens = long_text(8);
  let followers = 33_000u16;
  let mut records = vec![
    formula(0, 0, 0, result_of(3, 0), &follows(0, 0)),
    shared_formula((0, followers), (0, 0), &tokens),
  ];
  records.extend((1..=followers).map(|row| formula(row, 0, 0, result_of(3, 0), &follows(0, 0))));
  let workbook = opened(Book::new().sheet("S", &records));
  let sheet = &workbook.sheets[0];
  let each = 8 * 257 + 7;
  let fit = xlsxdown::MAX_EXPANDED_FORMULA_BYTES / each;
  let written = sheet
    .cells
    .values()
    .filter(|cell| cell.shared_master.is_some() && cell.formula.is_some())
    .count();
  assert_eq!(written, fit);
  assert_eq!(
    sheet.warnings,
    [format!(
      "{} cell(s) follow a shared formula that was not written out for them; they name their master only",
      usize::from(followers) - fit
    )]
  );
}

#[test]
fn sheets_are_listed_in_order_with_how_each_is_shown() {
  let book = Book::new()
    .sheet("First", &[number(0, 0, 0, 1.0)])
    .sheet_of("Kept back", 1, WORKSHEET, &[number(0, 0, 0, 2.0)])
    .sheet_of("Deep", 2, WORKSHEET, &[])
    .sheet_of("Plot", 0, CHART, &[number(0, 0, 0, 3.0)])
    .sheet("\u{6708}", &[number(4, 4, 0, 5.0)]);
  let workbook = opened(book);
  let listed: Vec<(&str, u32, bool, usize)> = workbook
    .sheets
    .iter()
    .map(|sheet| {
      (
        sheet.name.as_str(),
        sheet.index,
        sheet.hidden,
        sheet.cells.len(),
      )
    })
    .collect();
  assert_eq!(
    listed,
    [
      ("First", 0, false, 1),
      ("Kept back", 1, true, 1),
      ("Deep", 2, true, 0),
      // A sheet that is only a chart has no cells: the numbers in it are the chart's.
      ("Plot", 3, false, 0),
      ("\u{6708}", 4, false, 1)
    ]
  );
  assert_eq!(workbook.sheets[3].charts, 1);
  assert_eq!(value(&workbook.sheets[4], 4, 4), &CellValue::Number(5.0));
}

#[test]
fn a_chart_s_own_records_inside_a_sheet_are_not_the_sheet_s_cells() {
  let book = Book::new().sheet(
    "S",
    &[
      number(0, 0, 0, 1.0),
      object(5),
      bof(CHART),
      number(0, 0, 0, 99.0),
      number(7, 7, 0, 98.0),
      eof(),
      number(1, 0, 0, 2.0),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(
    sheet.cells.keys().copied().collect::<Vec<_>>(),
    [(0, 0), (1, 0)]
  );
  assert_eq!(value(sheet, 0, 0), &CellValue::Number(1.0));
  assert_eq!(sheet.charts, 1);
}

#[test]
fn the_extent_the_columns_the_rows_and_the_merged_ranges_are_read() {
  let book = Book::new().sheet(
    "S",
    &[
      dimensions((0, 12), (0, 6)),
      record(0x0225, &[0, 0, 0x2C, 0x01]),
      colinfo(1, 2, 20 * 256, false),
      colinfo(4, 4, 256, true),
      row(0, 600, false),
      row(1, 0x8000 | 255, false),
      row(3, 300, true),
      number(0, 0, 0, 1.0),
      number(5, 1, 0, 2.0),
      merged(&[(0, 1, 0, 3), (5, 6, 1, 2)]),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(sheet.dimension, CellRange::parse("A1:F12"));
  assert_eq!(sheet.default_row_height, 15.0);
  let cols: Vec<(u32, u32, f64, bool)> = sheet
    .cols
    .iter()
    .map(|col| (col.min, col.max, col.width, col.hidden))
    .collect();
  assert_eq!(cols, [(2, 3, 20.0, false), (5, 5, 1.0, true)]);
  assert!(sheet.col_hidden(4) && !sheet.col_hidden(3));
  assert_eq!(sheet.rows[&0].height, Some(30.0));
  // A row left at the default height states none.
  assert_eq!(sheet.rows[&1].height, None);
  assert_eq!(
    (sheet.rows[&3].height, sheet.rows[&3].hidden),
    (Some(15.0), true)
  );
  assert_eq!(
    sheet.merges,
    [
      CellRange::parse("A1:D2").unwrap(),
      CellRange::parse("B6:C7").unwrap()
    ]
  );
  assert_eq!(sheet.cells[&(0, 0)].merge, CellRange::parse("A1:D2"));
  assert_eq!(sheet.cells[&(5, 1)].merge, CellRange::parse("B6:C7"));
  assert_eq!(workbook.metadata().merged_range_count, 2);
}

#[test]
fn a_sheet_s_default_column_width_is_what_it_states() {
  let width =
    |records: &[Vec<u8>]| opened(Book::new().sheet("S", records)).sheets[0].default_col_width;
  // Eight characters is what a sheet has when it says nothing.
  assert_eq!(width(&[]), None);
  assert_eq!(width(&[record(0x0055, &8u16.to_le_bytes())]), None);
  // A count of characters has the room around them added, as a workbook of 2007 states it.
  assert_eq!(
    width(&[record(0x0055, &10u16.to_le_bytes())]),
    Some(11.140625)
  );
  // A width stated outright is in 256ths of a character, and is taken before the count.
  assert_eq!(
    width(&[
      record(0x0055, &10u16.to_le_bytes()),
      record(0x0099, &(12u16 * 256 + 128).to_le_bytes())
    ]),
    Some(12.5)
  );
}

#[test]
fn fonts_and_alignment_are_read_into_the_styles() {
  let book = Book::new()
    .global(font("Arial", 200, false, false))
    .global(font("Arial", 200, true, false))
    .global(font("Times New Roman", 240, false, true))
    .global(font("Arial", 200, false, false))
    // The file numbers its fonts with no fourth: the one after the first four is the fifth.
    .global(font("Courier New", 180, true, true))
    .global(xf(0, 0, 0, 0))
    .global(xf(1, 0, 0x02 | 0x08 | 0x10, 3))
    .global(xf(5, 0, 0x03, 0))
    .global(xf(2, 0, 0x06 | 0x20, 0))
    .sheet("S", &[number(0, 0, 1, 1.0)]);
  let workbook = opened(book);
  let styles = &workbook.styles;
  assert_eq!(styles.fonts.len(), 5);
  assert_eq!(
    (styles.normal_font.name.as_str(), styles.normal_font.size),
    ("Arial", 10.0)
  );
  let centered = &styles.xfs[1];
  assert_eq!(
    (
      centered.horizontal,
      centered.vertical,
      centered.wrap,
      centered.indent
    ),
    (Horizontal::Center, Vertical::Center, true, 3)
  );
  assert!(styles.fonts[centered.font_id].bold);
  let right = &styles.xfs[2];
  assert_eq!(
    (right.horizontal, right.vertical),
    (Horizontal::Right, Vertical::Top)
  );
  let courier = &styles.fonts[right.font_id];
  assert_eq!(
    (
      courier.name.as_str(),
      courier.size,
      courier.bold,
      courier.italic
    ),
    ("Courier New", 9.0, true, true)
  );
  // Centered across a selection is centered; the bottom is where a cell's text sits.
  let across = &styles.xfs[3];
  assert_eq!(
    (across.horizontal, across.vertical),
    (Horizontal::Center, Vertical::Bottom)
  );
  assert_eq!(styles.fonts[across.font_id].name, "Times New Roman");
  assert_eq!(workbook.sheets[0].cells[&(0, 0)].style, 1);
}

#[test]
fn a_print_area_is_read_for_its_sheet_and_the_names_a_workbook_defines_are_counted() {
  let both = [area((0, 4), (0, 2)), area((9, 12), (0, 2)), vec![0x10]].concat();
  let book = Book::new()
    .global(name(Some(0x06), "", 2, &area((0, 11), (0, 5))))
    .global(name(Some(0x06), "", 3, &both))
    .global(name(None, "Rate", 0, &[0x1E, 5, 0]))
    .global(name(None, "Total", 1, &area((1, 1), (1, 1))))
    .global(name(Some(0x07), "", 1, &area((0, 0), (0, 5))))
    .sheet("One", &[])
    .sheet("Two", &[])
    .sheet("Three", &[]);
  let workbook = opened(book);
  assert_eq!(workbook.sheets[0].print_area, None);
  assert_eq!(workbook.sheets[1].print_area, CellRange::parse("A1:F12"));
  // A print area of more than one range is no print area.
  assert_eq!(workbook.sheets[2].print_area, None);
  assert_eq!(workbook.defined_names, 2);
}

#[test]
fn what_else_a_sheet_holds_is_counted() {
  let book = Book::new().sheet(
    "S",
    &[
      number(0, 0, 0, 1.0),
      record(0x00EC, &[0u8; 8]),
      object(8),
      object(8),
      object(25),
      record(0x01B8, &[0u8; 8]),
      record(0x01B8, &[0u8; 8]),
      record(0x001C, &[0u8; 12]),
      record(0x00B0, &[0u8; 4]),
    ],
  );
  let workbook = opened(book);
  let sheet = &workbook.sheets[0];
  assert_eq!(
    (
      sheet.pictures,
      sheet.charts,
      sheet.links,
      sheet.comments,
      sheet.pivot_tables,
      sheet.drawings
    ),
    (2, 0, 2, 1, 1, 1)
  );
  let meta = workbook.metadata();
  assert_eq!(
    (meta.image_count, meta.link_count, meta.comment_count),
    (2, 2, 1)
  );
}

#[test]
fn a_sheet_whose_records_stop_early_keeps_what_came_before_and_says_so() {
  let book = Book::new()
    .sheet("Whole", &[number(0, 0, 0, 1.0)])
    .sheet("Cut", &[number(0, 0, 0, 2.0), number(1, 0, 0, 3.0)]);
  let mut stream = book.stream();
  // The last record is the end of the sheet, and the one before it the second number: both go,
  // with half of the number's.
  stream.truncate(stream.len() - 4 - 9);
  let file = cfbdown::compound(&[("Workbook", &stream)]).unwrap();
  let workbook = open(&file).unwrap();
  assert_eq!(workbook.sheets[0].cells.len(), 1);
  assert!(workbook.sheets[0].warnings.is_empty());
  let cut = &workbook.sheets[1];
  assert_eq!(value(cut, 0, 0), &CellValue::Number(2.0));
  assert_eq!(cut.cells.len(), 1);
  assert_eq!(cut.warnings, ["the sheet's records stop before its end"]);
}

#[test]
fn a_sheet_that_is_not_where_the_workbook_says_has_no_cells_and_says_so() {
  let mut stream = Book::new().sheet("S", &[number(0, 0, 0, 1.0)]).stream();
  // Where the sheet starts is the first thing its listing states, after the workbook's opening.
  let at = bof(GLOBALS).len() + 4;
  stream[at..at + 4].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
  let file = cfbdown::compound(&[("Workbook", &stream)]).unwrap();
  let workbook = open(&file).unwrap();
  assert_eq!(workbook.sheets[0].name, "S");
  assert!(workbook.sheets[0].cells.is_empty());
  assert_eq!(
    workbook.sheets[0].warnings,
    ["the sheet's records are not where the workbook says they are"]
  );
}

#[test]
fn a_workbook_saved_with_a_password_is_refused_and_named() {
  let book = Book::new()
    .global(record(0x002F, &[0u8; 6]))
    .sheet("S", &[number(0, 0, 0, 1.0)]);
  assert_eq!(unsupported(&book.file()), "an encrypted workbook");
  assert_eq!(
    open(&book.file()).unwrap_err().to_string(),
    "unsupported format: an encrypted workbook"
  );
}

#[test]
fn an_older_workbook_is_refused_and_named_for_what_it_is() {
  let older = Book::new().version(0x0500).sheet("S", &[]);
  // Excel 5 and 95 name the stream `Book`.
  let file = cfbdown::compound(&[("Book", &older.stream())]).unwrap();
  assert_eq!(unsupported(&file), "an Excel 5 or 95 workbook");
  // The same records in a stream named as a later workbook's are still what they are.
  assert_eq!(unsupported(&older.file()), "an Excel 5 or 95 workbook");
  // Before that a worksheet was a file of records with nothing around them.
  for opening in [
    [0x09, 0x00, 0x04, 0x00],
    [0x09, 0x02, 0x06, 0x00],
    [0x09, 0x04, 0x06, 0x00],
  ] {
    let mut file = opening.to_vec();
    file.extend([0u8; 6]);
    assert_eq!(unsupported(&file), "an Excel 2, 3, or 4 worksheet");
  }
}

#[test]
fn what_is_not_a_workbook_says_what_is_wrong_with_it() {
  assert_eq!(
    open(b"PK\x03\x04 a zip").unwrap_err().to_string(),
    "malformed workbook: not a compound file"
  );
  let cut = &Book::new().sheet("S", &[]).file()[..600];
  assert_eq!(
    open(cut).unwrap_err().to_string(),
    "malformed workbook: malformed compound file: its allocation table cannot be read"
  );
  let word = cfbdown::compound(&[("WordDocument", b"text"), ("1Table", b"t")]).unwrap();
  assert_eq!(unsupported(&word), "a compound file that holds no workbook");
  let empty = cfbdown::compound(&[("Workbook", b"")]).unwrap();
  assert_eq!(
    open(&empty).unwrap_err().to_string(),
    "malformed workbook: the workbook's records do not begin as a workbook's do"
  );
}

#[test]
fn a_workbook_with_a_storage_of_macros_says_it_carries_them() {
  let stream = Book::new().sheet("S", &[]).stream();
  let plain = cfbdown::compound(&[("Workbook", &stream)]).unwrap();
  assert!(!open(&plain).unwrap().has_macros);
  let with =
    cfbdown::compound(&[("Workbook", &stream), ("_VBA_PROJECT_CUR/VBA/dir", b"d")]).unwrap();
  let workbook = open(&with).unwrap();
  assert!(workbook.has_macros && !workbook.is_template);
}

#[test]
fn a_workbook_with_more_cells_than_the_limit_is_refused() {
  // 256 numbers to a record, and rows of them until the limit is passed.
  let values = vec![(0u16, 2u32 << 2 | 2); 256];
  let rows = xlsxdown::MAX_CELLS / 256 + 1;
  let records: Vec<Vec<u8>> = (0..rows)
    .map(|row| mulrk(u16::try_from(row % 65536).unwrap(), 0, &values))
    .collect();
  // No sheet has more than 65,536 rows, so the cells are spread over two.
  let (first, second) = records.split_at(rows / 2);
  let book = Book::new().sheet("A", first).sheet("B", second);
  assert_eq!(
    open(&book.file()).unwrap_err().to_string(),
    format!(
      "limit exceeded: the workbook has more than {} cells",
      xlsxdown::MAX_CELLS
    )
  );
}

#[test]
fn what_the_workbook_says_about_itself_is_read_from_its_property_streams() {
  let stream = Book::new().sheet("S", &[number(0, 0, 0, 1.0)]).stream();
  let first = properties(&[
    (2, Ok("Order form")),
    (3, Ok("Parts")),
    (4, Ok("A. Clerk")),
    (5, Ok("orders; parts")),
    (6, Ok("For the spring")),
    (8, Ok("B. Checker")),
    (9, Ok("7")),
    (12, Err(134_392_086_000_000_000)),
    (13, Err(134_392_122_000_000_000)),
    (18, Ok("Microsoft Excel")),
  ]);
  let second = properties(&[
    (2, Ok("Forms")),
    (15, Ok("Acme Tooling")),
    (28, Ok("en-US")),
  ]);
  let file = cfbdown::compound(&[
    ("Workbook", &stream),
    ("\u{5}SummaryInformation", &first),
    ("\u{5}DocumentSummaryInformation", &second),
  ])
  .unwrap();
  let props = open(&file).unwrap().props;
  let text = |value: &str| Some(value.to_string());
  assert_eq!(props.title, text("Order form"));
  assert_eq!(props.subject, text("Parts"));
  // The author is the creator, as a workbook of 2007 names them.
  assert_eq!(props.creator, text("A. Clerk"));
  assert_eq!(props.keywords, text("orders; parts"));
  assert_eq!(props.description, text("For the spring"));
  assert_eq!(props.last_modified_by, text("B. Checker"));
  assert_eq!(props.revision, text("7"));
  assert_eq!(props.created, text("2026-11-15T09:30:00Z"));
  assert_eq!(props.modified, text("2026-11-15T10:30:00Z"));
  assert_eq!(props.application, text("Microsoft Excel"));
  assert_eq!(props.category, text("Forms"));
  assert_eq!(props.company, text("Acme Tooling"));
  assert_eq!(props.language, text("en-US"));

  // A workbook with no property streams states nothing about itself.
  let bare = open(&Book::new().sheet("S", &[]).file()).unwrap().props;
  assert_eq!(bare, xlsxdown::DocProps::default());
}

#[test]
fn the_tables_of_a_sheet_are_counted() {
  let book = Book::new()
    .sheet(
      "S",
      &[
        number(0, 0, 0, 1.0),
        // A table as Excel 2003 keeps one, and as a later Excel keeps one in this format.
        record(0x0872, &[0u8; 30]),
        record(0x0878, &[0u8; 30]),
      ],
    )
    .sheet("T", &[number(0, 0, 0, 2.0)]);
  let workbook = opened(book);
  assert_eq!(
    (workbook.sheets[0].tables, workbook.sheets[1].tables),
    (2, 0)
  );
  assert_eq!(workbook.metadata().table_count, 2);
}
