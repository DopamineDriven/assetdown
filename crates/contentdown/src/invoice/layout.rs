//! The layout track: invoice fields read from `PageLayout` pages, whatever produced them (a
//! native PDF, the OCR cascade, a workbook's logical pages, or a consumer's own layout call).
//! This file is the engine's entry; the modules beside it do the reading.

use super::amounts::{Amount, Separator, has_decimals, parse_amount, written_as_money};
use super::anchors::{Candidate, MAX_VALUE_TOKENS, Seat, candidates, score};
use super::dates::parse_date;
use super::identifiers::{parse_identifier, parse_registration, parse_serial};
use super::lexicon::Hit;
use super::lexicon::{Field, PageRole, role_of_title};
use super::parties::{Party, party_block, vendor};
use super::priors::{Priors, infer};
use super::segments::{Segment, label_hits, segments, span_of};
use super::totals::{TotalCandidate, Totals, resolve};
use crate::invoice::types::{
  AddressField, AircraftReference, DateField, FieldSpan, InvoiceDocumentType, InvoiceFields,
  InvoiceOptions, InvoiceResult, InvoiceSource, MoneyField, StringField,
};
use crate::layout::{PageLayout, TextSource};
use std::collections::HashMap;

pub(crate) const NO_WORDS_WARNING: &str = "no words on the first page; nothing to read";
/// A field scoring under this is withheld (counted in a warning) rather than reported.
pub(crate) const CONFIDENCE_FLOOR: f64 = 0.3;

/// The tier the pages with words came from: one tier when they agree, `Mixed` when they differ,
/// `None` when no page has a word.
fn source_of(pages: &[PageLayout]) -> InvoiceSource {
  let mut tiers: Vec<TextSource> = pages
    .iter()
    .filter(|p| !p.words.is_empty())
    .map(|p| p.source)
    .collect();
  tiers.sort_by_key(|t| *t as u8);
  tiers.dedup();
  match tiers.as_slice() {
    [] => InvoiceSource::None,
    [TextSource::Native] => InvoiceSource::Native,
    [TextSource::Ocr] => InvoiceSource::Ocr,
    [TextSource::Rendered] => InvoiceSource::Rendered,
    _ => InvoiceSource::Mixed,
  }
}

/// A value a validator accepted from a candidate: the normalized value, the validator's score,
/// and how many leading tokens of the candidate it used.
struct Accepted {
  value: String,
  validator: f64,
  tokens_used: usize,
}

/// How an amount on the page is written, most like money first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Written {
  /// With a currency symbol or code beside it.
  Currency,
  /// With two decimals and no currency.
  Decimals,
  /// With no more than its digits grouped in threes.
  Grouped,
}

/// How much an amount is written like a total, most like one first: the order of the variants
/// is the order the last resort takes them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Standing {
  CurrencyRepeated,
  DecimalsRepeated,
  Currency,
  Decimals,
  GroupedRepeated,
  Grouped,
}

/// How much else the pages state beside the last resort's pick. The fewer other amounts there
/// are to mistake it for, the surer the pick; and stating it twice counts for much while they
/// are few, and for nothing once they are many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Contest {
  /// No other amount, and the pick stated twice or more.
  AloneRepeated,
  /// No other amount.
  Alone,
  /// One or two other amounts, and the pick stated twice or more.
  FewRepeated,
  /// One or two other amounts.
  Few,
  /// Three to five other amounts.
  Several,
  /// Six or more other amounts.
  Many,
}

impl Contest {
  fn of(others: usize, repeated: bool) -> Self {
    match others {
      0 if repeated => Contest::AloneRepeated,
      0 => Contest::Alone,
      1..=2 if repeated => Contest::FewRepeated,
      1..=2 => Contest::Few,
      3..=5 => Contest::Several,
      _ => Contest::Many,
    }
  }

  /// How sure the pick is. Every grade is over the floor, so the total is always returned; the
  /// warning beside it is what says it was no label's reading.
  fn confidence(self) -> f64 {
    match self {
      Contest::AloneRepeated => 0.9,
      Contest::Alone | Contest::FewRepeated => 0.8,
      Contest::Few | Contest::Several => 0.5,
      Contest::Many => 0.35,
    }
  }
}

/// An amount the pages state, as the last resort weighs it: how it is written, and how often.
struct Stated {
  amount: Amount,
  text: String,
  times: usize,
  /// The most like money any statement of it is written.
  written: Written,
  /// Where it is first stated: the page's position among the pages, and the word's on it.
  at: (usize, usize),
}

impl Stated {
  /// An amount stated twice or more stands before one stated once, except that digits grouped
  /// in threes and nothing more stand last either way.
  fn standing(&self) -> Standing {
    let repeated = self.times >= 2;
    match self.written {
      Written::Currency if repeated => Standing::CurrencyRepeated,
      Written::Decimals if repeated => Standing::DecimalsRepeated,
      Written::Currency => Standing::Currency,
      Written::Decimals => Standing::Decimals,
      Written::Grouped if repeated => Standing::GroupedRepeated,
      Written::Grouped => Standing::Grouped,
    }
  }
}

/// The engine's working state for one document.
struct Reader<'a> {
  pages: &'a [PageLayout],
  first: Vec<Segment>,
  hits: Vec<Vec<Hit>>,
  /// The last page with words when it is not the first: where multi-page invoices total up.
  last: Vec<Segment>,
  last_hits: Vec<Vec<Hit>>,
  priors: Priors,
  opts: Option<&'a InvoiceOptions>,
  warnings: Vec<String>,
  withheld: usize,
}

impl<'a> Reader<'a> {
  fn page(&self, index: usize) -> Option<&'a PageLayout> {
    self.pages.get(index)
  }

  fn preferred_lang(&self, lang: &str) -> bool {
    match self.opts.and_then(|o| o.languages.as_ref()) {
      Some(langs) => langs.iter().any(|l| l.eq_ignore_ascii_case(lang)),
      None => lang == "en",
    }
  }

  /// The best candidate for `field` under `validate`, which may accept the whole candidate text
  /// or a shorter run of its leading tokens. Returns the value, its text, score and span.
  fn best(
    &mut self,
    field: Field,
    validate: &dyn Fn(&str) -> Option<(String, f64)>,
  ) -> Option<(String, String, f64, FieldSpan)> {
    let cands = candidates(&self.first, &self.hits, field);
    let mut picked: Option<(f64, &Candidate, Accepted)> = None;
    let mut seen: Vec<String> = Vec::new();
    for c in &cands {
      // A value is a handful of tokens at most; try the leading runs from the longest down.
      let tokens: Vec<&str> = c.text.split_whitespace().take(MAX_VALUE_TOKENS).collect();
      let mut accepted: Option<Accepted> = None;
      for k in (1..=tokens.len()).rev() {
        let text = tokens[..k].join(" ");
        if let Some((value, validator)) = validate(&text) {
          accepted = Some(Accepted {
            value,
            validator,
            tokens_used: k,
          });
          break;
        }
      }
      let Some(a) = accepted else { continue };
      let mut total = score(
        c.label_score,
        a.validator,
        c.geometry_score,
        c.ocr_confidence,
      );
      if self.preferred_lang(c.label.lang) {
        total += 0.01;
      }
      if self.opts.and_then(|o| o.include_candidates) == Some(true) {
        seen.push(format!("{} ({:.2})", a.value, total));
      }
      if picked.as_ref().is_none_or(|(s, _, _)| total > *s) {
        picked = Some((total, c, a));
      }
    }
    if !seen.is_empty() {
      self
        .warnings
        .push(format!("candidates for {field:?}: {}", seen.join(" | ")));
    }
    let (total, c, a) = picked?;
    let confidence = total.min(0.99);
    if confidence < CONFIDENCE_FLOOR {
      self.withheld += 1;
      return None;
    }
    let page = self.page(c.page_index)?;
    let used = if a.tokens_used >= c.words.len() {
      c.words.clone()
    } else {
      c.words[..a.tokens_used].to_vec()
    };
    let text = c
      .text
      .split_whitespace()
      .take(a.tokens_used)
      .collect::<Vec<_>>()
      .join(" ");
    Some((a.value, text, confidence, span_of(page, &used)))
  }

  fn string_field(
    &mut self,
    field: Field,
    validate: &dyn Fn(&str) -> Option<String>,
  ) -> Option<StringField> {
    self
      .best(field, &|t| validate(t).map(|v| (v, 1.0)))
      .map(|(value, text, confidence, span)| StringField {
        value,
        text,
        confidence,
        span: Some(span),
      })
  }

  /// Every amount a totals label points at, on the first and the last page.
  fn money_candidates(&self, field: Field) -> Vec<TotalCandidate> {
    let separator: Separator = self.priors.separator;
    let mut out = Vec::new();
    for (segs, hits) in [(&self.first, &self.hits), (&self.last, &self.last_hits)] {
      for c in candidates(segs, hits, field) {
        let tokens: Vec<&str> = c.text.split_whitespace().collect();
        // The whole text; else its last token, else its first. A long remainder is prose, and a
        // number at its end is an amount only when it is written with decimals (`Total hours
        // worked this period 12` is not). The number the rest of the label's own line opens with
        // is an amount only when it is written as money (`Total: 48,250 payable by …` is,
        // `Total: 12 hours worked` is not), and it gives way to an amount at the end, where a row
        // keeps its total. A paragraph below or beside a label is never read for its opening, and
        // nor is a line whose opening is followed by more digits: that is one amount set with
        // its characters apart (`$4 2 , 7 1 0 . 0 0`), and its first piece is not the amount.
        let mut attempts: Vec<(String, Vec<usize>)> = vec![(c.text.clone(), c.words.clone())];
        let last = (
          tokens.last().map(|t| (*t).to_string()).unwrap_or_default(),
          c.words.last().copied().into_iter().collect(),
        );
        let first: (String, Vec<usize>) = (
          tokens.first().map(|t| (*t).to_string()).unwrap_or_default(),
          c.words.first().copied().into_iter().collect(),
        );
        if tokens.len() <= 3 {
          attempts.push(last);
          attempts.push(first);
        } else {
          if has_decimals(&last.0) {
            attempts.push(last);
          }
          let opening = first.0.trim_end_matches([',', ';', ':']);
          let words_follow = tokens
            .get(1)
            .is_some_and(|next| next.starts_with(|ch: char| ch.is_alphabetic() || ch == '('));
          if c.seat == Seat::AfterLabel && words_follow && written_as_money(opening, separator) {
            attempts.push((opening.to_string(), first.1));
          }
        }
        let Some((text, words, amount)) = attempts
          .into_iter()
          .find_map(|(text, words)| parse_amount(&text, separator).map(|a| (text, words, a)))
        else {
          continue;
        };
        let Some(page) = self.page(c.page_index) else {
          continue;
        };
        out.push(TotalCandidate {
          field,
          amount,
          text,
          span: span_of(page, &words),
          base: score(c.label_score, 1.0, c.geometry_score, c.ocr_confidence).min(0.99),
        });
      }
    }
    out
  }

  fn totals(&self) -> Totals {
    let mut items = Vec::new();
    for field in [Field::SubTotal, Field::Tax, Field::Total, Field::AmountDue] {
      items.extend(self.money_candidates(field));
    }
    resolve(&items, self.priors.currency.as_deref())
  }

  /// The total when no label pointed at one: of the amounts the given pages state, the one
  /// written most like a total (`Stated::standing`), and the largest of those. A currency set as a
  /// word of its own before the amount counts as beside it. Nought is never it, and nor is a
  /// lone digit, which is what an amount set with its characters apart leaves. It is a guess and
  /// is returned as one: as sure as its `Contest` allows, with the warning that says so.
  fn last_resort(&self, readable: &[usize]) -> Option<LastResort> {
    let separator = self.priors.separator;
    let mut stated: Vec<Stated> = Vec::new();
    let mut by_amount: HashMap<String, usize> = HashMap::new();
    for &index in readable {
      let Some(page) = self.page(index) else {
        continue;
      };
      for (at, word) in page.words.iter().enumerate() {
        let token = word.text.trim_end_matches([',', ';', ':']);
        let digits = token.chars().filter(char::is_ascii_digit).count();
        if digits == 0 || (digits == 1 && !has_decimals(token)) {
          continue;
        }
        let beside = at
          .checked_sub(1)
          .and_then(|before| page.words.get(before))
          .filter(|before| {
            before.line == word.line
              && before.text.chars().count() <= 3
              && !before.text.chars().any(|c| c.is_ascii_digit())
          })
          .and_then(|before| parse_amount(&format!("{}{token}", before.text), separator))
          .filter(|amount| amount.symbol.is_some() || amount.code.is_some());
        let Some(amount) = beside.or_else(|| {
          written_as_money(token, separator)
            .then(|| parse_amount(token, separator))
            .flatten()
        }) else {
          continue;
        };
        if amount.value == 0.0 {
          continue;
        }
        let written = if amount.symbol.is_some() || amount.code.is_some() {
          Written::Currency
        } else if has_decimals(token) {
          Written::Decimals
        } else {
          Written::Grouped
        };
        match by_amount.get(&format!("{:.2}", amount.value)) {
          Some(&held) => {
            let held = &mut stated[held];
            held.times += 1;
            held.written = held.written.min(written);
            if held.amount.symbol.is_none() && held.amount.code.is_none() {
              held.amount.symbol = amount.symbol;
              held.amount.code = amount.code;
            }
          }
          None => {
            by_amount.insert(format!("{:.2}", amount.value), stated.len());
            stated.push(Stated {
              amount,
              text: token.to_string(),
              times: 1,
              written,
              at: (index, at),
            });
          }
        }
      }
    }
    let found = stated.iter().min_by(|a, b| {
      a.standing()
        .cmp(&b.standing())
        .then(b.amount.value.total_cmp(&a.amount.value))
    })?;
    let page = self.page(found.at.0)?;
    Some(LastResort {
      warning: format!(
        "no label pointed at a total; `{}` is a last resort (the amount written most like one, \
         stated {} time(s)) and wants checking",
        found.text, found.times
      ),
      total: MoneyField {
        value: found.amount.value,
        text: found.text.clone(),
        confidence: Contest::of(stated.len().saturating_sub(1), found.times >= 2).confidence(),
        span: Some(span_of(page, &[found.at.1])),
        currency_code: found
          .amount
          .code
          .clone()
          .or_else(|| self.priors.currency.clone()),
        currency_symbol: found.amount.symbol.clone(),
      },
    })
  }

  /// Credit titles in the top band of the first page.
  fn credit_titles(&self, page_height: f64) -> Vec<String> {
    self
      .first
      .iter()
      .zip(&self.hits)
      .filter(|(s, _)| s.y1 >= 0.75 * page_height)
      .filter(|(_, hits)| {
        hits
          .iter()
          .any(|h| h.start == 0 && h.score >= 0.8 && h.label.field == Field::CreditTitle)
      })
      .map(|(s, _)| format!("credit title: {}", s.text))
      .collect()
  }

  fn date_field(&mut self, field: Field) -> Option<DateField> {
    let order = self.priors.order;
    self
      .best(field, &|t| {
        parse_date(t, order).map(|(v, unambiguous)| (v, if unambiguous { 1.0 } else { 0.8 }))
      })
      .map(|(value, text, confidence, span)| DateField {
        value,
        text,
        confidence,
        span: Some(span),
      })
  }
}

type PartyFields = (
  Option<StringField>,
  Option<AddressField>,
  Option<StringField>,
  Option<StringField>,
  Option<StringField>,
);

/// `(name, address, email, phone, id)`.
fn party_fields(party: Option<Party>) -> PartyFields {
  match party {
    Some(p) => (p.name, p.address, p.email, p.phone, p.id),
    None => (None, None, None, None, None),
  }
}

/// The role a page's top band declares; a page without words has none, a page without a title is
/// the invoice's own. A title must look like one: set larger than the page's median text or
/// bold, which keeps a boxed form's `PURCHASE ORDER` header cell from naming the page. An invoice
/// title on the page wins over any other title.
fn page_role(page: &PageLayout, index: usize) -> Option<PageRole> {
  if page.words.is_empty() {
    return None;
  }
  let segs = segments(page, index);
  let mut sizes: Vec<f64> = segs.iter().map(|s| s.font_size).collect();
  sizes.sort_by(f64::total_cmp);
  let median = sizes.get(sizes.len() / 2).copied().unwrap_or(0.0);
  let mut role = PageRole::Invoice;
  for s in segs
    .iter()
    .filter(|s| s.y1 >= 0.75 * page.height && (s.bold || s.font_size >= 1.2 * median))
  {
    let words: Vec<&str> = s.tokens.iter().map(String::as_str).collect();
    match role_of_title(&words) {
      Some(PageRole::Invoice) => return Some(PageRole::Invoice),
      Some(other) => role = other,
      None => {}
    }
  }
  Some(role)
}

fn role_name(role: PageRole) -> &'static str {
  match role {
    PageRole::Invoice => "invoice",
    PageRole::PurchaseOrder => "purchase order",
    PageRole::Delivery => "delivery note",
    PageRole::Remittance => "remittance advice",
  }
}

/// How sure each key field that was read is: the invoice's number, date, vendor, total, and
/// amount due.
fn key_confidences(fields: &InvoiceFields) -> Vec<f64> {
  [
    fields.invoice_id.as_ref().map(|f| f.confidence),
    fields.invoice_date.as_ref().map(|f| f.confidence),
    fields.vendor_name.as_ref().map(|f| f.confidence),
    fields.invoice_total.as_ref().map(|f| f.confidence),
    fields.amount_due.as_ref().map(|f| f.confidence),
  ]
  .into_iter()
  .flatten()
  .collect()
}

/// How sure a document's reading is, from nought to one: the mean of its key fields'
/// confidences, a tenth more when its amounts were found to add up (`reconciled`), and never
/// quite certain. Nought when no key field was read. A reader that changes a key field after
/// the pages were read (a workbook's total, taken from its formulas) asks again with this.
pub fn document_confidence(fields: &InvoiceFields, reconciled: bool) -> f64 {
  let key = key_confidences(fields);
  if key.is_empty() {
    return 0.0;
  }
  let bonus = if reconciled { 0.1 } else { 0.0 };
  (key.iter().sum::<f64>() / key.len() as f64 + bonus).min(0.99)
}

/// The last resort's guess at a total, and the warning that says it is one.
#[derive(Debug, Clone)]
pub struct LastResort {
  pub total: MoneyField,
  pub warning: String,
}

/// What the pages gave: the fields their labels pointed at and, where no label pointed at a
/// total, the last resort's guess at one. The guess is kept apart so that a reader with more
/// to go on than the page (a workbook, which has its formulas) can ask that first.
#[derive(Debug, Clone)]
pub struct PagesRead {
  pub result: InvoiceResult,
  pub last_resort: Option<LastResort>,
}

impl PagesRead {
  /// The result, with the guess written in as its total where it still has none: a document
  /// with a total is an invoice, and it is as sure as its key fields then are.
  pub fn settled(self) -> InvoiceResult {
    let mut result = self.result;
    if result.fields.invoice_total.is_none()
      && let Some(guess) = self.last_resort
    {
      result.fields.invoice_total = Some(guess.total);
      result.warnings.push(guess.warning);
      if result.document_type == InvoiceDocumentType::Unknown {
        result.document_type = InvoiceDocumentType::Invoice;
      }
      result.confidence = document_confidence(&result.fields, false);
    }
    result
  }
}

/// Header fields from pages: what their labels point at and, where no label points at a
/// total, the last resort's guess at one, marked as a guess (see [`PagesRead::settled`]).
pub fn extract_from_pages(pages: &[PageLayout], opts: Option<&InvoiceOptions>) -> InvoiceResult {
  read_pages(pages, opts).settled()
}

/// Header fields from pages by their labels alone, with the last resort's guess beside them.
pub fn read_pages(pages: &[PageLayout], opts: Option<&InvoiceOptions>) -> PagesRead {
  let source = source_of(pages);
  let mut warnings = Vec::new();
  // Pages play roles: the invoice's own pages carry the fields; our own purchase order, a
  // delivery note or a remittance advice appended to them do not, and are read only for the
  // order number. Roles only ever set pages aside when an invoice page remains: a document
  // that is nothing but such pages is read as it stands, with a warning.
  let roles: Vec<Option<PageRole>> = pages
    .iter()
    .enumerate()
    .map(|(i, p)| page_role(p, i))
    .collect();
  let invoice_pages: Vec<usize> = roles
    .iter()
    .enumerate()
    .filter(|(_, r)| matches!(r, Some(PageRole::Invoice)))
    .map(|(i, _)| i)
    .collect();
  let other_pages: Vec<(usize, PageRole)> = roles
    .iter()
    .enumerate()
    .filter_map(|(i, r)| match r {
      Some(PageRole::Invoice) | None => None,
      Some(role) => Some((i, *role)),
    })
    .collect();
  let only_other_pages = invoice_pages.is_empty() && !other_pages.is_empty();
  let readable: Vec<usize> = if invoice_pages.is_empty() {
    (0..pages.len()).filter(|&i| roles[i].is_some()).collect()
  } else {
    invoice_pages
  };
  let Some(&first_index) = readable.first() else {
    warnings.push(NO_WORDS_WARNING.to_string());
    return PagesRead {
      result: InvoiceResult {
        source,
        document_type: InvoiceDocumentType::Unknown,
        confidence: 0.0,
        fields: InvoiceFields::default(),
        warnings,
        embedded_xml: None,
        currency_source: None,
        credit_signals: Vec::new(),
      },
      last_resort: None,
    };
  };
  let first_page = &pages[first_index];
  if pages.first().is_some_and(|p| p.words.is_empty()) {
    warnings.push(NO_WORDS_WARNING.to_string());
  }
  if only_other_pages {
    warnings.push(format!(
      "every readable page is titled as a {}; it was read as the invoice regardless",
      role_name(other_pages[0].1)
    ));
  } else {
    for (i, role) in &other_pages {
      warnings.push(format!(
        "page {} is a {} and was not read as the invoice",
        pages[*i].page,
        role_name(*role)
      ));
    }
  }
  {
    let mut numbers: Vec<u32> = pages.iter().map(|p| p.page).collect();
    numbers.sort_unstable();
    if numbers.windows(2).any(|w| w[0] == w[1]) {
      warnings.push(
        "page numbers repeat; spans carry the numbers as given and fields were read by position"
          .to_string(),
      );
    }
  }
  let first = segments(first_page, first_index);
  let hits = label_hits(&first);
  let priors = infer(&first, &hits, opts);
  let last_index = readable.last().copied().filter(|&i| i != first_index);
  let last = last_index
    .map(|i| segments(&pages[i], i))
    .unwrap_or_default();
  let last_hits = label_hits(&last);
  let mut reader = Reader {
    pages,
    first,
    hits,
    last,
    last_hits,
    priors,
    opts,
    warnings,
    withheld: 0,
  };

  let invoice_id = reader.string_field(Field::InvoiceId, &parse_identifier);
  let invoice_date = reader.date_field(Field::InvoiceDate);
  let due_date = reader.date_field(Field::DueDate);
  let mut purchase_order = reader.string_field(Field::PurchaseOrder, &parse_identifier);
  // An appended purchase order is the most authoritative source of its own number.
  if purchase_order.is_none() && !only_other_pages {
    for &(i, role) in &other_pages {
      if role != PageRole::PurchaseOrder {
        continue;
      }
      let segs = segments(&pages[i], i);
      let po_hits = label_hits(&segs);
      let mut po_reader = Reader {
        pages,
        first: segs,
        hits: po_hits,
        last: Vec::new(),
        last_hits: Vec::new(),
        priors: reader.priors.clone(),
        opts,
        warnings: Vec::new(),
        withheld: 0,
      };
      purchase_order = po_reader.string_field(Field::PurchaseOrder, &parse_identifier);
      if purchase_order.is_some() {
        break;
      }
    }
  }
  let work_order = reader.string_field(Field::WorkOrder, &parse_identifier);
  let registration = reader.string_field(Field::Registration, &parse_registration);
  let serial = reader.string_field(Field::Serial, &parse_serial);
  let engine_serial = reader.string_field(Field::EngineSerial, &parse_serial);
  let mut aircraft = Vec::new();
  if registration.is_some() || serial.is_some() || engine_serial.is_some() {
    let span = registration
      .as_ref()
      .or(serial.as_ref())
      .or(engine_serial.as_ref())
      .and_then(|f| f.span.clone());
    aircraft.push(AircraftReference {
      registration,
      serial_number: serial,
      engine_serial_number: engine_serial,
      span,
      ..AircraftReference::default()
    });
  }
  let vendor_party = party_block(&reader.first, &reader.hits, Field::Vendor)
    .or_else(|| vendor(&reader.first, &reader.hits, first_page.height));
  let (vendor_name, vendor_address, vendor_email, vendor_phone, _) = party_fields(vendor_party);
  // A labeled party block comes from the first page, else the last: a cover page often names
  // the customer only in an unlabeled block while every itemization page says `Bill To:`.
  let party = |field: Field| {
    party_block(&reader.first, &reader.hits, field)
      .or_else(|| party_block(&reader.last, &reader.last_hits, field))
  };
  let (customer_name, customer_address, _, _, customer_id) = party_fields(party(Field::BillTo));
  let (_, shipping_address, _, _, _) = party_fields(party(Field::ShipTo));
  let (_, remittance_address, _, _, _) = party_fields(party(Field::RemitTo));
  let totals = reader.totals();
  let last_resort = if totals.invoice_total.is_none() {
    reader.last_resort(&readable)
  } else {
    None
  };
  let fields = InvoiceFields {
    sub_total: totals.sub_total,
    total_tax: totals.total_tax,
    invoice_total: totals.invoice_total,
    amount_due: totals.amount_due,
    invoice_id,
    invoice_date,
    due_date,
    purchase_order,
    work_order,
    aircraft,
    vendor_name,
    vendor_address,
    vendor_email,
    vendor_phone,
    customer_name,
    customer_id,
    customer_address,
    shipping_address,
    remittance_address,
    currency_code: reader.priors.currency.clone(),
    ..InvoiceFields::default()
  };

  let mut credit_signals = reader.credit_titles(first_page.height);
  if totals.negative {
    let text = fields
      .invoice_total
      .as_ref()
      .map(|f| f.text.clone())
      .unwrap_or_default();
    credit_signals.push(format!("negative total: {text}"));
  }
  let confidence = document_confidence(&fields, totals.reconciled);
  let document_type = if !credit_signals.is_empty() {
    InvoiceDocumentType::CreditNote
  } else if key_confidences(&fields).is_empty() {
    InvoiceDocumentType::Unknown
  } else {
    InvoiceDocumentType::Invoice
  };
  let mut warnings = std::mem::take(&mut reader.warnings);
  if reader.withheld > 0 {
    warnings.push(format!(
      "{} field(s) withheld under the {CONFIDENCE_FLOOR} confidence floor",
      reader.withheld
    ));
  }
  PagesRead {
    result: InvoiceResult {
      source,
      document_type,
      confidence,
      fields,
      warnings,
      embedded_xml: None,
      currency_source: Some(reader.priors.currency_source),
      credit_signals,
    },
    last_resort,
  }
}

#[cfg(test)]
mod tests;
