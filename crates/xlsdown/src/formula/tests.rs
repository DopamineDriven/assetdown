use super::*;

fn links() -> Links {
  Links {
    sheets: vec![
      "Costs".to_string(),
      "My Sheet".to_string(),
      "Q4".to_string(),
      "It's".to_string(),
      "Last".to_string(),
    ],
    names: vec!["Rate".to_string(), "Print_Area".to_string()],
    // Which sheets a reference names: by the book they are in, and the first and the last.
    sheet_spans: vec![
      SheetSpan {
        book: 0,
        first: 0,
        last: 0,
      },
      SheetSpan {
        book: 0,
        first: 1,
        last: 1,
      },
      SheetSpan {
        book: 0,
        first: 0,
        last: 4,
      },
      SheetSpan {
        book: 0,
        first: 2,
        last: 2,
      },
      SheetSpan {
        book: 0,
        first: 3,
        last: 3,
      },
      SheetSpan {
        book: 0,
        first: -1,
        last: -1,
      },
      SheetSpan {
        book: 1,
        first: -2,
        last: -2,
      },
      SheetSpan {
        book: 2,
        first: 1,
        last: 1,
      },
    ],
    books: vec![
      Book::Own,
      Book::AddIn {
        names: vec!["EOMONTH".to_string()],
      },
      Book::Other {
        sheets: vec!["Jan".to_string(), "Feb".to_string()],
        names: vec!["Budget".to_string()],
      },
    ],
  }
}

fn written(tokens: &[u8]) -> Option<String> {
  text(tokens, &[], (0, 0), &links())
}

fn shown(tokens: &[u8]) -> String {
  written(tokens).unwrap_or_else(|| panic!("not written out: {tokens:02X?}"))
}

/// A whole number of up to sixteen bits.
fn int(value: u16) -> Vec<u8> {
  [&[0x1E][..], &value.to_le_bytes()].concat()
}

/// A cell, with whether its row and its column move when the formula is copied.
fn cell(row: u16, col: u8, row_moves: bool, col_moves: bool) -> Vec<u8> {
  let flags = u16::from(col) | u16::from(col_moves) << 14 | u16::from(row_moves) << 15;
  [row.to_le_bytes(), flags.to_le_bytes()].concat()
}

fn reference(row: u16, col: u8) -> Vec<u8> {
  [&[0x44][..], &cell(row, col, true, true)].concat()
}

/// A range, both corners moving.
fn area(rows: (u16, u16), cols: (u8, u8)) -> Vec<u8> {
  let corner = |col: u8| (u16::from(col) | 0xC000).to_le_bytes();
  [
    &[0x25][..],
    &rows.0.to_le_bytes(),
    &rows.1.to_le_bytes(),
    &corner(cols.0),
    &corner(cols.1),
  ]
  .concat()
}

fn string(value: &str) -> Vec<u8> {
  let mut out = vec![0x17, u8::try_from(value.len()).unwrap(), 0];
  out.extend(value.as_bytes());
  out
}

/// A function that always takes the same number of arguments, by its number.
fn fixed(function: u16) -> Vec<u8> {
  [&[0x41][..], &function.to_le_bytes()].concat()
}

/// A function with the number of arguments it was given.
fn variable(count: u8, function: u16) -> Vec<u8> {
  [&[0x42, count][..], &function.to_le_bytes()].concat()
}

fn all(parts: &[Vec<u8>]) -> Vec<u8> {
  parts.concat()
}

#[test]
fn numbers_text_and_the_values_of_truth_and_error_are_written_as_a_sheet_writes_them() {
  assert_eq!(shown(&int(42)), "42");
  let half = [&[0x1F][..], &1234.5f64.to_le_bytes()].concat();
  assert_eq!(shown(&half), "1234.5");
  let million = [&[0x1F][..], &1_000_000f64.to_le_bytes()].concat();
  assert_eq!(shown(&million), "1000000");
  assert_eq!(shown(&string("due")), "\"due\"");
  // A quotation mark inside text is written twice.
  assert_eq!(shown(&string("6\" bolt")), "\"6\"\" bolt\"");
  // Text two bytes to a character.
  let wide: Vec<u8> = [0x17, 2, 1]
    .into_iter()
    .chain("\u{65e5}\u{672c}".encode_utf16().flat_map(u16::to_le_bytes))
    .collect();
  assert_eq!(shown(&wide), "\"\u{65e5}\u{672c}\"");
  assert_eq!(shown(&[0x1D, 1]), "TRUE");
  assert_eq!(shown(&[0x1D, 0]), "FALSE");
  assert_eq!(shown(&[0x1C, 0x07]), "#DIV/0!");
  assert_eq!(shown(&[0x1C, 0x2A]), "#N/A");
}

#[test]
fn operators_are_written_between_what_they_join_with_the_brackets_the_formula_has() {
  // 1+2*3, as the file keeps it: each operator after what it joins.
  let sum = all(&[int(1), int(2), int(3), vec![0x05], vec![0x03]]);
  assert_eq!(shown(&sum), "1+2*3");
  let bracketed = all(&[int(1), int(2), vec![0x03, 0x15], int(3), vec![0x05]]);
  assert_eq!(shown(&bracketed), "(1+2)*3");
  let each = [
    (0x03, "+"),
    (0x04, "-"),
    (0x05, "*"),
    (0x06, "/"),
    (0x07, "^"),
    (0x08, "&"),
    (0x09, "<"),
    (0x0A, "<="),
    (0x0B, "="),
    (0x0C, ">="),
    (0x0D, ">"),
    (0x0E, "<>"),
    (0x0F, " "),
    (0x10, ","),
    (0x11, ":"),
  ];
  for (token, sign) in each {
    let joined = all(&[reference(0, 0), reference(1, 1), vec![token]]);
    assert_eq!(shown(&joined), format!("A1{sign}B2"), "{token:#04X}");
  }
  // One that stands before what it is for, and one that stands after.
  assert_eq!(shown(&all(&[int(5), vec![0x13]])), "-5");
  assert_eq!(shown(&all(&[int(5), vec![0x12]])), "+5");
  assert_eq!(shown(&all(&[int(5), vec![0x14]])), "5%");
}

#[test]
fn a_reference_says_which_of_its_row_and_column_stay_put() {
  let one = |row_moves: bool, col_moves: bool| {
    shown(&[&[0x24][..], &cell(4, 2, row_moves, col_moves)].concat())
  };
  assert_eq!(one(true, true), "C5");
  assert_eq!(one(false, false), "$C$5");
  assert_eq!(one(false, true), "C$5");
  assert_eq!(one(true, false), "$C5");
  assert_eq!(shown(&area((0, 9), (1, 3))), "B1:D10");
  // A range of whole columns names no rows, and one of whole rows no columns.
  assert_eq!(shown(&area((0, 65535), (0, 1))), "A:B");
  assert_eq!(shown(&area((2, 4), (0, 255))), "3:5");
  let fixed_columns = [
    &[0x25][..],
    &0u16.to_le_bytes(),
    &65535u16.to_le_bytes(),
    &2u16.to_le_bytes(),
    &2u16.to_le_bytes(),
  ]
  .concat();
  assert_eq!(shown(&fixed_columns), "$C:$C");
  // A reference to what has been deleted.
  assert_eq!(shown(&[0x2A, 0, 0, 0, 0]), "#REF!");
  assert_eq!(shown(&[0x2B, 0, 0, 0, 0, 0, 0, 0, 0]), "#REF!");
}

#[test]
fn a_function_is_written_by_its_name_with_its_arguments() {
  // One that always takes two, and one that takes none.
  let round = all(&[reference(0, 0), int(2), fixed(27)]);
  assert_eq!(shown(&round), "ROUND(A1,2)");
  assert_eq!(shown(&fixed(19)), "PI()");
  assert_eq!(shown(&fixed(221)), "TODAY()");
  // One that takes as many as it is given.
  let sum = all(&[area((1, 8), (1, 1)), reference(0, 3), variable(2, 4)]);
  assert_eq!(shown(&sum), "SUM(B2:B9,D1)");
  // A sum of one thing is kept the short way.
  let short = all(&[area((1, 8), (1, 1)), vec![0x19, 0x10, 0, 0]]);
  assert_eq!(shown(&short), "SUM(B2:B9)");
  // The file notes where a choice jumps to; the notes are not part of the text.
  let choice = all(&[
    reference(0, 0),
    int(0),
    vec![0x0D],
    vec![0x19, 0x02, 9, 0],
    string("y"),
    vec![0x19, 0x08, 3, 0],
    string("n"),
    vec![0x19, 0x08, 3, 0],
    variable(3, 1),
  ]);
  assert_eq!(shown(&choice), "IF(A1>0,\"y\",\"n\")");
  // An argument left out is written as nothing between its commas.
  let skipped = all(&[reference(0, 0), vec![0x16], int(2), variable(3, 1)]);
  assert_eq!(shown(&skipped), "IF(A1,,2)");
  let nested = all(&[
    area((0, 2), (0, 0)),
    vec![0x19, 0x10, 0, 0],
    int(2),
    fixed(27),
    vec![0x13],
  ]);
  assert_eq!(shown(&nested), "-ROUND(SUM(A1:A3),2)");
  // A function this release has no name for is not written out.
  assert_eq!(written(&all(&[int(1), fixed(999)])), None);
}

#[test]
fn a_name_is_written_as_the_workbook_spells_it() {
  let rate = [&[0x23][..], &1u32.to_le_bytes()].concat();
  assert_eq!(
    shown(&all(&[reference(0, 0), rate.clone(), vec![0x05]])),
    "A1*Rate"
  );
  let area_name = [&[0x43][..], &2u32.to_le_bytes()].concat();
  assert_eq!(shown(&area_name), "Print_Area");
  // A name the workbook does not have.
  assert_eq!(written(&[&[0x23][..], &9u32.to_le_bytes()].concat()), None);
  // A function an add-in gives is named in a list of its own, and called with its name as
  // the first of what it is given.
  let add_in = [&[0x39][..], &6u16.to_le_bytes(), &1u32.to_le_bytes()].concat();
  let month_end = all(&[add_in, reference(0, 0), int(1), variable(3, 255)]);
  assert_eq!(shown(&month_end), "EOMONTH(A1,1)");
  // A name another workbook defines.
  let theirs = [&[0x39][..], &7u16.to_le_bytes(), &1u32.to_le_bytes()].concat();
  assert_eq!(shown(&theirs), "[1]!Budget");
}

#[test]
fn a_reference_to_another_sheet_names_the_sheet() {
  let on = |span: u16| shown(&[&[0x3A][..], &span.to_le_bytes(), &cell(0, 0, true, true)].concat());
  assert_eq!(on(0), "Costs!A1");
  // A name with a space in it, or one that reads as a cell, is put in quotation marks, and
  // a mark inside the name is written twice.
  assert_eq!(on(1), "'My Sheet'!A1");
  assert_eq!(on(3), "'Q4'!A1");
  assert_eq!(on(4), "'It''s'!A1");
  assert_eq!(on(2), "Costs:Last!A1");
  // A sheet that has been deleted.
  assert_eq!(on(5), "#REF!A1");
  // A sheet of another workbook, which is named by its place among those the file links to.
  assert_eq!(on(7), "[1]Feb!A1");
  let range = [
    &[0x3B][..],
    &1u16.to_le_bytes(),
    &0u16.to_le_bytes(),
    &1u16.to_le_bytes(),
    &0xC000u16.to_le_bytes(),
    &0xC001u16.to_le_bytes(),
  ]
  .concat();
  assert_eq!(shown(&range), "'My Sheet'!A1:B2");
  assert_eq!(shown(&[0x3C, 0, 0, 0, 0, 0, 0]), "Costs!#REF!");
}

#[test]
fn a_list_of_constants_is_read_from_what_follows_the_formula() {
  // Two columns and two rows: a number, text, a value of truth, and an error.
  let mut extra = vec![1, 1, 0];
  extra.push(0x01);
  extra.extend(1.5f64.to_le_bytes());
  extra.extend([0x02, 1, 0, 0, b'x']);
  extra.extend([0x04, 1, 0, 0, 0, 0, 0, 0, 0]);
  extra.extend([0x10, 0x2A, 0, 0, 0, 0, 0, 0, 0]);
  let tokens = all(&[vec![0x60, 0, 0, 0, 0, 0, 0, 0], vec![0x19, 0x10, 0, 0]]);
  assert_eq!(
    text(&tokens, &extra, (0, 0), &links()).as_deref(),
    Some("SUM({1.5,\"x\";TRUE,#N/A})")
  );
  // With nothing after the formula there is no list to write.
  assert_eq!(text(&tokens, &[], (0, 0), &links()), None);
}

#[test]
fn a_formula_many_cells_share_is_written_for_the_cell_that_uses_it() {
  // One row up and one column left of the cell, and a range two rows down from it.
  let up_left = [
    &[0x4C][..],
    &0xFFFFu16.to_le_bytes(),
    &0xC0FFu16.to_le_bytes(),
  ]
  .concat();
  let below = [
    &[0x2D][..],
    &1u16.to_le_bytes(),
    &2u16.to_le_bytes(),
    &0xC000u16.to_le_bytes(),
    &0xC000u16.to_le_bytes(),
  ]
  .concat();
  // A row that stays put while the column moves.
  let anchored = [&[0x2C][..], &0u16.to_le_bytes(), &0x4001u16.to_le_bytes()].concat();
  let tokens = all(&[
    up_left,
    below,
    vec![0x19, 0x10, 0, 0],
    vec![0x03],
    anchored,
    vec![0x05],
  ]);
  let at = |row: u32, col: u32| text(&tokens, &[], (row, col), &links()).unwrap();
  assert_eq!(at(1, 1), "A1+SUM(B3:B4)*C$1");
  assert_eq!(at(9, 4), "D9+SUM(E11:E12)*F$1");
  // A step off the top of the sheet comes round to its foot.
  assert_eq!(at(0, 1), "A65536+SUM(B2:B3)*C$1");
}

#[test]
fn what_cannot_be_read_as_a_formula_is_not_written_out() {
  assert_eq!(written(&[]), None);
  // A token this release does not know, a token cut short, and an operator with nothing
  // to join.
  assert_eq!(written(&[0x1A]), None);
  assert_eq!(written(&[0x1E, 1]), None);
  assert_eq!(written(&all(&[int(1), vec![0x03]])), None);
  // Two things with nothing between them.
  assert_eq!(written(&all(&[int(1), int(2)])), None);
  // A formula that says only which cell it is shared from is the caller's to follow.
  assert_eq!(written(&[0x01, 0, 0, 0, 0]), None);
}
