//! Invoice extraction as JavaScript sees it: mirrors of `contentdown::invoice`'s types, converted
//! at the boundary. Results are copied out; only the options are read back from JavaScript.

use contentdown::invoice as model;
use napi_derive::napi;

// ── Invoice result shape (shared by every extraction source) ──────

/// Where a field was read from, in PDF user space on a 1-based page.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct FieldSpan {
  pub page: u32,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct StringField {
  pub value: String,
  /// The exact source text the value was derived from.
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct NumberField {
  pub value: f64,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
}

/// `value` is an ISO 8601 calendar date, `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct DateField {
  pub value: String,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
}

/// Money as a JS number plus the exact printed text; `text` is authoritative
/// when cents matter.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct MoneyField {
  pub value: f64,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
  pub currency_code: Option<String>,
  pub currency_symbol: Option<String>,
}

/// `value` is `lines` joined with `\n`.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct AddressField {
  pub value: String,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
  pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct InvoiceItem {
  pub description: Option<StringField>,
  pub product_code: Option<StringField>,
  pub quantity: Option<NumberField>,
  pub unit: Option<StringField>,
  pub unit_price: Option<MoneyField>,
  pub tax_rate: Option<NumberField>,
  pub tax: Option<MoneyField>,
  pub discount: Option<MoneyField>,
  pub date: Option<DateField>,
  pub amount: Option<MoneyField>,
  /// Absent when the item came from embedded XML (no page geometry).
  pub span: Option<FieldSpan>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct TaxDetail {
  pub rate: Option<NumberField>,
  pub amount: Option<MoneyField>,
  pub base: Option<MoneyField>,
  pub category: Option<StringField>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct PaymentDetail {
  pub iban: Option<StringField>,
  pub swift: Option<StringField>,
  pub bank_name: Option<StringField>,
  pub account_number: Option<StringField>,
  pub routing_number: Option<StringField>,
}

/// An aircraft or engine the invoice relates to. Maintenance, parts and
/// program invoices identify the asset by airframe (model, manufacturer serial
/// number, registration) and/or by engine (model, engine serial number); one
/// invoice can cover several assets, so `InvoiceFields.aircraft` is a list.
#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct AircraftReference {
  /// Airframe model, e.g. "Challenger 350", "Global 6000", "Phenom 300".
  pub model: Option<StringField>,
  /// Manufacturer serial number (MSN).
  pub serial_number: Option<StringField>,
  /// Registration / tail number, e.g. "N123AB", "C-GXYZ".
  pub registration: Option<StringField>,
  /// Engine model, e.g. "BR710", "AE 3007", "PW308C".
  pub engine_model: Option<StringField>,
  /// Engine serial number (ESN).
  pub engine_serial_number: Option<StringField>,
  pub span: Option<FieldSpan>,
}

/// Field names mirror the de-facto vocabulary of common invoice extractors so
/// consumers can swap sources with a thin mapper. Naming only: nothing here
/// talks to any service.
#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct InvoiceFields {
  pub vendor_name: Option<StringField>,
  pub vendor_address: Option<AddressField>,
  pub vendor_tax_id: Option<StringField>,
  pub vendor_email: Option<StringField>,
  pub vendor_phone: Option<StringField>,
  pub vendor_website: Option<StringField>,
  pub customer_name: Option<StringField>,
  pub customer_id: Option<StringField>,
  pub customer_address: Option<AddressField>,
  pub customer_tax_id: Option<StringField>,
  pub billing_address: Option<AddressField>,
  pub shipping_address: Option<AddressField>,
  pub remittance_address: Option<AddressField>,
  pub invoice_id: Option<StringField>,
  pub invoice_date: Option<DateField>,
  pub due_date: Option<DateField>,
  pub purchase_order: Option<StringField>,
  pub payment_term: Option<StringField>,
  pub service_start_date: Option<DateField>,
  pub service_end_date: Option<DateField>,
  pub currency_code: Option<String>,
  pub sub_total: Option<MoneyField>,
  pub total_discount: Option<MoneyField>,
  pub total_tax: Option<MoneyField>,
  pub shipping_cost: Option<MoneyField>,
  pub invoice_total: Option<MoneyField>,
  pub amount_due: Option<MoneyField>,
  pub previous_unpaid_balance: Option<MoneyField>,
  /// MRO work order / shop order the invoice bills against.
  pub work_order: Option<StringField>,
  /// Aircraft and engines the invoice relates to; empty when none is identified.
  pub aircraft: Vec<AircraftReference>,
  pub tax_details: Vec<TaxDetail>,
  pub payment_details: Vec<PaymentDetail>,
  pub items: Vec<InvoiceItem>,
}

/// Which extraction path produced the result: embedded XML, the page text (native, OCR,
/// rendered or mixed), or `None` when the document had no readable
/// pages and no embedded XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum InvoiceSource {
  EmbeddedXml,
  Native,
  Ocr,
  Rendered,
  Mixed,
  None,
}

/// Which tier named the currency: evidence on the page (an ISO code or an unambiguous glyph),
/// content priors such as the vendor's country, the caller's option, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum CurrencySource {
  Page,
  Content,
  Option,
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum EInvoiceStandard {
  FacturX,
  ZUGFeRD,
  XRechnung,
  UBL,
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum InvoiceDocumentType {
  Invoice,
  CreditNote,
  Unknown,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct EmbeddedInvoiceXml {
  pub filename: String,
  pub standard: EInvoiceStandard,
  /// The guideline / customization identifier, e.g.
  /// `urn:cen.eu:en16931:2017#compliant#urn:factur-x.eu:1p0:basic`.
  pub profile: Option<String>,
  pub xml: String,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object, object_from_js = false)]
pub struct InvoiceResult {
  pub source: InvoiceSource,
  pub document_type: InvoiceDocumentType,
  /// Document-level confidence, 0..1.
  pub confidence: f64,
  pub fields: InvoiceFields,
  pub warnings: Vec<String>,
  pub embedded_xml: Option<EmbeddedInvoiceXml>,
  /// Which tier named `fields.currencyCode`; absent when no layout pass ran.
  pub currency_source: Option<CurrencySource>,
  /// The evidence behind `documentType: CreditNote`; empty otherwise.
  pub credit_signals: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum DateOrder {
  Auto,
  MDY,
  DMY,
  YMD,
}

/// Options accepted by every invoice entry point. `preferEmbeddedXml` picks the embedded
/// XML over the pages when both exist; `languages`, `currency`, `dateOrder` and
/// `decimalSeparator` steer the page reader where the page leaves a choice;
/// `includeCandidates` lists, in `warnings`, the candidates each field was chosen from;
/// `includeLayout` is reserved.
#[derive(Debug, Clone, Default)]
#[napi(object)]
pub struct InvoiceOptions {
  /// Label lexicons to enable; default `["en"]`.
  pub languages: Option<Vec<String>>,
  /// ISO 4217 hint when the page shows only symbols.
  pub currency: Option<String>,
  pub date_order: Option<DateOrder>,
  #[napi(ts_type = "\"Auto\" | \".\" | \",\"")]
  pub decimal_separator: Option<String>,
  /// Default `true`. Embedded Factur-X / ZUGFeRD / XRechnung / UBL XML is used when present.
  pub prefer_embedded_xml: Option<bool>,
  /// Reserved; accepted and ignored.
  pub include_layout: Option<bool>,
  /// Default `false`. Lists, in `warnings`, the candidates each field was chosen from, with
  /// their scores.
  pub include_candidates: Option<bool>,
}

// ── Conversions ───────────────────────────────────────────────────

impl From<model::InvoiceSource> for InvoiceSource {
  fn from(value: model::InvoiceSource) -> Self {
    match value {
      model::InvoiceSource::EmbeddedXml => InvoiceSource::EmbeddedXml,
      model::InvoiceSource::Native => InvoiceSource::Native,
      model::InvoiceSource::Ocr => InvoiceSource::Ocr,
      model::InvoiceSource::Rendered => InvoiceSource::Rendered,
      model::InvoiceSource::Mixed => InvoiceSource::Mixed,
      model::InvoiceSource::None => InvoiceSource::None,
    }
  }
}

impl From<model::CurrencySource> for CurrencySource {
  fn from(value: model::CurrencySource) -> Self {
    match value {
      model::CurrencySource::Page => CurrencySource::Page,
      model::CurrencySource::Content => CurrencySource::Content,
      model::CurrencySource::Option => CurrencySource::Option,
      model::CurrencySource::Unknown => CurrencySource::Unknown,
    }
  }
}

impl From<model::EInvoiceStandard> for EInvoiceStandard {
  fn from(value: model::EInvoiceStandard) -> Self {
    match value {
      model::EInvoiceStandard::FacturX => EInvoiceStandard::FacturX,
      model::EInvoiceStandard::ZUGFeRD => EInvoiceStandard::ZUGFeRD,
      model::EInvoiceStandard::XRechnung => EInvoiceStandard::XRechnung,
      model::EInvoiceStandard::UBL => EInvoiceStandard::UBL,
      model::EInvoiceStandard::Unknown => EInvoiceStandard::Unknown,
    }
  }
}

impl From<model::InvoiceDocumentType> for InvoiceDocumentType {
  fn from(value: model::InvoiceDocumentType) -> Self {
    match value {
      model::InvoiceDocumentType::Invoice => InvoiceDocumentType::Invoice,
      model::InvoiceDocumentType::CreditNote => InvoiceDocumentType::CreditNote,
      model::InvoiceDocumentType::Unknown => InvoiceDocumentType::Unknown,
    }
  }
}

impl From<model::DateOrder> for DateOrder {
  fn from(value: model::DateOrder) -> Self {
    match value {
      model::DateOrder::Auto => DateOrder::Auto,
      model::DateOrder::MDY => DateOrder::MDY,
      model::DateOrder::DMY => DateOrder::DMY,
      model::DateOrder::YMD => DateOrder::YMD,
    }
  }
}

impl From<DateOrder> for model::DateOrder {
  fn from(value: DateOrder) -> Self {
    match value {
      DateOrder::Auto => model::DateOrder::Auto,
      DateOrder::MDY => model::DateOrder::MDY,
      DateOrder::DMY => model::DateOrder::DMY,
      DateOrder::YMD => model::DateOrder::YMD,
    }
  }
}

impl From<model::FieldSpan> for FieldSpan {
  fn from(value: model::FieldSpan) -> Self {
    FieldSpan {
      page: value.page,
      x0: value.x0,
      y0: value.y0,
      x1: value.x1,
      y1: value.y1,
    }
  }
}

impl From<model::StringField> for StringField {
  fn from(value: model::StringField) -> Self {
    StringField {
      value: value.value,
      text: value.text,
      confidence: value.confidence,
      span: value.span.map(Into::into),
    }
  }
}

impl From<model::NumberField> for NumberField {
  fn from(value: model::NumberField) -> Self {
    NumberField {
      value: value.value,
      text: value.text,
      confidence: value.confidence,
      span: value.span.map(Into::into),
    }
  }
}

impl From<model::DateField> for DateField {
  fn from(value: model::DateField) -> Self {
    DateField {
      value: value.value,
      text: value.text,
      confidence: value.confidence,
      span: value.span.map(Into::into),
    }
  }
}

impl From<model::MoneyField> for MoneyField {
  fn from(value: model::MoneyField) -> Self {
    MoneyField {
      value: value.value,
      text: value.text,
      confidence: value.confidence,
      span: value.span.map(Into::into),
      currency_code: value.currency_code,
      currency_symbol: value.currency_symbol,
    }
  }
}

impl From<model::AddressField> for AddressField {
  fn from(value: model::AddressField) -> Self {
    AddressField {
      value: value.value,
      text: value.text,
      confidence: value.confidence,
      span: value.span.map(Into::into),
      lines: value.lines,
    }
  }
}

impl From<model::InvoiceItem> for InvoiceItem {
  fn from(value: model::InvoiceItem) -> Self {
    InvoiceItem {
      description: value.description.map(Into::into),
      product_code: value.product_code.map(Into::into),
      quantity: value.quantity.map(Into::into),
      unit: value.unit.map(Into::into),
      unit_price: value.unit_price.map(Into::into),
      tax_rate: value.tax_rate.map(Into::into),
      tax: value.tax.map(Into::into),
      discount: value.discount.map(Into::into),
      date: value.date.map(Into::into),
      amount: value.amount.map(Into::into),
      span: value.span.map(Into::into),
    }
  }
}

impl From<model::TaxDetail> for TaxDetail {
  fn from(value: model::TaxDetail) -> Self {
    TaxDetail {
      rate: value.rate.map(Into::into),
      amount: value.amount.map(Into::into),
      base: value.base.map(Into::into),
      category: value.category.map(Into::into),
    }
  }
}

impl From<model::PaymentDetail> for PaymentDetail {
  fn from(value: model::PaymentDetail) -> Self {
    PaymentDetail {
      iban: value.iban.map(Into::into),
      swift: value.swift.map(Into::into),
      bank_name: value.bank_name.map(Into::into),
      account_number: value.account_number.map(Into::into),
      routing_number: value.routing_number.map(Into::into),
    }
  }
}

impl From<model::AircraftReference> for AircraftReference {
  fn from(value: model::AircraftReference) -> Self {
    AircraftReference {
      model: value.model.map(Into::into),
      serial_number: value.serial_number.map(Into::into),
      registration: value.registration.map(Into::into),
      engine_model: value.engine_model.map(Into::into),
      engine_serial_number: value.engine_serial_number.map(Into::into),
      span: value.span.map(Into::into),
    }
  }
}

impl From<model::InvoiceFields> for InvoiceFields {
  fn from(value: model::InvoiceFields) -> Self {
    InvoiceFields {
      vendor_name: value.vendor_name.map(Into::into),
      vendor_address: value.vendor_address.map(Into::into),
      vendor_tax_id: value.vendor_tax_id.map(Into::into),
      vendor_email: value.vendor_email.map(Into::into),
      vendor_phone: value.vendor_phone.map(Into::into),
      vendor_website: value.vendor_website.map(Into::into),
      customer_name: value.customer_name.map(Into::into),
      customer_id: value.customer_id.map(Into::into),
      customer_address: value.customer_address.map(Into::into),
      customer_tax_id: value.customer_tax_id.map(Into::into),
      billing_address: value.billing_address.map(Into::into),
      shipping_address: value.shipping_address.map(Into::into),
      remittance_address: value.remittance_address.map(Into::into),
      invoice_id: value.invoice_id.map(Into::into),
      invoice_date: value.invoice_date.map(Into::into),
      due_date: value.due_date.map(Into::into),
      purchase_order: value.purchase_order.map(Into::into),
      payment_term: value.payment_term.map(Into::into),
      service_start_date: value.service_start_date.map(Into::into),
      service_end_date: value.service_end_date.map(Into::into),
      currency_code: value.currency_code,
      sub_total: value.sub_total.map(Into::into),
      total_discount: value.total_discount.map(Into::into),
      total_tax: value.total_tax.map(Into::into),
      shipping_cost: value.shipping_cost.map(Into::into),
      invoice_total: value.invoice_total.map(Into::into),
      amount_due: value.amount_due.map(Into::into),
      previous_unpaid_balance: value.previous_unpaid_balance.map(Into::into),
      work_order: value.work_order.map(Into::into),
      aircraft: value.aircraft.into_iter().map(Into::into).collect(),
      tax_details: value.tax_details.into_iter().map(Into::into).collect(),
      payment_details: value.payment_details.into_iter().map(Into::into).collect(),
      items: value.items.into_iter().map(Into::into).collect(),
    }
  }
}

impl From<model::EmbeddedInvoiceXml> for EmbeddedInvoiceXml {
  fn from(value: model::EmbeddedInvoiceXml) -> Self {
    EmbeddedInvoiceXml {
      filename: value.filename,
      standard: value.standard.into(),
      profile: value.profile,
      xml: value.xml,
    }
  }
}

impl From<model::InvoiceResult> for InvoiceResult {
  fn from(value: model::InvoiceResult) -> Self {
    InvoiceResult {
      source: value.source.into(),
      document_type: value.document_type.into(),
      confidence: value.confidence,
      fields: value.fields.into(),
      warnings: value.warnings,
      embedded_xml: value.embedded_xml.map(Into::into),
      currency_source: value.currency_source.map(Into::into),
      credit_signals: value.credit_signals,
    }
  }
}

impl From<InvoiceOptions> for model::InvoiceOptions {
  fn from(value: InvoiceOptions) -> Self {
    model::InvoiceOptions {
      languages: value.languages,
      currency: value.currency,
      date_order: value.date_order.map(Into::into),
      decimal_separator: value.decimal_separator,
      prefer_embedded_xml: value.prefer_embedded_xml,
      include_layout: value.include_layout,
      include_candidates: value.include_candidates,
    }
  }
}
