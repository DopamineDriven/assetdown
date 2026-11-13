use super::*;

/// `formula` moved one row down and one column right.
fn moved(formula: &str) -> String {
  shift_formula(formula, 1, 1)
}

#[test]
fn relative_parts_move_and_absolute_parts_stay() {
  assert_eq!(moved("B2"), "C3");
  assert_eq!(moved("$B$2"), "$B$2");
  assert_eq!(moved("$B2"), "$B3");
  assert_eq!(moved("B$2"), "C$2");
  assert_eq!(shift_formula("B2", 3, 0), "B5");
  assert_eq!(shift_formula("B2", 0, 3), "E2");
  assert_eq!(shift_formula("D9", -2, -3), "A7");
  assert_eq!(
    shift_formula("Z1", 0, 1),
    "AA1",
    "a column that gains a letter"
  );
  assert_eq!(moved("b2"), "C3", "references are spelled in capitals");
}

#[test]
fn every_reference_in_an_expression_moves_and_nothing_else_does() {
  assert_eq!(moved("SUM(A1:B2)*C3"), "SUM(B2:C3)*D4");
  assert_eq!(moved("A1+A1*2-(B7/C9)^2"), "B2+B2*2-(C8/D10)^2");
  assert_eq!(moved("IF(A1>0,TRUE,FALSE)"), "IF(B2>0,TRUE,FALSE)");
  assert_eq!(moved("A1 B2"), "B2 C3", "an intersection");
  assert_eq!(moved("{1,2;3,4}"), "{1,2;3,4}");
  assert_eq!(moved("-A1%"), "-B2%");
  assert_eq!(moved("@A1"), "@B2");
  assert_eq!(shift_formula("C3*2", 1, 0), "C4*2");
}

#[test]
fn a_formula_that_does_not_move_is_unchanged() {
  for formula in [
    "SUM($A$1:B2)*Sheet2!C3",
    "IF(AND(A1>1E5,B:B<>\"\"),'It''s here'!$A1,#N/A)",
    "Table1[[#This Row],[Unit price]]*_xlfn.XLOOKUP(A1,$1:$1,2:2)",
  ] {
    assert_eq!(shift_formula(formula, 0, 0), formula);
  }
}

#[test]
fn functions_and_names_that_look_like_references_are_not() {
  assert_eq!(moved("LOG10(A1)"), "LOG10(B2)");
  assert_eq!(moved("ATAN2(A1,B1)"), "ATAN2(B2,C2)");
  assert_eq!(moved("DAYS360(A1,B1)"), "DAYS360(B2,C2)");
  assert_eq!(moved("BIN2DEC(A1)"), "BIN2DEC(B2)");
  assert_eq!(moved("Q1_Sales+Rate_A1+TAX"), "Q1_Sales+Rate_A1+TAX");
  assert_eq!(
    moved("A1B*2"),
    "A1B*2",
    "letters after the digits make it a name"
  );
  assert_eq!(moved("Gr\u{f6}\u{df}e*A1"), "Gr\u{f6}\u{df}e*B2");
  assert_eq!(
    moved("_xlfn.XLOOKUP(A1,B1:B9,C1:C9)"),
    "_xlfn.XLOOKUP(B2,C2:C10,D2:D10)"
  );
  assert_eq!(
    moved("_xlfn.LET(_xlpm.x,A1,_xlpm.x*2)"),
    "_xlfn.LET(_xlpm.x,B2,_xlpm.x*2)"
  );
}

#[test]
fn quoted_text_is_copied_and_references_after_it_still_move() {
  assert_eq!(moved("\"A1\"&A1"), "\"A1\"&B2");
  assert_eq!(
    moved("\"say \"\"A1\"\" to \"&A1"),
    "\"say \"\"A1\"\" to \"&B2",
    "a doubled quote does not end the text"
  );
  assert_eq!(moved("\"\"&A1"), "\"\"&B2", "empty text");
  assert_eq!(moved("\"unclosed A1"), "\"unclosed A1");
}

#[test]
fn sheet_names_stay_and_the_reference_after_them_moves() {
  assert_eq!(moved("Sheet2!A1"), "Sheet2!B2");
  assert_eq!(moved("'My Sheet A1'!A1"), "'My Sheet A1'!B2");
  assert_eq!(moved("'It''s A1'!A1+1"), "'It''s A1'!B2+1");
  assert_eq!(
    moved("AB12!A1"),
    "AB12!B2",
    "a sheet name shaped like a cell"
  );
  assert_eq!(moved("Data!$B$2:$B$10"), "Data!$B$2:$B$10");
  assert_eq!(
    moved("Jan:Mar!B2"),
    "Jan:Mar!C3",
    "a span of sheets is not columns"
  );
  assert_eq!(moved("[1]Sheet1!A1"), "[1]Sheet1!B2");
  assert_eq!(
    moved("'[Book A1.xlsx]Sheet1'!A1"),
    "'[Book A1.xlsx]Sheet1'!B2"
  );
}

#[test]
fn table_references_are_copied_whole() {
  assert_eq!(
    moved("Table1[[#This Row],[Qty]]*A1"),
    "Table1[[#This Row],[Qty]]*B2"
  );
  assert_eq!(moved("SUM(Table1[A1])"), "SUM(Table1[A1])");
  assert_eq!(
    moved("Table1[[A1']]]+A1"),
    "Table1[[A1']]]+B2",
    "an escaped bracket does not close the reference"
  );
}

#[test]
fn whole_columns_and_whole_rows_move_along_their_own_axis() {
  assert_eq!(moved("SUM(A:A)"), "SUM(B:B)");
  assert_eq!(moved("SUM($A:B)"), "SUM($A:C)");
  assert_eq!(moved("SUM(1:1)"), "SUM(2:2)");
  assert_eq!(moved("SUM($1:3)"), "SUM($1:4)");
  assert_eq!(moved("SUM($1:$3)"), "SUM($1:$3)");
  assert_eq!(moved("Sheet2!A:C"), "Sheet2!B:D");
  assert_eq!(shift_formula("SUM(A:A)", 5, 0), "SUM(A:A)");
  assert_eq!(shift_formula("SUM(1:1)", 0, 5), "SUM(1:1)");
}

#[test]
fn numbers_are_not_references() {
  assert_eq!(moved("1E5+A1"), "1E5+B2");
  assert_eq!(moved("2.5E-3*A1"), "2.5E-3*B2");
  assert_eq!(moved("1.5+E5"), "1.5+F6", "E5 on its own is a cell");
  assert_eq!(moved(".5*A1"), ".5*B2");
  assert_eq!(moved("A1*100"), "B2*100");
  assert_eq!(moved("3E"), "3E", "an exponent needs digits");
}

#[test]
fn error_values_are_copied_and_the_spill_operator_is_not_one() {
  assert_eq!(moved("#REF!+A1"), "#REF!+B2");
  assert_eq!(moved("IFERROR(A1,#N/A)"), "IFERROR(B2,#N/A)");
  assert_eq!(moved("#DIV/0!"), "#DIV/0!");
  assert_eq!(moved("#NAME?"), "#NAME?");
  assert_eq!(moved("SUM(A1#)"), "SUM(B2#)");
}

#[test]
fn a_reference_that_leaves_the_grid_is_a_ref_error() {
  assert_eq!(shift_formula("A1", -1, 0), "#REF!");
  assert_eq!(shift_formula("A1", 0, -1), "#REF!");
  assert_eq!(shift_formula("XFD1", 0, 1), "#REF!");
  assert_eq!(shift_formula("A1048576", 1, 0), "#REF!");
  assert_eq!(shift_formula("$A$1+A1", -1, -1), "$A$1+#REF!");
  assert_eq!(shift_formula("SUM(A:B)", 0, -1), "SUM(#REF!:A)");
  assert_eq!(shift_formula("XFD1048576", 0, 0), "XFD1048576");
}
