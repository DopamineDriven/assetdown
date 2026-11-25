//! A workbook read as an invoice.
//!
//! Each visible sheet is laid out as a page, and `contentdown`'s invoice reader reads the
//! pages by their labels, as it reads a PDF's. A workbook also has what a page does not: its
//! formulas. So the total is then taken from them (see `audit`), and the lines it adds up
//! become the invoice's items. The labels say what a sheet's maker called things; the
//! formulas say what is added to what.
//!
//! The two are set against each other. Where the labels gave the same amount, the two
//! agree. Where they gave another, the formulas have it, with two exceptions that leave the
//! labels' reading as it was: a formula total of nought, and one shown as a plain number
//! (hours added up, a count) against a labeled amount shown as money. The labels are never
//! asked which formula is the total: that was tried, and measured, and was wrong far more
//! often than right.

use std::collections::HashMap;

use contentdown::PageLayout;
use contentdown::invoice::{
  FieldSpan, InvoiceDocumentType, InvoiceItem, InvoiceOptions, InvoiceResult, MoneyField,
  NumberField, StringField, document_confidence, extract_from_pages,
};

use crate::address::cell_ref;
use crate::audit::{Audit, AuditLine, CellAt, reads_as_money};
use crate::workbook::{Cell, CellValue, Workbook};

/// How sure a total taken from the formulas is: as sure as a field gets when the labels on
/// the page read the same amount, less when only its lines come to it, and least when they
/// do not, or it has none.
const AGREED: f64 = 0.99;
const ADDS_UP: f64 = 0.9;
const ALONE: f64 = 0.75;
/// How sure a line of a total is, when the lines come to the total and when they do not.
const LINE: f64 = 0.9;
const LINE_ADRIFT: f64 = 0.6;

/// A number as a spreadsheet shows it in full: to fifteen figures, which leaves out the
/// noise that adding in binary puts past them (a tenth and a fifth come to
/// 0.30000000000000004).
fn in_full(value: f64) -> f64 {
  format!("{value:.14e}").parse().unwrap_or(value)
}

/// Whether two amounts are the same to the cent.
fn same(a: f64, b: f64) -> bool {
  (a - b).abs() <= 0.005 + 1e-9
}

/// An amount as its cell shows it, where the cell shows cents or finer: the number the text
/// spells, when that is the amount rounded to the places shown (1,250.38 for a cell that
/// holds 1250.3756). None where the cell shows fewer than two places, and where what it shows
/// is not the amount rounded (a percentage, a number scaled to thousands).
fn as_shown(value: f64, text: &str) -> Option<f64> {
  let last = text.rfind(|c: char| c.is_ascii_digit())?;
  let number = &text[..=last];
  let (whole, places) = number.rsplit_once(['.', ','])?;
  if places.len() < 2 || !places.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  // A comma before three digits groups thousands; before two it stands for the point.
  if number[whole.len()..].starts_with(',') && places.len() != 2 {
    return None;
  }
  let digits: String = whole
    .chars()
    .filter(char::is_ascii_digit)
    .chain(places.chars())
    .collect();
  let scale = 10f64.powi(i32::try_from(places.len()).ok()?);
  let shown = digits.parse::<f64>().ok()? / scale;
  ((shown - value.abs()).abs() <= 0.5 / scale + 1e-9).then(|| shown.copysign(value))
}

/// Where the cells of the visible sheets were laid out: for each sheet, by its place among
/// the workbook's, its page and the box round each cell's words.
struct Places {
  pages: HashMap<usize, (u32, HashMap<String, [f64; 4]>)>,
}

impl Places {
  /// `cells` names, for each page, the cell each of its words was laid out from.
  fn of(workbook: &Workbook, pages: &[PageLayout], cells: &[Vec<String>]) -> Places {
    // The pages are the visible sheets, in the workbook's order.
    let visible = workbook
      .sheets
      .iter()
      .enumerate()
      .filter(|(_, sheet)| !sheet.hidden)
      .map(|(index, _)| index);
    let pages = visible
      .zip(pages.iter().zip(cells))
      .map(|(index, (page, cells))| {
        let mut boxes: HashMap<String, [f64; 4]> = HashMap::new();
        for (word, cell) in page.words.iter().zip(cells) {
          let around = [word.x0, word.y0, word.x1, word.y1];
          boxes
            .entry(cell.clone())
            .and_modify(|held| *held = union(*held, around))
            .or_insert(around);
        }
        (index, (page.page, boxes))
      })
      .collect();
    Places { pages }
  }

  /// The box round the words of the given cells of one sheet. None when the sheet is not
  /// shown, or none of the cells put a word on its page.
  fn span(&self, sheet: usize, cells: impl IntoIterator<Item = CellAt>) -> Option<FieldSpan> {
    let (page, boxes) = self.pages.get(&sheet)?;
    let [x0, y0, x1, y1] = cells
      .into_iter()
      .filter(|cell| cell.sheet == sheet)
      .filter_map(|cell| boxes.get(&cell_ref(cell.row, cell.col)).copied())
      .reduce(union)?;
    Some(FieldSpan {
      page: *page,
      x0,
      y0,
      x1,
      y1,
    })
  }
}

fn union(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
  [
    a[0].min(b[0]),
    a[1].min(b[1]),
    a[2].max(b[2]),
    a[3].max(b[3]),
  ]
}

/// What turns the cells of an audit into the fields of an invoice.
struct Fields<'a> {
  workbook: &'a Workbook,
  places: Places,
  /// The currency the page's reading settled on, for amounts whose cells name none.
  currency: Option<String>,
}

impl<'a> Fields<'a> {
  fn cell(&self, at: CellAt) -> Option<&'a Cell> {
    self
      .workbook
      .sheets
      .get(at.sheet)?
      .cells
      .get(&(at.row, at.col))
  }

  /// A cell as it is named in a warning: `Invoice!D7`.
  fn named(&self, at: CellAt) -> String {
    let sheet = self
      .workbook
      .sheets
      .get(at.sheet)
      .map_or("", |sheet| sheet.name.as_str());
    format!("{sheet}!{}", cell_ref(at.row, at.col))
  }

  fn money(&self, at: CellAt, value: f64, confidence: f64) -> Option<MoneyField> {
    let cell = self.cell(at)?;
    Some(MoneyField {
      value: as_shown(value, &cell.text).unwrap_or_else(|| in_full(value)),
      text: cell.text.clone(),
      confidence,
      span: self.places.span(at.sheet, [at]),
      currency_code: self.currency.clone(),
      currency_symbol: cell.currency_symbol.clone(),
    })
  }

  fn number(&self, at: CellAt, confidence: f64) -> Option<NumberField> {
    let cell = self.cell(at)?;
    let CellValue::Number(value) = cell.value else {
      return None;
    };
    Some(NumberField {
      value: in_full(value),
      text: cell.text.clone(),
      confidence,
      span: self.places.span(at.sheet, [at]),
    })
  }

  /// A line as an item. `turned` is -1 for the lines of a total below nought, which is
  /// reported as an amount, so that the items still come to it.
  fn item(&self, line: &AuditLine, turned: f64, confidence: f64) -> InvoiceItem {
    let price = |at: CellAt| {
      let CellValue::Number(value) = self.cell(at)?.value else {
        return None;
      };
      self.money(at, value, confidence)
    };
    // The item is its row: the words that describe it there, how many, the price of each,
    // and the amount. A heading far above it is not part of it.
    let row = line
      .described_by
      .iter()
      .copied()
      .filter(|cell| cell.row == line.at.row)
      .chain(line.quantity)
      .chain(line.unit_price)
      .chain([line.at]);
    InvoiceItem {
      description: line.description.as_ref().map(|text| StringField {
        value: text.clone(),
        text: text.clone(),
        confidence,
        span: self
          .places
          .span(line.at.sheet, line.described_by.iter().copied()),
      }),
      quantity: line.quantity.and_then(|at| self.number(at, confidence)),
      unit_price: line.unit_price.and_then(price),
      amount: self.money(line.at, turned * line.amount, confidence),
      span: self.places.span(line.at.sheet, row),
      ..InvoiceItem::default()
    }
  }
}

/// Write what the formulas say into what the labels gave. A total of nought says nothing (a
/// form not filled in yet, or charges and credits that cancel), so it changes nothing; and
/// nought is anything under half a cent, which is what adding in binary leaves of it.
fn audited(workbook: &Workbook, places: Places, audit: &Audit, result: &mut InvoiceResult) {
  if same(audit.value, 0.0) {
    return;
  }
  let fields = Fields {
    workbook,
    places,
    currency: result.fields.currency_code.clone(),
  };
  let amount = audit.value.abs();
  let by_labels = result.fields.invoice_total.as_ref();
  let agreed = by_labels.is_some_and(|read| same(read.value, amount));
  let confidence = match (agreed, audit.adds_up) {
    (true, _) => AGREED,
    (false, true) => ADDS_UP,
    (false, false) => ALONE,
  };
  // A total the formulas show as a plain number (hours added up, a count) does not set aside
  // an amount the page labels as the total and shows as money.
  if let Some(read) = by_labels
    && !agreed
    && !audit.shown_as_money
    && (read.currency_symbol.is_some() || reads_as_money(&read.text))
  {
    result.warnings.push(format!(
      "the formulas add up to a total ({}) that is not shown as money, so the amount the labels gave, {}, was kept",
      fields.named(audit.total),
      read.text
    ));
    return;
  }
  let Some(total) = fields.money(audit.total, amount, confidence) else {
    return;
  };
  if let Some(read) = by_labels.filter(|_| !agreed) {
    result.warnings.push(format!(
      "the formulas add up to {} ({}); the labels on the page gave {}, which was set aside",
      total.text,
      fields.named(audit.total),
      read.text
    ));
  }
  if audit.value < 0.0 {
    let signal = format!("negative total: {}", total.text);
    if !result.credit_signals.contains(&signal) {
      result.credit_signals.push(signal);
    }
    result.document_type = InvoiceDocumentType::CreditNote;
  } else if result.document_type == InvoiceDocumentType::Unknown {
    result.document_type = InvoiceDocumentType::Invoice;
  }
  result.fields.invoice_total = Some(total);

  if audit.lines.is_empty() {
    result.warnings.push(format!(
      "the total ({}) is worked out by more than adding cells up, so no lines were read from it",
      fields.named(audit.total)
    ));
  } else if result.fields.items.is_empty() {
    let turned = audit.value.signum();
    let line_confidence = if audit.adds_up { LINE } else { LINE_ADRIFT };
    result.fields.items = audit
      .lines
      .iter()
      .map(|line| fields.item(line, turned, line_confidence))
      .collect();
    if !audit.adds_up {
      let sum: f64 = audit.lines.iter().map(|line| turned * line.amount).sum();
      result.warnings.push(format!(
        "the lines the total adds up come to {sum:.2}, not {amount:.2}"
      ));
    }
  }
  result.confidence = document_confidence(&result.fields, audit.adds_up);
}

impl Workbook {
  /// Invoice fields read from the workbook's visible sheets, each laid out as a page, with
  /// the total and the items taken from the workbook's formulas where they give a total (see
  /// [`Workbook::audit`]): the total is the formula that nothing reads which is shown as
  /// money and reaches the most cells, and the items are the lines it adds up. An amount is given as its cell
  /// shows it, where the cell shows cents. Where the labels on the page gave another total,
  /// a warning says both; where the lines do not come to the total, a warning says what they
  /// come to.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    let (pages, cells): (Vec<PageLayout>, Vec<Vec<String>>) = self
      .layout(None, false, false)
      .into_iter()
      .map(|layout| (layout.page, layout.cells))
      .unzip();
    let mut result = extract_from_pages(&pages, options);
    if let Some(audit) = self.audit() {
      audited(self, Places::of(self, &pages, &cells), &audit, &mut result);
    }
    result
  }
}

#[cfg(test)]
mod tests;
