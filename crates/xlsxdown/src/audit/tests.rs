use super::*;
use crate::address::parse_cell_ref;
use crate::testing::Holds::{Formula, Money, MoneyFormula, Number, Text, TextFormula};
use crate::testing::{Holds, an_invoice, one, sheet, workbook};

fn at(sheet: usize, reference: &str) -> CellAt {
  let (row, col) = parse_cell_ref(reference).unwrap();
  CellAt { sheet, row, col }
}

/// Each line as the cell it is, its amount, and its description.
fn lines(audit: &Audit) -> Vec<(String, f64, Option<&str>)> {
  audit
    .lines
    .iter()
    .map(|line| {
      (
        crate::address::cell_ref(line.at.row, line.at.col),
        line.amount,
        line.description.as_deref(),
      )
    })
    .collect()
}

#[test]
fn the_total_is_the_formula_nothing_reads_and_its_lines_are_what_it_adds_up() {
  let audit = an_invoice().audit().unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "D7"), 66.0));
  // The subtotal is followed down to what it sums. The tax is worked out from the subtotal
  // and is a line of its own, so that the lines come to the total.
  assert_eq!(
    lines(&audit),
    [
      ("D2".to_string(), 20.0, Some("Bolt")),
      ("D3".to_string(), 15.5, Some("Seal")),
      ("D4".to_string(), 24.5, Some("Labor")),
      ("D6".to_string(), 6.0, Some("Tax")),
    ]
  );
  assert!(audit.adds_up);
}

#[test]
fn a_line_that_multiplies_two_cells_says_which_is_how_many_and_which_is_the_price() {
  let audit = an_invoice().audit().unwrap();
  // The price is the one shown as money.
  assert_eq!(
    (audit.lines[0].quantity, audit.lines[0].unit_price),
    (Some(at(0, "B2")), Some(at(0, "C2")))
  );
  // A number typed in, and an amount worked out some other way, have neither.
  assert_eq!(
    (audit.lines[2].quantity, audit.lines[2].unit_price),
    (None, None)
  );
  assert_eq!(
    (audit.lines[3].quantity, audit.lines[3].unit_price),
    (None, None)
  );
  // Where neither of the two is shown as money, or both are, which is which is not known.
  let unknown = one(&[
    ("B2", Number(2.0)),
    ("C2", Number(10.0)),
    ("D2", Formula("B2*C2", 20.0)),
    ("D3", Formula("SUM(D2:D2)", 20.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(
    (unknown.lines[0].quantity, unknown.lines[0].unit_price),
    (None, None)
  );
}

#[test]
fn hours_times_a_rate_is_one_line_and_gives_way_to_the_rows_that_come_to_the_same() {
  // Each task has its hours, and beside them its hours times the rate. The grand total is
  // worked out from the total of the hours times the rate: it never reads the amounts.
  let tasks = [
    ("A1", Text("Hourly rate")),
    ("B1", Money(120.0)),
    ("A3", Text("Inspect the cowl")),
    ("B3", Number(3.0)),
    ("C3", MoneyFormula("B3*$B$1", 360.0)),
    ("A4", Text("Replace the seal")),
    ("B4", Number(1.5)),
    ("C4", MoneyFormula("B4*$B$1", 180.0)),
    ("A5", Text("Unused row")),
    ("C5", MoneyFormula("B5*$B$1", 0.0)),
    ("B6", Formula("SUM(B3:B5)", 4.5)),
    ("C6", MoneyFormula("SUM(C3:C5)", 540.0)),
    ("A8", Text("Work")),
    ("B8", MoneyFormula("B6*B1", 540.0)),
    ("A9", Text("Materials")),
    ("B9", Money(75.25)),
    ("A11", Text("To pay")),
    ("B11", MoneyFormula("SUM(B8:C10)", 615.25)),
  ];
  let audit = one(&tasks).audit().unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "B11"), 615.25));
  // The rate and the hours are not lines. The rows that come to the same as the hours
  // times the rate stand in its place, each with its hours and the rate.
  assert_eq!(
    lines(&audit),
    [
      ("C3".to_string(), 360.0, Some("Inspect the cowl")),
      ("C4".to_string(), 180.0, Some("Replace the seal")),
      ("B9".to_string(), 75.25, Some("Materials")),
    ]
  );
  assert_eq!(
    (audit.lines[0].quantity, audit.lines[0].unit_price),
    (Some(at(0, "B3")), Some(at(0, "B1")))
  );
  assert!(audit.adds_up);

  // With no such rows, the hours times the rate is the line.
  let without: Vec<_> = tasks
    .into_iter()
    .filter(|(reference, _)| !reference.starts_with('C'))
    .collect();
  let audit = one(&without).audit().unwrap();
  assert_eq!(
    lines(&audit),
    [
      ("B8".to_string(), 540.0, Some("Work")),
      ("B9".to_string(), 75.25, Some("Materials")),
    ]
  );
  assert_eq!(
    (audit.lines[0].quantity, audit.lines[0].unit_price),
    (Some(at(0, "B6")), Some(at(0, "B1")))
  );
}

#[test]
fn a_total_is_followed_onto_the_other_sheets_it_reads() {
  let book = workbook(vec![
    sheet(
      "Invoice",
      false,
      &[
        ("A5", Text("Total due")),
        ("B5", MoneyFormula("Labor!C4+'Parts & Freight'!C3", 190.0)),
      ],
    ),
    sheet(
      "Labor",
      false,
      &[
        ("A2", Text("Inspection")),
        ("C2", Money(100.0)),
        ("A3", Text("Repair")),
        ("C3", Money(50.0)),
        ("C4", MoneyFormula("SUM(C2:C3)", 150.0)),
      ],
    ),
    // A sheet kept out of sight still holds what the total adds up.
    sheet(
      "Parts & Freight",
      true,
      &[
        ("A2", Text("Seal")),
        ("C2", Money(40.0)),
        ("C3", MoneyFormula("SUM(C2:C2)", 40.0)),
      ],
    ),
  ]);
  let audit = book.audit().unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "B5"), 190.0));
  let found: Vec<(CellAt, f64, Option<&str>)> = audit
    .lines
    .iter()
    .map(|line| (line.at, line.amount, line.description.as_deref()))
    .collect();
  assert_eq!(
    found,
    [
      (at(1, "C2"), 100.0, Some("Inspection")),
      (at(1, "C3"), 50.0, Some("Repair")),
      (at(2, "C2"), 40.0, Some("Seal")),
    ]
  );
  assert!(audit.adds_up);
}

#[test]
fn of_the_formulas_nothing_reads_the_total_is_the_one_that_reaches_the_most() {
  let book = one(&[
    ("A1", Money(1.0)),
    ("A2", Money(2.0)),
    ("A3", Money(3.0)),
    ("A4", MoneyFormula("SUM(A1:A3)", 6.0)),
    // A check on two of the cells, lower on the sheet, which nothing reads either.
    ("C9", MoneyFormula("A1+A2", 3.0)),
  ]);
  assert_eq!(book.audit().unwrap().total, at(0, "A4"));

  // Reaching as many, the one shown as money is the total; and of two alike, the lower.
  let book = one(&[
    ("A1", Money(1.0)),
    ("A2", Money(2.0)),
    ("B3", Formula("A1+A2", 3.0)),
    ("C3", MoneyFormula("A1+A2", 3.0)),
    ("C8", MoneyFormula("SUM(A1:A2)", 3.0)),
  ]);
  assert_eq!(book.audit().unwrap().total, at(0, "C8"));

  // A formula on a sheet kept out of sight is not the total, however much it reaches.
  let book = workbook(vec![
    sheet(
      "Shown",
      false,
      &[("A1", Money(5.0)), ("A2", MoneyFormula("SUM(A1:A1)", 5.0))],
    ),
    sheet(
      "Hidden",
      true,
      &[
        ("A1", Money(1.0)),
        ("A2", Money(2.0)),
        ("A3", Money(3.0)),
        ("A4", MoneyFormula("SUM(A1:A3)", 6.0)),
      ],
    ),
  ]);
  assert_eq!(book.audit().unwrap().total, at(0, "A2"));
}

#[test]
fn a_workbook_with_no_formula_that_adds_cells_up_has_no_total_to_follow() {
  assert_eq!(
    one(&[("A1", Money(5.0)), ("A2", Text("Total"))]).audit(),
    None
  );
  // A formula that reads no cell, and one that reads only text.
  assert_eq!(one(&[("A1", Formula("1+2", 3.0))]).audit(), None);
  assert_eq!(
    one(&[("A1", Text("five")), ("A2", Formula("LEN(A1)", 4.0))]).audit(),
    None
  );
  assert_eq!(workbook(Vec::new()).audit(), None);
}

#[test]
fn what_a_total_takes_away_is_a_line_below_nought() {
  let audit = one(&[
    ("A1", Text("Work")),
    ("B1", Money(100.0)),
    ("A2", Text("Discount")),
    ("B2", Money(10.0)),
    ("A3", Text("Credit")),
    ("B3", Money(5.0)),
    ("B4", MoneyFormula("B1-B2-B3", 85.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(
    lines(&audit),
    [
      ("B1".to_string(), 100.0, Some("Work")),
      ("B2".to_string(), -10.0, Some("Discount")),
      ("B3".to_string(), -5.0, Some("Credit")),
    ]
  );
  assert!(audit.adds_up);
}

#[test]
fn lines_of_nought_and_cells_of_text_are_not_lines() {
  let audit = one(&[
    ("A1", Text("Used")),
    ("B1", Money(7.0)),
    ("A2", Text("Unused")),
    ("B2", Money(0.0)),
    ("B3", Text("n/a")),
    ("B4", MoneyFormula("SUM(B1:B3)", 7.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(lines(&audit), [("B1".to_string(), 7.0, Some("Used"))]);
}

#[test]
fn a_total_that_is_more_than_a_sum_of_cells_says_its_lines_do_not_come_to_it() {
  // A fee is typed into the formula: no cell holds it.
  let audit = one(&[
    ("A1", Money(10.0)),
    ("A2", Money(20.0)),
    ("A3", MoneyFormula("SUM(A1:A2)", 30.0)),
    ("A4", MoneyFormula("A3+5", 35.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "A4"), 35.0));
  // The formula is not a plain sum, so it is not followed: it is all there is.
  assert!(audit.lines.is_empty());
  assert!(!audit.adds_up);
}

#[test]
fn sums_wrapped_in_rounding_or_counted_as_subtotals_are_followed_like_any_other() {
  let audit = one(&[
    ("A1", Money(1.005)),
    ("A2", Money(2.0)),
    ("A3", MoneyFormula("ROUND(SUM(A1:A2),2)", 3.01)),
    ("A4", Money(4.0)),
    ("A5", MoneyFormula("SUBTOTAL(9,A3:A4)", 7.01)),
    ("A6", MoneyFormula("+A5", 7.01)),
  ])
  .audit()
  .unwrap();
  assert_eq!(audit.total, at(0, "A6"));
  let amounts: Vec<f64> = audit.lines.iter().map(|line| line.amount).collect();
  assert_eq!(amounts, [1.005, 2.0, 4.0]);
  // Half a cent of rounding to a line is still coming to the total.
  assert!(audit.adds_up);
}

#[test]
fn formulas_that_read_each_other_in_a_ring_are_gone_round_once() {
  let audit = one(&[
    ("A1", Money(1.0)),
    ("A2", MoneyFormula("SUM(A1:A1)+A3", 1.0)),
    ("A3", MoneyFormula("SUM(A2:A2)", 1.0)),
    ("A4", MoneyFormula("SUM(A2:A3)", 2.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(audit.total, at(0, "A4"));
  assert!(audit.lines.len() <= 3);
}

#[test]
fn a_line_is_described_by_the_text_of_its_row_or_failing_that_the_heading_over_it() {
  let audit = one(&[
    ("D1", Text("Fees")),
    ("A2", Text("K-204")),
    ("B2", Text("Gasket")),
    ("D2", Money(12.5)),
    ("E2", Text("each")),
    // Nothing to the left: what stands to the right.
    ("D3", Money(4.0)),
    ("E3", Text("Carriage")),
    // Nothing in the row: the heading over the column.
    ("D4", Money(5.0)),
    ("D5", MoneyFormula("SUM(D2:D4)", 21.5)),
  ])
  .audit()
  .unwrap();
  assert_eq!(
    lines(&audit),
    [
      ("D2".to_string(), 12.5, Some("K-204 Gasket")),
      ("D3".to_string(), 4.0, Some("Carriage")),
      ("D4".to_string(), 5.0, Some("Fees")),
    ]
  );
  // And it says which cells the words were read from.
  let read_from: Vec<&[CellAt]> = audit
    .lines
    .iter()
    .map(|line| line.described_by.as_slice())
    .collect();
  assert_eq!(
    read_from,
    [
      &[at(0, "A2"), at(0, "B2")][..],
      &[at(0, "E3")][..],
      &[at(0, "D1")][..],
    ]
  );
}

#[test]
fn a_workbook_whose_formulas_read_more_cells_than_any_real_one_is_not_followed() {
  // Three thousand formulas, each summing the same two thousand cells.
  let numbers: Vec<String> = (1..=2000).map(|row| format!("A{row}")).collect();
  let formulas: Vec<String> = (1..=3000).map(|row| format!("B{row}")).collect();
  let mut cells: Vec<(&str, Holds<'_>)> =
    numbers.iter().map(|at| (at.as_str(), Money(1.0))).collect();
  cells.extend(
    formulas
      .iter()
      .map(|at| (at.as_str(), MoneyFormula("SUM(A1:A2000)", 2000.0))),
  );
  assert_eq!(one(&cells).audit(), None);
}

#[test]
fn a_workbook_made_to_be_slow_to_follow_is_given_up_on() {
  // A chain of two hundred formulas, each reading the one before it, and two hundred more
  // that nothing reads, each reading the chain's last. To say which of those reaches
  // furthest, every one is followed the whole way down: forty thousand looks for four
  // hundred formulas, and the square of however many a file cares to hold.
  let chain: Vec<(String, String)> = (2..=200)
    .map(|row| (format!("A{row}"), format!("A{}+0", row - 1)))
    .collect();
  let heads: Vec<String> = (1..=200).map(|row| format!("B{row}")).collect();
  let mut cells: Vec<(&str, Holds<'_>)> = vec![("A1", Money(1.0))];
  cells.extend(
    chain
      .iter()
      .map(|(at, reads)| (at.as_str(), MoneyFormula(reads, 1.0))),
  );
  cells.extend(
    heads
      .iter()
      .map(|at| (at.as_str(), MoneyFormula("A200+0", 1.0))),
  );
  let book = one(&cells);
  // Within what a real workbook is allowed, it is followed.
  assert_eq!(
    book.audit().map(|audit| (audit.total, audit.value)),
    Some((at(0, "B200"), 1.0))
  );
  // Allowed fewer looks than it takes, it is not.
  assert_eq!(book.audited(30_000), None);
  assert!(book.audited(60_000).is_some());
}

#[test]
fn a_total_that_is_one_cell_times_another_is_itself_the_one_line() {
  // So many hours at so much an hour, and nothing else on the sheet to add up.
  let audit = one(&[
    ("A1", Text("Rate")),
    ("B1", Money(142.5)),
    ("A2", Text("Hours")),
    ("B2", Number(3.0)),
    ("A3", Text("Total")),
    ("B3", MoneyFormula("B1*B2", 427.5)),
  ])
  .audit()
  .unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "B3"), 427.5));
  assert_eq!(lines(&audit), [("B3".to_string(), 427.5, Some("Total"))]);
  assert_eq!(
    (audit.lines[0].quantity, audit.lines[0].unit_price),
    (Some(at(0, "B2")), Some(at(0, "B1")))
  );
  assert!(audit.adds_up);
}

#[test]
fn a_sum_kept_blank_until_it_has_something_to_show_is_followed_like_any_other() {
  let audit = one(&[
    ("A1", Text("Gasket")),
    ("B1", Number(2.0)),
    ("C1", Money(12.5)),
    ("D1", MoneyFormula("IF(B1=\"\",\"\",B1*C1)", 25.0)),
    ("A2", Text("Carriage")),
    ("D2", Money(4.0)),
    // A row not filled in: its amount shows nothing.
    ("D3", TextFormula("IF(B3=\"\",\"\",B3*C3)", "")),
    ("A4", Text("Parts")),
    ("D4", MoneyFormula("IF(SUM(D1:D3)>0,SUM(D1:D3),\"\")", 29.0)),
    ("A5", Text("Tax")),
    ("D5", Money(2.9)),
    ("A6", Text("In all")),
    (
      "D6",
      MoneyFormula("IF(SUM(D4:D5)>0,ROUND(SUM(D4,D5),2),\"\")", 31.9),
    ),
  ])
  .audit()
  .unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "D6"), 31.9));
  assert_eq!(
    lines(&audit),
    [
      ("D1".to_string(), 25.0, Some("Gasket")),
      ("D2".to_string(), 4.0, Some("Carriage")),
      ("D5".to_string(), 2.9, Some("Tax")),
    ]
  );
  assert!(audit.adds_up);
  // The amount that is two cells multiplied, behind its guard, says which is which.
  assert_eq!(
    (audit.lines[0].quantity, audit.lines[0].unit_price),
    (Some(at(0, "B1")), Some(at(0, "C1")))
  );
}

#[test]
fn what_a_guard_asks_about_is_not_a_line_of_what_it_guards() {
  // The count of jobs decides whether the total shows. It is not part of the total.
  let audit = one(&[
    ("A1", Text("Jobs")),
    ("B1", Number(3.0)),
    ("A2", Text("Parts")),
    ("B2", Money(40.0)),
    ("A3", Text("Labor")),
    ("B3", Money(20.0)),
    ("A4", Text("In all")),
    ("B4", MoneyFormula("IF(B1>0,SUM(B2:B3),0)", 60.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(
    lines(&audit),
    [
      ("B2".to_string(), 40.0, Some("Parts")),
      ("B3".to_string(), 20.0, Some("Labor")),
    ]
  );
  assert!(audit.adds_up);
}

#[test]
fn a_total_written_out_as_text_is_the_sum_of_the_cells_it_adds_up() {
  let audit = one(&[
    ("A1", Text("Currency")),
    ("B1", Text("USD")),
    ("A2", Text("Parts")),
    ("B2", Money(1200.0)),
    ("A3", Text("Carriage")),
    ("B3", Money(86.5)),
    ("A4", Text("In all")),
    (
      "B4",
      TextFormula(
        "IF(B1=\"\",TEXT(SUM(B2:B3),\"#,##0.00\"),B1&\" \"&TEXT(SUM(B2:B3),\"#,##0.00\"))",
        "USD 1,286.50",
      ),
    ),
    // The same written out for a part of it, which nothing reads either.
    (
      "D2",
      TextFormula("TEXT(SUM(B2:B2),\"#,##0.00\")", "1,200.00"),
    ),
    // Text that is worked out but holds no amount is not a total.
    ("D4", TextFormula("TEXT(SUM(B2:B3),\"\")", "")),
  ])
  .audit()
  .unwrap();
  assert_eq!((audit.total, audit.value), (at(0, "B4"), 1286.5));
  assert_eq!(
    lines(&audit),
    [
      ("B2".to_string(), 1200.0, Some("Parts")),
      ("B3".to_string(), 86.5, Some("Carriage")),
    ]
  );
  assert!(audit.adds_up);
}

#[test]
fn a_minus_before_a_sum_takes_away_every_cell_the_sum_adds_up() {
  let audit = one(&[
    ("A1", Text("Work")),
    ("B1", Money(100.0)),
    ("A2", Text("Deposit")),
    ("B2", Money(30.0)),
    ("A3", Text("Discount")),
    ("B3", Money(5.0)),
    ("A4", Text("To pay")),
    ("B4", MoneyFormula("B1-SUM(B2:B3)", 65.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(
    lines(&audit),
    [
      ("B1".to_string(), 100.0, Some("Work")),
      ("B2".to_string(), -30.0, Some("Deposit")),
      ("B3".to_string(), -5.0, Some("Discount")),
    ]
  );
  assert!(audit.adds_up);
}

#[test]
fn a_total_shown_as_money_is_the_total_before_a_plain_one_that_reaches_further() {
  // Five days of hours, added up, and two amounts, added up. Nothing reads either sum.
  let book = one(&[
    ("A1", Number(7.5)),
    ("A2", Number(7.5)),
    ("A3", Number(7.5)),
    ("A4", Number(7.5)),
    ("A5", Number(7.5)),
    ("A6", Formula("SUM(A1:A5)", 37.5)),
    ("C1", Money(900.0)),
    ("C2", Money(50.0)),
    ("C3", MoneyFormula("SUM(C1:C2)", 950.0)),
  ]);
  assert_eq!(book.audit().unwrap().total, at(0, "C3"));
}

#[test]
fn of_two_cells_multiplied_with_neither_shown_as_money_the_one_that_is_a_sum_is_how_many() {
  // Hours added up, times a rate typed in as a plain number.
  let audit = one(&[
    ("A1", Text("Rate")),
    ("B1", Number(120.0)),
    ("A2", Text("Monday")),
    ("B2", Number(3.0)),
    ("A3", Text("Tuesday")),
    ("B3", Number(1.5)),
    ("A4", Text("Hours")),
    ("B4", Formula("SUM(B2:B3)", 4.5)),
    ("A5", Text("Labor")),
    ("B5", Formula("B4*B1", 540.0)),
    ("A6", Text("Parts")),
    ("B6", Number(60.0)),
    ("A7", Text("In all")),
    ("B7", Formula("SUM(B5:B6)", 600.0)),
  ])
  .audit()
  .unwrap();
  assert_eq!(
    lines(&audit),
    [
      ("B5".to_string(), 540.0, Some("Labor")),
      ("B6".to_string(), 60.0, Some("Parts")),
    ]
  );
  assert_eq!(
    (audit.lines[0].quantity, audit.lines[0].unit_price),
    (Some(at(0, "B4")), Some(at(0, "B1")))
  );
}
