use contentdown::PageLayout;
use contentdown::invoice::{
  FieldSpan, InvoiceDocumentType, InvoiceResult, document_confidence, extract_from_pages,
};

use crate::testing::Holds::{Formula, Money, MoneyFormula, Number, Text, TextFormula};
use crate::testing::{Holds, an_invoice, one, sheet, workbook};
use crate::workbook::Workbook;

/// What the labels on the pages give by themselves, before the formulas are asked.
fn read_from_labels(book: &Workbook) -> InvoiceResult {
  let pages: Vec<PageLayout> = book
    .layout(None, false, false)
    .into_iter()
    .map(|layout| layout.page)
    .collect();
  extract_from_pages(&pages, None)
}

/// Each item as its description, how many, the price of each, and its amount.
type Items<'a> = Vec<(Option<&'a str>, Option<f64>, Option<f64>, Option<f64>)>;

fn items(result: &InvoiceResult) -> Items<'_> {
  result
    .fields
    .items
    .iter()
    .map(|item| {
      (
        item.description.as_ref().map(|field| field.value.as_str()),
        item.quantity.as_ref().map(|field| field.value),
        item.unit_price.as_ref().map(|field| field.value),
        item.amount.as_ref().map(|field| field.value),
      )
    })
    .collect()
}

/// The box round the words laid out from the named cells of the sheet that is page `page`.
fn boxed(book: &Workbook, page: u32, cells: &[&str]) -> Option<FieldSpan> {
  let layouts = book.layout(None, false, false);
  let layout = layouts.iter().find(|layout| layout.page.page == page)?;
  layout
    .page
    .words
    .iter()
    .zip(&layout.cells)
    .filter(|(_, cell)| cells.contains(&cell.as_str()))
    .map(|(word, _)| FieldSpan {
      page,
      x0: word.x0,
      y0: word.y0,
      x1: word.x1,
      y1: word.y1,
    })
    .reduce(|a, b| FieldSpan {
      page,
      x0: a.x0.min(b.x0),
      y0: a.y0.min(b.y0),
      x1: a.x1.max(b.x1),
      y1: a.y1.max(b.y1),
    })
}

/// Parts and labor under a row labeled as the total, then freight, then what is really owed
/// under a label that says nothing of totals.
fn a_total_the_labels_miss() -> Workbook {
  one(&[
    ("A1", Text("Parts")),
    ("B1", Money(40.0)),
    ("A2", Text("Labor")),
    ("B2", Money(20.0)),
    ("A3", Text("Total")),
    ("B3", MoneyFormula("B1+B2", 60.0)),
    ("A4", Text("Freight")),
    ("B4", Money(5.0)),
    ("A5", Text("In all")),
    ("B5", MoneyFormula("B3+B4", 65.0)),
  ])
}

#[test]
fn a_workbook_s_items_are_the_lines_its_total_adds_up() {
  let result = an_invoice().invoice(None);
  assert_eq!(
    items(&result),
    [
      (Some("Bolt"), Some(2.0), Some(10.0), Some(20.0)),
      (Some("Seal"), Some(1.0), Some(15.5), Some(15.5)),
      (Some("Labor"), None, None, Some(24.5)),
      (Some("Tax"), None, None, Some(6.0)),
    ]
  );
  // Each field keeps the text its cell shows.
  let first = &result.fields.items[0];
  assert_eq!(first.quantity.as_ref().unwrap().text, "2");
  assert_eq!(first.unit_price.as_ref().unwrap().text, "10.00");
  assert_eq!(first.amount.as_ref().unwrap().text, "20.00");
  let total = result.fields.invoice_total.as_ref().unwrap();
  assert_eq!((total.value, total.text.as_str()), (66.0, "66.00"));
  assert!(result.warnings.is_empty(), "{:?}", result.warnings);
}

#[test]
fn where_the_labels_gave_another_total_the_formulas_have_it_and_a_warning_says_both() {
  let book = a_total_the_labels_miss();
  // Read by its labels alone, the row labeled as the total is taken for it.
  assert_eq!(
    read_from_labels(&book)
      .fields
      .invoice_total
      .map(|field| field.value),
    Some(60.0)
  );
  let result = book.invoice(None);
  let total = result.fields.invoice_total.as_ref().unwrap();
  assert_eq!((total.value, total.text.as_str()), (65.0, "65.00"));
  assert_eq!(
    items(&result),
    [
      (Some("Parts"), None, None, Some(40.0)),
      (Some("Labor"), None, None, Some(20.0)),
      (Some("Freight"), None, None, Some(5.0)),
    ]
  );
  let said: Vec<&String> = result
    .warnings
    .iter()
    .filter(|warning| warning.contains("65.00") && warning.contains("60.00"))
    .collect();
  assert_eq!(said.len(), 1, "{:?}", result.warnings);
  assert!(said[0].contains("Invoice!B5"), "{}", said[0]);
}

#[test]
fn a_total_is_surest_when_the_labels_read_it_too_then_when_its_lines_come_to_it() {
  let confidence = |book: &Workbook| book.invoice(None).fields.invoice_total.unwrap().confidence;
  // The labels and the formulas both say 66.
  let agreed = an_invoice();
  assert_eq!(
    read_from_labels(&agreed)
      .fields
      .invoice_total
      .map(|field| field.value),
    Some(66.0)
  );
  assert_eq!(confidence(&agreed), 0.99);
  // The labels say otherwise, but the lines come to it.
  assert_eq!(confidence(&a_total_the_labels_miss()), 0.9);
  // Its lines do not come to it.
  let adrift = one(&[
    ("A1", Text("Parts")),
    ("B1", Money(40.0)),
    ("A2", Text("Labor")),
    ("B2", Money(20.0)),
    ("A3", Text("In all")),
    ("B3", MoneyFormula("SUM(B1:B2)", 70.0)),
  ]);
  assert_eq!(confidence(&adrift), 0.75);
}

#[test]
fn lines_that_do_not_come_to_the_total_are_listed_and_a_warning_says_what_they_come_to() {
  let result = one(&[
    ("A1", Text("Parts")),
    ("B1", Money(40.0)),
    ("A2", Text("Labor")),
    ("B2", Money(20.0)),
    ("A3", Text("In all")),
    ("B3", MoneyFormula("SUM(B1:B2)", 70.0)),
  ])
  .invoice(None);
  assert_eq!(
    items(&result),
    [
      (Some("Parts"), None, None, Some(40.0)),
      (Some("Labor"), None, None, Some(20.0)),
    ]
  );
  assert!(
    result
      .warnings
      .iter()
      .any(|warning| warning.contains("60.00") && warning.contains("70.00")),
    "{:?}",
    result.warnings
  );
  // A line of a total it does not come to is less sure than one of a total it does.
  let line = |result: &InvoiceResult| result.fields.items[0].amount.as_ref().unwrap().confidence;
  assert!(line(&result) < line(&an_invoice().invoice(None)));
}

#[test]
fn a_total_worked_out_by_more_than_adding_cells_up_has_no_items_and_a_warning_says_why() {
  let result = one(&[
    ("A1", Text("Parts")),
    ("B1", Money(40.0)),
    ("A2", Text("Labor")),
    ("B2", Money(20.0)),
    ("A3", Text("With tax")),
    ("B3", MoneyFormula("SUM(B1:B2)*1.1", 66.0)),
  ])
  .invoice(None);
  assert_eq!(
    result.fields.invoice_total.as_ref().map(|f| f.value),
    Some(66.0)
  );
  assert!(result.fields.items.is_empty());
  assert!(
    result
      .warnings
      .iter()
      .any(|warning| warning.contains("Invoice!B3") && warning.contains("no lines")),
    "{:?}",
    result.warnings
  );
}

#[test]
fn every_field_read_from_a_cell_says_where_on_its_sheet_s_page_the_cell_is() {
  let book = an_invoice();
  let result = book.invoice(None);
  assert_eq!(
    result.fields.invoice_total.as_ref().unwrap().span,
    boxed(&book, 1, &["D7"])
  );
  let first = &result.fields.items[0];
  assert_eq!(
    first.description.as_ref().unwrap().span,
    boxed(&book, 1, &["A2"])
  );
  assert_eq!(
    first.quantity.as_ref().unwrap().span,
    boxed(&book, 1, &["B2"])
  );
  assert_eq!(
    first.unit_price.as_ref().unwrap().span,
    boxed(&book, 1, &["C2"])
  );
  assert_eq!(
    first.amount.as_ref().unwrap().span,
    boxed(&book, 1, &["D2"])
  );
  // The item is its whole row.
  assert_eq!(first.span, boxed(&book, 1, &["A2", "B2", "C2", "D2"]));
  assert!(first.span.is_some());
}

#[test]
fn a_line_on_another_sheet_is_on_that_sheet_s_page_and_one_on_a_hidden_sheet_is_on_none() {
  let cover = |reads: &'static str, value: f64| {
    sheet(
      "Cover",
      false,
      &[
        ("A1", Text("Freight")),
        ("B1", Money(5.0)),
        ("A2", Text("In all")),
        ("B2", MoneyFormula(reads, value)),
      ],
    )
  };
  let detail = |hidden: bool| {
    sheet(
      "Detail",
      hidden,
      &[("A1", Text("Parts")), ("B1", Money(40.0))],
    )
  };
  // A hidden sheet stands between the two, so the detail is the third sheet and page 2.
  let book = workbook(vec![
    cover("B1+Detail!B1", 45.0),
    sheet("Notes", true, &[("A1", Text("Not shown"))]),
    detail(false),
  ]);
  let result = book.invoice(None);
  assert_eq!(
    items(&result),
    [
      (Some("Freight"), None, None, Some(5.0)),
      (Some("Parts"), None, None, Some(40.0)),
    ]
  );
  let spans: Vec<Option<FieldSpan>> = result
    .fields
    .items
    .iter()
    .map(|item| item.amount.as_ref().unwrap().span.clone())
    .collect();
  assert_eq!(spans, [boxed(&book, 1, &["B1"]), boxed(&book, 2, &["B1"])]);
  assert_eq!(spans[1].as_ref().map(|span| span.page), Some(2));

  let hidden = workbook(vec![cover("B1+Detail!B1", 45.0), detail(true)]);
  let result = hidden.invoice(None);
  assert_eq!(result.fields.items.len(), 2);
  assert_eq!(result.fields.items[1].amount.as_ref().unwrap().span, None);
  assert_eq!(result.fields.items[1].span, None);
}

#[test]
fn a_total_of_nought_leaves_the_reading_of_the_labels_as_it_was() {
  // A form not filled in yet: its formulas are there and come to nothing.
  let book = one(&[
    ("A1", Text("Invoice No")),
    ("B1", Text("A-1001")),
    ("A2", Text("Parts")),
    ("B2", Money(0.0)),
    ("A3", Text("Labor")),
    ("B3", Money(0.0)),
    ("A4", Text("Total")),
    ("B4", MoneyFormula("SUM(B2:B3)", 0.0)),
  ]);
  assert_eq!(book.invoice(None), read_from_labels(&book));
}

#[test]
fn a_workbook_with_no_formulas_is_read_by_its_labels_alone() {
  let book = one(&[
    ("A1", Text("Invoice No")),
    ("B1", Text("A-1001")),
    ("A2", Text("Total")),
    ("B2", Money(66.0)),
  ]);
  let result = book.invoice(None);
  assert_eq!(result, read_from_labels(&book));
  assert_eq!(
    result.fields.invoice_total.as_ref().map(|f| f.value),
    Some(66.0)
  );
}

#[test]
fn a_total_below_nought_is_a_credit_note_for_that_much_and_its_lines_still_come_to_it() {
  let result = one(&[
    ("A1", Text("Parts returned")),
    ("B1", Money(-40.0)),
    ("A2", Text("Restocking")),
    ("B2", Money(5.0)),
    ("A3", Text("In all")),
    ("B3", MoneyFormula("B1+B2", -35.0)),
  ])
  .invoice(None);
  let total = result.fields.invoice_total.as_ref().unwrap();
  // As the reader of labels reports a total: the amount, with the sign told apart.
  assert_eq!((total.value, total.text.as_str()), (35.0, "-35.00"));
  assert_eq!(result.document_type, InvoiceDocumentType::CreditNote);
  assert_eq!(result.credit_signals, ["negative total: -35.00"]);
  assert_eq!(
    items(&result),
    [
      (Some("Parts returned"), None, None, Some(40.0)),
      (Some("Restocking"), None, None, Some(-5.0)),
    ]
  );
}

#[test]
fn amounts_are_the_numbers_a_spreadsheet_shows_in_full_and_not_the_noise_past_them() {
  // What a tenth and a fifth come to in binary is a little over three tenths.
  let result = one(&[
    ("A1", Text("Parts")),
    ("B1", Number(0.1)),
    ("A2", Text("Labor")),
    ("B2", Formula("0.1+0.1", 0.2)),
    ("A3", Text("In all")),
    ("B3", Formula("B1+B2", 0.1 + 0.2)),
  ])
  .invoice(None);
  assert_ne!(0.1 + 0.2, 0.3);
  assert_eq!(
    result.fields.invoice_total.as_ref().map(|f| f.value),
    Some(0.3)
  );
}

#[test]
fn a_workbook_whose_labels_say_nothing_is_still_an_invoice_once_its_formulas_give_a_total() {
  let cells: Vec<(&str, Holds<'_>)> = vec![
    ("B1", Money(40.0)),
    ("B2", Money(20.0)),
    ("B3", MoneyFormula("SUM(B1:B2)", 60.0)),
  ];
  let book = one(&cells);
  let before = read_from_labels(&book);
  assert_eq!(
    (before.document_type, before.confidence),
    (InvoiceDocumentType::Unknown, 0.0)
  );
  let result = book.invoice(None);
  assert_eq!(result.document_type, InvoiceDocumentType::Invoice);
  // As sure as its key fields, and surer for lines that come to the total.
  assert_eq!(result.confidence, document_confidence(&result.fields, true));
  assert!(result.confidence > 0.9, "{}", result.confidence);
}

#[test]
fn a_total_that_is_nought_but_for_the_noise_of_adding_leaves_the_labels_as_they_were() {
  // A charge and the credit that undoes it: in binary, a tenth and a fifth less three
  // tenths is not quite nothing.
  let left_over: f64 = 0.1 + 0.2 - 0.3;
  assert!(left_over != 0.0 && left_over.abs() < 1e-12);
  let book = one(&[
    ("A1", Text("Invoice No")),
    ("B1", Text("A-1001")),
    ("A2", Text("Charge")),
    ("B2", Money(0.1)),
    ("A3", Text("Charge")),
    ("B3", Money(0.2)),
    ("A4", Text("Credit")),
    ("B4", Money(-0.3)),
    ("A5", Text("Balance")),
    ("B5", MoneyFormula("SUM(B2:B4)", left_over)),
  ]);
  assert_eq!(book.invoice(None), read_from_labels(&book));
}

#[test]
fn the_formula_that_comes_to_what_the_labels_read_is_the_total_before_one_that_reaches_further() {
  // A column of hours added up, which reaches the most cells, and the amounts, labeled.
  let book = one(&[
    ("A1", Money(4.0)),
    ("A2", Money(4.0)),
    ("A3", Money(4.0)),
    ("A4", Money(4.0)),
    ("A5", MoneyFormula("SUM(A1:A4)", 16.0)),
    ("C1", Text("Parts")),
    ("D1", Money(40.0)),
    ("C2", Text("Labor")),
    ("D2", Money(20.0)),
    ("C3", Text("Total")),
    ("D3", MoneyFormula("SUM(D1:D2)", 60.0)),
  ]);
  assert_eq!(
    read_from_labels(&book)
      .fields
      .invoice_total
      .map(|field| field.value),
    Some(60.0)
  );
  // Left to themselves the formulas give the longer sum.
  assert_eq!(book.audit().map(|audit| audit.value), Some(16.0));
  let result = book.invoice(None);
  let total = result.fields.invoice_total.as_ref().unwrap();
  assert_eq!((total.value, total.confidence), (60.0, 0.99));
  assert_eq!(
    items(&result),
    [
      (Some("Parts"), None, None, Some(40.0)),
      (Some("Labor"), None, None, Some(20.0)),
    ]
  );
  assert!(result.warnings.is_empty(), "{:?}", result.warnings);
}

#[test]
fn a_formula_total_not_shown_as_money_does_not_set_aside_a_labeled_amount_that_is() {
  // Hours added up by a formula, as a plain number; and the amount owed, typed in.
  let book = one(&[
    ("A1", Number(7.5)),
    ("A2", Number(7.5)),
    ("A3", Number(7.5)),
    ("A4", Number(7.5)),
    ("A5", Number(7.5)),
    ("A6", Formula("SUM(A1:A5)", 37.5)),
    ("C8", Text("Total")),
    ("D8", Money(950.0)),
  ]);
  let by_labels = read_from_labels(&book);
  assert_eq!(
    by_labels.fields.invoice_total.as_ref().map(|f| f.value),
    Some(950.0)
  );
  let result = book.invoice(None);
  assert_eq!(result.fields, by_labels.fields);
  assert!(result.fields.items.is_empty());
  assert!(
    result
      .warnings
      .iter()
      .any(|warning| warning.contains("Invoice!A6") && warning.contains("not shown as money")),
    "{:?}",
    result.warnings
  );
  // With no labeled amount to keep, the formulas are all there is, and they have it.
  let alone = one(&[
    ("A1", Number(4.0)),
    ("A2", Number(4.0)),
    ("A6", Formula("SUM(A1:A2)", 8.0)),
  ])
  .invoice(None);
  assert_eq!(
    alone.fields.invoice_total.as_ref().map(|f| f.value),
    Some(8.0)
  );
}

#[test]
fn an_amount_is_given_as_its_cell_shows_it_where_the_cell_shows_cents() {
  // More places are kept than are shown: a price to the tenth of a cent, times three.
  let result = one(&[
    ("A1", Text("Fasteners")),
    ("B1", Money(12.3456)),
    ("A2", Text("Carriage")),
    ("B2", Money(1238.03)),
    ("A3", Text("In all")),
    ("B3", MoneyFormula("SUM(B1:B2)", 1250.3756)),
  ])
  .invoice(None);
  let total = result.fields.invoice_total.as_ref().unwrap();
  assert_eq!(total.text, "1,250.38");
  assert_eq!(total.value, 1250.38);
  let amounts: Vec<(f64, &str)> = result
    .fields
    .items
    .iter()
    .map(|item| {
      let amount = item.amount.as_ref().unwrap();
      (amount.value, amount.text.as_str())
    })
    .collect();
  assert_eq!(amounts, [(12.35, "12.35"), (1238.03, "1,238.03")]);
  // A cell that shows no cents gives the number it holds.
  let plain = one(&[
    ("A1", Text("Fasteners")),
    ("B1", Number(12.3456)),
    ("A2", Text("In all")),
    ("B2", Formula("SUM(B1:B1)", 12.3456)),
  ])
  .invoice(None);
  assert_eq!(
    plain.fields.invoice_total.as_ref().map(|f| f.value),
    Some(12.3456)
  );
}

#[test]
fn a_total_written_out_as_text_is_the_invoice_s_total_with_the_text_the_sheet_shows() {
  let result = one(&[
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
  ])
  .invoice(None);
  let total = result.fields.invoice_total.as_ref().unwrap();
  assert_eq!((total.value, total.text.as_str()), (1286.5, "USD 1,286.50"));
  assert_eq!(
    items(&result),
    [
      (Some("Parts"), None, None, Some(1200.0)),
      (Some("Carriage"), None, None, Some(86.5)),
    ]
  );
}
