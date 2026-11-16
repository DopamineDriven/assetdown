# @d0paminedriven/pdfdown

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/pdfdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/pdfdown-node.yml)

Rust-powered PDF extraction for Node.js via [napi-rs](https://napi.rs). Extracts per-page text, positioned words and lines, images (as PNG), annotations (links, destinations), structured text (with header/footer detection), AcroForm fields (read-only, one entry per widget placement), embedded files, invoice fields, and metadata with each page size and the paper it is nearest to, from PDF buffers. Pages can also be rendered to PNG on a host that has PDFium. All extraction is parallelized with [rayon](https://github.com/rayon-rs/rayon) for multi-core performance.

An OCR variant is available as [`@d0paminedriven/pdfdown-ocr`](https://www.npmjs.com/package/@d0paminedriven/pdfdown-ocr) for image-only PDF pages (scanned documents, embedded screenshots). It includes all of the APIs below plus Tesseract-based OCR fallback.

## Install

```sh
npm install @d0paminedriven/pdfdown
yarn add @d0paminedriven/pdfdown
pnpm add @d0paminedriven/pdfdown
```

node &ge; 24. The package is an ES module.

## API

### Standalone functions

#### Synchronous

```typescript
export declare function extractTextPerPage(buffer: Buffer): Array<PageText>;
export declare function extractStructuredTextPerPage(
  buffer: Buffer
): Array<StructuredPageText>;
export declare function layoutPerPage(buffer: Buffer): Array<PageLayout>;
export declare function extractImagesPerPage(buffer: Buffer): Array<PageImage>;
export declare function extractAnnotationsPerPage(
  buffer: Buffer
): Array<PageAnnotation>;
export declare function extractFormFieldsPerPage(
  buffer: Buffer
): Array<PageFormField>;
export declare function attachments(buffer: Buffer): Array<Attachment>;
export declare function extractInvoice(
  buffer: Buffer,
  opts?: InvoiceOptions
): InvoiceResult;
export declare function invoiceFromLayout(
  pages: Array<PageLayout>,
  opts?: InvoiceOptions
): InvoiceResult; // the engine on pages from anywhere
export declare function pdfMetadata(buffer: Buffer): PdfMeta;
export declare function pdfDocument(buffer: Buffer): PdfDocument;
export declare function capabilities(): Capabilities; // what this build and this host can do
```

#### Async (libuv thread pool)

Each sync function that takes a buffer has an async counterpart that runs on the libuv thread pool, keeping the main thread free. Rendering is async only.

```typescript
export declare function extractTextPerPageAsync(
  buffer: Buffer
): Promise<Array<PageText>>;
export declare function extractStructuredTextPerPageAsync(
  buffer: Buffer
): Promise<Array<StructuredPageText>>;
export declare function layoutPerPageAsync(
  buffer: Buffer
): Promise<Array<PageLayout>>;
export declare function extractImagesPerPageAsync(
  buffer: Buffer
): Promise<Array<PageImage>>;
export declare function extractAnnotationsPerPageAsync(
  buffer: Buffer
): Promise<Array<PageAnnotation>>;
export declare function extractFormFieldsPerPageAsync(
  buffer: Buffer
): Promise<Array<PageFormField>>;
export declare function attachmentsAsync(
  buffer: Buffer
): Promise<Array<Attachment>>;
export declare function extractInvoiceAsync(
  buffer: Buffer,
  opts?: InvoiceOptions
): Promise<InvoiceResult>;
export declare function invoiceFromLayoutAsync(
  pages: Array<PageLayout>,
  opts?: InvoiceOptions
): Promise<InvoiceResult>;
export declare function pdfMetadataAsync(buffer: Buffer): Promise<PdfMeta>;
export declare function pdfDocumentAsync(buffer: Buffer): Promise<PdfDocument>;
export declare function renderPagesAsync(
  buffer: Buffer,
  opts?: RenderOptions
): Promise<Array<RenderedPage>>;
```

### `PdfDown` class

Parse once, call many. The constructor parses the PDF once and reuses it across all sync method calls; `openAsync` does the same parse off the JavaScript thread. Async methods share the parsed document across worker threads via `Arc` (atomic reference counting) with zero re-parsing overhead.

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer);
  static openAsync(buffer: Buffer): Promise<PdfDown>;
  textPerPage(): Array<PageText>;
  structuredText(): Array<StructuredPageText>;
  layout(): Array<PageLayout>;
  imagesPerPage(): Array<PageImage>;
  annotationsPerPage(): Array<PageAnnotation>;
  formFieldsPerPage(): Array<PageFormField>;
  attachments(): Array<Attachment>;
  invoice(opts?: InvoiceOptions): InvoiceResult;
  metadata(): PdfMeta;
  document(): PdfDocument;
  textPerPageAsync(): Promise<Array<PageText>>;
  structuredTextAsync(): Promise<Array<StructuredPageText>>;
  layoutAsync(): Promise<Array<PageLayout>>;
  imagesPerPageAsync(): Promise<Array<PageImage>>;
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>;
  formFieldsPerPageAsync(): Promise<Array<PageFormField>>;
  attachmentsAsync(): Promise<Array<Attachment>>;
  invoiceAsync(opts?: InvoiceOptions): Promise<InvoiceResult>;
  metadataAsync(): Promise<PdfMeta>;
  documentAsync(): Promise<PdfDocument>;
  renderPagesAsync(opts?: RenderOptions): Promise<Array<RenderedPage>>;
}
```

### Types

```typescript
export interface PageText {
  page: number;
  text: string;
}

export interface StructuredPageText {
  page: number;
  header: string;
  body: string;
  footer: string;
}

// Positioned text. All coordinates are PDF user space in points, y up,
// exactly as the content stream placed them (not shifted to the CropBox
// origin, not rotated by /Rotate).
export interface PageLayout {
  page: number;
  width: number; // CropBox (fallback MediaBox) size in points
  height: number;
  rotate: number; // /Rotate, reported and not applied
  contentRotation: number; // 0 | 90 | 180 | 270: the page frame's turn, the one carrying the most confident OCR text across its images; words, lines and images all share it; the render tier's turn includes /Rotate (always 0 for Native). Displayed coordinates = this frame turned by rotate - contentRotation
  source: TextSource; // "Native" here; "Ocr" | "Rendered" from the OCR package: the deepest cascade tier that ran, native words kept alongside
  words: Array<TextWord>;
  lines: Array<TextLine>;
  images: Array<PlacedImage>;
  warnings: Array<string>;
}

export interface TextWord {
  text: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  line: number; // index into PageLayout.lines
  fontSize?: number; // native only: size in points after the CTM
  fontName?: string; // native only: base font, subset prefix stripped
  bold?: boolean; // native only: name, ForceBold flag or FontWeight >= 600; absent when the font gives no weight evidence (anonymous CIDFont+Fn names)
  italic?: boolean;
  symbolic?: boolean; // native only: set in a barcode font, so the glyphs are bars, not letters
  confidence?: number; // OCR only: 0..1
}

export interface TextLine {
  text: string; // words joined by single spaces
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  firstWord: number; // PageLayout.words[firstWord .. firstWord + wordCount) are this line's words
  wordCount: number;
}

export interface PlacedImage {
  xobjectName: string; // matches PageImage.xobjectName
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export type TextSource = "Native" | "Ocr" | "Rendered";

export interface PageImage {
  page: number;
  imageIndex: number;
  width: number;
  height: number;
  data: Buffer; // PNG-encoded bytes
  colorSpace: string;
  bitsPerComponent: number;
  filter: string;
  xobjectName: string;
  objectId: string;
}

export interface PageAnnotation {
  page: number;
  subtype: string; // "Link", "Text", "Highlight", etc.
  rect: Array<number>; // [x1, y1, x2, y2] bounding box
  uri?: string; // external link URL
  dest?: string; // internal named destination
  content?: string; // tooltip / alt text
}

export type BoxType = "CropBox" | "MediaBox" | "Unknown";

export interface PageBox {
  pageCount: number; // number of pages with these dimensions
  left: number;
  bottom: number;
  right: number;
  top: number;
  width: number;
  height: number;
  boxType: BoxType;
  paperSize: PaperSize; // the standard paper size nearest to width by height, in either orientation
  pages?: Array<number>; // only present on non-dominant entries
}

// A union keyed on `type`, spelled the way a database enum spells it. A standard size
// carries the standard's own dimensions in points, portrait: `width` is the short side.
// Abridged here: there is one member for each of A0 to A10, B0 to B10, LETTER, LEGAL,
// TABLOID, EXECUTIVE and STATEMENT.
export type StandardPaperSize =
  | { type: "A4"; width: number; height: number }
  | { type: "LETTER"; width: number; height: number };

// `UNKNOWN` when no standard size is within 3% on both sides. `closestTo` names the standard
// sizes the page lies between by area: the largest one no larger than the page, then the
// smallest one larger. A side is null when the page is past that end of the list.
export type PaperSize =
  | StandardPaperSize
  | {
      type: "UNKNOWN";
      closestTo: [
        StandardPaperSize | null | undefined,
        StandardPaperSize | null | undefined
      ];
    };

export interface PdfMeta {
  pageCount: number;
  version: string;
  isLinearized: boolean;
  creator?: string;
  producer?: string;
  creationDate?: string;
  modificationDate?: string;
  pageBoxes: Array<PageBox>;
  form?: FormInfo; // present when /AcroForm exists or orphan widgets were found
}

export interface PdfDocument {
  version: string;
  isLinearized: boolean;
  pageCount: number;
  creator?: string;
  producer?: string;
  creationDate?: string;
  modificationDate?: string;
  pageBoxes: Array<PageBox>;
  totalImages: number;
  totalAnnotations: number;
  imagePages: Array<number>;
  annotationPages: Array<number>;
  text: Array<PageText>;
  structuredText: Array<StructuredPageText>;
  images: Array<PageImage>;
  annotations: Array<PageAnnotation>;
  form?: FormInfo;
  formFields: Array<PageFormField>; // one entry per widget placement, sorted by page
  totalFormFields: number; // === formFields.length
  formFieldPages: Array<number>; // sorted unique pages carrying widgets
}

export interface Capabilities {
  ocr: boolean; // false in this package; true in @d0paminedriven/pdfdown-ocr
  render: boolean; // PDFium was found on this host
}

export type RenderMode = "Auto" | "Never" | "Always";

export interface RenderOptions {
  dpi?: number; // default 300, clamped to 72..600
  mode?: RenderMode; // default "Always"
}

export interface RenderedPage {
  page: number;
  width: number; // pixels
  height: number;
  dpi: number;
  data: Buffer; // PNG-encoded bytes
}
```

### Form field types

`PageFormField` is a discriminated union keyed by `type`: one entry per **widget placement**, so a field shown on two pages yields two entries and a radio group yields one entry per option. Join placements of one logical field on `fieldObjectId`. Presentation data (`/DA`, `/Q`, `/MK`, appearance streams) is deliberately not surfaced: pdfdown reports field data and geometry, never how a viewer should draw it, and it never writes to a form.

```typescript
export type PageFormField =
  | {
      type: "Text";
      /* common */ multiline: boolean;
      password: boolean;
      comb: boolean;
      richText: boolean;
      maxLen?: number;
    }
  | {
      type: "Checkbox";
      /* common */ onState?: string;
      exportValue?: string;
      checked: boolean;
    }
  | {
      type: "Radio";
      /* common */ onState?: string;
      exportValue?: string;
      selected: boolean;
    }
  | {
      type: "ComboBox";
      /* common */ options: Array<ChoiceOption>;
      editable: boolean;
    }
  | {
      type: "ListBox";
      /* common */ options: Array<ChoiceOption>;
      multiSelect: boolean;
      values?: Array<string>;
    }
  | { type: "PushButton" /* common */ }
  | { type: "Signature"; /* common */ signed: boolean } // signature data present in /V, not cryptographic validation
  | { type: "Unknown" /* common */ }; // widget annotations no field claims (orphans, or no /AcroForm at all)

// "common" on every variant:
//   page: number;          // 1-based, like PageText / PageImage / PageAnnotation
//   rect: Array<number>;   // [x0, y0, x1, y1] PDF user space, same convention as PageAnnotation.rect
//   objectId: string;      // this widget, "N G obj" like PageImage.objectId ("page P annot I" for an inline dictionary)
//   fieldObjectId: string; // the logical field; identical on every placement of one field
//   name: string;          // fully qualified "parent.kid"
//   partialName: string; widgetIndex: number; widgetCount: number;
//   value?: string; defaultValue?: string; tooltip?: string; mappingName?: string;
//   readOnly: boolean; required: boolean; noExport: boolean; hasActions: boolean

export interface ChoiceOption {
  exportValue: string;
  displayValue: string;
}

export interface FormInfo {
  hasAcroForm: boolean; // /AcroForm present (false when only orphan widgets were found)
  hasXfa: boolean; // /XFA present; XFA is not parsed
  needAppearances: boolean;
  sigFlags: number;
  fieldCount: number; // logical fields, placed + unplaced; may differ from totalFormFields
  widgetCount: number; // placements; always equals totalFormFields
  unplacedFields: Array<UnplacedFormField>; // fields with no widget on any page: data kept, no geometry invented
  truncated: boolean; // the /Kids depth cap (100) was hit: the result is partial
  warnings: Array<string>; // dangling references, non-array /Annots, non-dictionary /Kids entries, depth cap
}

export interface UnplacedFormField {
  fieldObjectId: string;
  name: string;
  partialName: string;
  type: PageFormField["type"];
  value?: string;
  values?: Array<string>;
  defaultValue?: string;
  tooltip?: string;
  mappingName?: string;
  options?: Array<ChoiceOption>;
  maxLen?: number;
  readOnly: boolean;
  required: boolean;
  noExport: boolean;
  hasActions: boolean;
  multiline: boolean;
  password: boolean;
  comb: boolean;
  richText: boolean;
  editable: boolean;
  multiSelect: boolean;
  signed: boolean;
}
```

Buttons keep the spec's two representations apart: `onState` is the appearance state name from the widget's `/AP /N` (absent, never invented, when there is no appearance dictionary) and `exportValue` is the `/Opt` entry that state maps to. `checked` / `selected` follow `/AS`, then `/V`; `value` always reports `/V`, so a stored value and an appearance state that disagree are both visible.

### Utility types

The declarations are self-contained: they import nothing and export only what the binding produces. The keyed views of the form-field union are a line each with `@d0paminedriven/type-utils` (or any equivalent helper):

```typescript
import type { PageFormField } from "@d0paminedriven/pdfdown";
import type { UTR } from "@d0paminedriven/type-utils";

type FormFieldType = PageFormField["type"]; // "Text" | "Checkbox" | …
type FormFieldByType = UTR<PageFormField, "type", true>; // { Text: {…}, Checkbox: {…}, … } without the discriminant
type FormFieldPayload<T extends keyof FormFieldByType = keyof FormFieldByType> =
  {
    [P in T]: FormFieldByType[P];
  }[T]; // one or more variants' payloads, by key

function handle<K extends FormFieldType>(type: K, field: FormFieldByType[K]) {
  /* … */
}
```

### Attachment and invoice types

```typescript
// ── Attachments ──
export interface Attachment {
  name: string;
  mimeType?: string; // from the embedded file stream's /Subtype, e.g. "text/xml"
  size: number; // decoded bytes
  relationship?: string; // /AFRelationship: "Source", "Data", "Alternative", "Supplement", ...
  description?: string;
  page?: number; // 1-based page of a /FileAttachment annotation; absent for document-level files
  data: Buffer;
}

// ── Invoice ──
// Every field carries the parsed value, the exact source text, a 0..1 confidence and,
// when it came from a page, the span it was read from. Embedded XML has no spans.
export interface FieldSpan {
  page: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}
export interface StringField {
  value: string;
  text: string;
  confidence: number;
  span?: FieldSpan;
}
export interface NumberField {
  value: number;
  text: string;
  confidence: number;
  span?: FieldSpan;
}
export interface DateField {
  value: string;
  text: string;
  confidence: number;
  span?: FieldSpan;
} // value is YYYY-MM-DD
export interface MoneyField {
  value: number;
  text: string;
  confidence: number;
  span?: FieldSpan;
  currencyCode?: string;
  currencySymbol?: string;
}
export interface AddressField {
  value: string;
  text: string;
  confidence: number;
  span?: FieldSpan;
  lines: Array<string>;
}

export interface InvoiceItem {
  description?: StringField;
  productCode?: StringField;
  quantity?: NumberField;
  unit?: StringField;
  unitPrice?: MoneyField;
  taxRate?: NumberField;
  tax?: MoneyField;
  discount?: MoneyField;
  date?: DateField;
  amount?: MoneyField;
  span?: FieldSpan;
}
export interface TaxDetail {
  rate?: NumberField;
  amount?: MoneyField;
  base?: MoneyField;
  category?: StringField;
}
// Aircraft maintenance, parts and program invoices identify the asset by airframe and/or engine;
// one invoice may cover several, so `aircraft` is a list.
export interface AircraftReference {
  model?: StringField; // "Challenger 350", "Global 6000", ...
  serialNumber?: StringField; // manufacturer serial number (MSN)
  registration?: StringField; // tail number, "N123AB"
  engineModel?: StringField; // "BR710", "AE 3007", ...
  engineSerialNumber?: StringField; // ESN
  span?: FieldSpan;
}
export interface PaymentDetail {
  iban?: StringField;
  swift?: StringField;
  bankName?: StringField;
  accountNumber?: StringField;
  routingNumber?: StringField;
}

export interface InvoiceFields {
  vendorName?: StringField;
  vendorAddress?: AddressField;
  vendorTaxId?: StringField;
  vendorEmail?: StringField;
  vendorPhone?: StringField;
  vendorWebsite?: StringField;
  customerName?: StringField;
  customerId?: StringField;
  customerAddress?: AddressField;
  customerTaxId?: StringField;
  billingAddress?: AddressField;
  shippingAddress?: AddressField;
  remittanceAddress?: AddressField;
  invoiceId?: StringField;
  invoiceDate?: DateField;
  dueDate?: DateField;
  purchaseOrder?: StringField;
  paymentTerm?: StringField;
  serviceStartDate?: DateField;
  serviceEndDate?: DateField;
  currencyCode?: string;
  subTotal?: MoneyField;
  totalDiscount?: MoneyField;
  totalTax?: MoneyField;
  shippingCost?: MoneyField;
  invoiceTotal?: MoneyField;
  amountDue?: MoneyField;
  previousUnpaidBalance?: MoneyField;
  workOrder?: StringField; // MRO work / shop order the invoice bills against
  aircraft: Array<AircraftReference>;
  taxDetails: Array<TaxDetail>;
  paymentDetails: Array<PaymentDetail>;
  items: Array<InvoiceItem>;
}

export type InvoiceSource =
  | "EmbeddedXml"
  | "Native"
  | "Ocr"
  | "Rendered"
  | "Mixed"
  | "None"; // where the fields came from: embedded XML, or the tier of the pages read
export type CurrencySource = "Page" | "Content" | "Option" | "Unknown"; // which tier named the currency
export type EInvoiceStandard =
  | "FacturX"
  | "ZUGFeRD"
  | "XRechnung"
  | "UBL"
  | "Unknown";
export type InvoiceDocumentType = "Invoice" | "CreditNote" | "Unknown";
export type DateOrder = "Auto" | "MDY" | "DMY" | "YMD";

export interface EmbeddedInvoiceXml {
  filename: string;
  standard: EInvoiceStandard;
  profile?: string; // guideline / customization URN, e.g. "urn:cen.eu:en16931:2017#compliant#urn:factur-x.eu:1p0:basic"
  xml: string;
}

export interface InvoiceResult {
  source: InvoiceSource; // "None" when no page had a word and no XML was embedded
  documentType: InvoiceDocumentType;
  confidence: number; // document-level, 0..1
  fields: InvoiceFields;
  warnings: Array<string>;
  embeddedXml?: EmbeddedInvoiceXml;
  currencySource?: CurrencySource; // present after a layout pass: "Page" (an ISO code or an unambiguous glyph on the page), "Option", or "Unknown"; "Content" (address priors) is a later release
  creditSignals: Array<string>; // why documentType is "CreditNote": "credit title: …", "negative total: …"; empty otherwise
}

export interface InvoiceOptions {
  languages?: Array<string>; // default ["en"]; every lexicon row matches regardless, the option orders ties toward these languages
  currency?: string; // ISO 4217 fallback when the page shows only an ambiguous glyph such as "$"
  dateOrder?: DateOrder; // fallback for numeric dates both of whose parts are 12 or less; a day over 12 on the page decides on its own
  decimalSeparator?: "Auto" | "." | ","; // fallback when no amount on the page shows two decimals
  preferEmbeddedXml?: boolean; // default true
  includeLayout?: boolean; // reserved, accepted and ignored
  includeCandidates?: boolean; // default false; lists in `warnings` the candidates each field was chosen from
}
```

## Usage

### Standalone functions

#### Extract text per page

```typescript
import { readFileSync } from "node:fs";

import { extractTextPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const pages = extractTextPerPage(pdf);

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`);
}
```

#### Extract text per page (async)

```typescript
import { readFile } from "node:fs/promises";

import { extractTextPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const pages = await extractTextPerPageAsync(pdf);

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`);
}
```

#### Extract structured text with header/footer detection

Splits each page into `header`, `body`, and `footer` sections by detecting repeated lines across pages. Lines that appear at the same position (top or bottom) on >= 60% of pages are classified as headers or footers. Page numbers and other varying digits are normalized during comparison, so "Page 1" and "Page 42" are treated as the same line. A document of fewer than three pages has nothing to compare, so all of its text is `body`.

```typescript
import { readFileSync } from "node:fs";

import { extractStructuredTextPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const pages = extractStructuredTextPerPage(pdf);

for (const { page, header, body, footer } of pages) {
  console.log(`Page ${page}:`);
  if (header) console.log(`  Header: ${header}`);
  console.log(`  Body: ${body.slice(0, 100)}...`);
  if (footer) console.log(`  Footer: ${footer}`);
}
```

#### Extract structured text (async)

```typescript
import { readFile } from "node:fs/promises";

import { extractStructuredTextPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const pages = await extractStructuredTextPerPageAsync(pdf);

// Filter out headers/footers for clean text extraction
const cleanText = pages.map((p) => p.body).join("\n\n");
```

#### Extract images as PNG

```typescript
import { readFileSync } from "node:fs";

import { extractImagesPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const images = extractImagesPerPage(pdf);

for (const img of images) {
  const dataUrl = `data:image/png;base64,${img.data.toString("base64")}`;
  console.log(
    `Page ${img.page} image ${img.imageIndex}: ${img.width}x${img.height} ${img.colorSpace}`
  );
}
```

#### Extract images as PNG (async)

```typescript
import { readFile } from "node:fs/promises";

import { extractImagesPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const images = await extractImagesPerPageAsync(pdf);

for (const img of images) {
  const dataUrl = `data:image/png;base64,${img.data.toString("base64")}`;
  console.log(
    `Page ${img.page} image ${img.imageIndex}: ${img.width}x${img.height} ${img.colorSpace}`
  );
}
```

#### Extract annotations

```typescript
import { readFileSync } from "node:fs";

import { extractAnnotationsPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const annots = extractAnnotationsPerPage(pdf);

for (const annot of annots) {
  if (annot.uri) {
    console.log(`Page ${annot.page}: ${annot.uri}`);
  }
  if (annot.dest) {
    console.log(`Page ${annot.page}: internal link to ${annot.dest}`);
  }
}
```

#### Extract annotations (async)

```typescript
import { readFile } from "node:fs/promises";

import { extractAnnotationsPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const annots = await extractAnnotationsPerPageAsync(pdf);

const links = annots.filter((a) => a.uri);
console.log(
  `Found ${links.length} external links across ${new Set(links.map((a) => a.page)).size} pages`
);
```

#### Attachments and embedded e-invoices

`attachments()` lists every embedded file: the `/Names /EmbeddedFiles` tree, PDF/A-3 `/AF` associated files, and `/FileAttachment` annotations, each decoded once. `extractInvoice()` turns an invoice into typed fields with confidence and provenance. Embedded Factur-X, ZUGFeRD, XRechnung (CII or UBL) or plain UBL XML is read first and is exact, so every field reports confidence 1 and no page span. Without such XML the pages are read (next section), and `source` names the tier they came from.

#### Invoices from pages

`invoice()` on a `PdfDown`, `extractInvoice(buffer)`, and `invoiceFromLayout(pages)` on `PageLayout` pages from anywhere (your own `layoutWithOcr()` call included) run one engine over the first page and, for the totals, the last page. It knows no vendor and no template: a reviewed lexicon of label spellings in English, German, French and Spanish (`Invoice No`, `Rechnung Nr.`, `No de facture`, `Total Due`, `Rechnungsbetrag`, `Montant total`, tail and serial numbers, work orders) is matched on a normalised form that folds case, punctuation and OCR confusables, and a label's value is read from the rest of its line, the aligned line below, or the next column on the same baseline, through validators for dates, amounts and identifiers. Every field carries `text` (what was printed), `value` (normalised: dates as `YYYY-MM-DD`, amounts as numbers, identifiers verbatim minus the label and trailing punctuation), a `span` on its page and a `confidence` that combines the label match, the validator, the geometry and, for OCR words, their Tesseract confidence. A field under 0.3 is withheld and counted in a warning; `includeCandidates: true` lists every candidate per field in the warnings.

What this release reads: `invoiceId`, `invoiceDate`, `dueDate`, `purchaseOrder`, `workOrder`, one `aircraft` entry with the registration, serial and engine serial found, `vendorName` with its address, email and phone (the `From` block, else the largest non-title line in the top quarter of the page), `customerName` and `customerAddress` (`Bill To`, `Invoiced To`, `Sold To` …), `shippingAddress`, `remittanceAddress`, `currencyCode`, `subTotal`, `totalTax`, `invoiceTotal` and `amountDue`. Totals reconcile: the combination where subtotal plus tax (one line or several summed) equals the total within a cent or 1% wins and lifts each participant's confidence; an amount-due line stands in for a missing total; without a reconciliation the lowest `Total` on the page is the invoice total. A `Credit Memo` or `Credit Note` title in the top band, or a negative total, makes `documentType` `"CreditNote"` with the evidence in `creditSignals`; credit amounts are reported absolute. Currency, date order and decimal separator are read off the page first (an ISO code or an unambiguous glyph such as `€`; a day over 12; a token like `1.234,56`), then taken from the options, then defaulted; `$` alone is a glyph, never a code, and `currencySource` says which tier decided. Line items, multi-page continuation, page roles and bundle segmentation are later slices; `fields.items` is empty on the layout track.

In the OCR package, `invoice()` runs the OCR cascade on the first and last page only, so a long scan does not cost a whole-document OCR; `layoutWithOcr()` keeps reading every page.

```typescript
import { readFile } from "node:fs/promises";

import { attachments, extractInvoiceAsync } from "@d0paminedriven/pdfdown";

const buf = await readFile("invoice.pdf");

for (const a of attachments(buf)) {
  console.log(
    a.name,
    a.mimeType,
    a.relationship ?? "-",
    `${a.size} bytes`,
    a.page ? `(page ${a.page})` : ""
  );
}

const invoice = await extractInvoiceAsync(buf);
if (invoice.source === "EmbeddedXml") {
  const { fields: f } = invoice;
  console.log(`${f.vendorName?.value} -> ${f.customerName?.value}`);
  console.log(
    `${f.invoiceId?.value} dated ${f.invoiceDate?.value}, due ${f.dueDate?.value}`
  );
  console.log(
    `total ${f.invoiceTotal?.text} ${f.currencyCode}, ${f.items.length} line item(s)`
  );
  console.log(
    `standard ${invoice.embeddedXml?.standard}, profile ${invoice.embeddedXml?.profile}`
  );
} else {
  console.warn(invoice.warnings.join("\n"));
}
```

Money fields expose both `value` (a JS number) and `text` (the exact printed string). Use `text` when cents must survive.

#### Get PDF metadata

```typescript
import { readFileSync } from "node:fs";

import { pdfMetadata } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const meta = pdfMetadata(pdf);

console.log(
  `v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`
);
```

#### Get PDF metadata (async)

```typescript
import { readFile } from "node:fs/promises";

import { pdfMetadataAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const meta = await pdfMetadataAsync(pdf);

console.log(
  `v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`
);
```

#### Page bounding boxes and paper sizes

`pageBoxes` on `PdfMeta` and `PdfDocument` returns deduplicated page dimensions. Uniform PDFs (all pages the same size) return a single entry. Mixed-size PDFs return one entry per distinct geometry — the dominant (most frequent) entry has `pages` absent, while non-dominant entries list their specific page numbers.

Each entry also names the standard paper size its page is nearest to, in either orientation, as a union keyed on `type`: `{ type: "LETTER", width: 612, height: 792 }` for an 8.5 by 11 inch page, `{ type: "A4", width: 595, height: 842 }` for an A4 one. The dimensions on `paperSize` are the standard's, portrait; the page's own are on the box. A page that is within 3% of no standard size is `UNKNOWN`, with the two standard sizes it lies between by area.

```typescript
import { readFileSync } from "node:fs";

import { pdfMetadata } from "@d0paminedriven/pdfdown";

const meta = pdfMetadata(readFileSync("document.pdf"));
const [box] = meta.pageBoxes;

if (meta.pageBoxes.length === 1 && box) {
  console.log(
    `Uniform: ${box.width}x${box.height} ${box.boxType} ${box.paperSize.type} (${box.pageCount} pages)`
  );
} else {
  for (const box of meta.pageBoxes) {
    const scope = box.pages
      ? `pages ${box.pages.join(", ")}`
      : "all other pages";
    console.log(
      `${box.width}x${box.height} ${box.boxType} ${box.paperSize.type} — ${scope}`
    );
  }
}
```

#### Extract form fields per page

Read-only. One entry per widget placement, sorted by page, then top-to-bottom, left-to-right. Encrypted PDFs with an empty user password are decrypted on load.

```typescript
import { readFileSync } from "node:fs";

import { extractFormFieldsPerPage, pdfMetadata } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("application.pdf");

for (const field of extractFormFieldsPerPage(pdf)) {
  switch (field.type) {
    case "Text":
      console.log(`p${field.page} ${field.name} = ${field.value ?? ""}`);
      break;
    case "Checkbox":
      console.log(
        `p${field.page} ${field.name} ${field.checked ? "[x]" : "[ ]"}`
      );
      break;
    case "Radio":
      if (field.selected)
        console.log(`p${field.page} ${field.name} -> ${field.exportValue}`);
      break;
    default:
      console.log(`p${field.page} ${field.type} ${field.name}`);
  }
}

// Group placements back into logical fields
const byField = Map.groupBy(
  extractFormFieldsPerPage(pdf),
  (f) => f.fieldObjectId
);

// Document-level facts: fields without a widget, truncation, warnings
const form = pdfMetadata(pdf).form;
if (form?.unplacedFields.length)
  console.log(
    "hidden fields:",
    form.unplacedFields.map((u) => u.name)
  );
if (form?.truncated || form?.warnings.length)
  console.log("partial extraction:", form.warnings);
```

`pdfDocument()` carries the same data as `formFields`, `totalFormFields`, `formFieldPages` and `form`, mirroring `images` / `totalImages` / `imagePages`.

### `PdfDown` class

The class-based API parses the PDF once in the constructor. Sync methods reuse the parsed document directly (zero re-parsing). Async methods share the parsed document across libuv worker threads via `Arc` — no data copying, no re-parsing.

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));

// Sync — reuses the already-parsed document
const text = pdf.textPerPage();
const images = pdf.imagesPerPage();
const annots = pdf.annotationsPerPage();
const structured = pdf.structuredText();
const meta = pdf.metadata();

// Async — shares parsed document via Arc across worker threads
const [asyncText, asyncImages, asyncAnnots, asyncStructured] =
  await Promise.all([
    pdf.textPerPageAsync(),
    pdf.imagesPerPageAsync(),
    pdf.annotationsPerPageAsync(),
    pdf.structuredTextAsync()
  ]);
```

### Combined: text + images + links for multimodal embeddings

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));

const [text, images, annots] = await Promise.all([
  pdf.textPerPageAsync(),
  pdf.imagesPerPageAsync(),
  pdf.annotationsPerPageAsync()
]);

// Group images and links by page
const imagesByPage = Map.groupBy(images, (img) => img.page);
const linksByPage = Map.groupBy(
  annots.filter((a) => a.uri),
  (a) => a.page
);

// Build per-page payloads for multimodal embeddings
for (const { page, text: pageText } of text) {
  const payload = {
    page,
    text: pageText,
    images: (imagesByPage.get(page) ?? []).map((img) => ({
      dataUrl: `data:image/png;base64,${img.data.toString("base64")}`,
      width: img.width,
      height: img.height
    })),
    links: (linksByPage.get(page) ?? []).map((a) => a.uri)
  };
  // Send payload to Voyage AI, pgvector, etc.
}
```

### Combined: structured text for clean content pipelines

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));
const pages = await pdf.structuredTextAsync();

// Strip headers/footers for clean RAG ingestion
const chunks = pages
  .map(({ page, body }) => ({
    page,
    content: body.trim()
  }))
  .filter((c) => c.content.length > 0);
```

### Positioned text: words and lines with boxes

`extractTextPerPage` returns text in content-stream order, which is whatever order the generator happened to paint it in (a header drawn last comes out last). `layoutPerPage` interprets the content stream instead: it tracks the graphics and text state, measures each glyph with the font's `/Widths` (or built-in Standard 14 metrics), groups glyphs into words and words into baseline-clustered lines, and returns everything top-to-bottom, left-to-right with boxes in points. Form XObjects are entered through their `/Matrix`, and image XObjects report where they were painted so a scan's OCR words can be mapped back onto the page.

```typescript
import { readFileSync } from "node:fs";

import { layoutPerPage } from "@d0paminedriven/pdfdown";

const [page] = layoutPerPage(readFileSync("invoice.pdf"));
if (!page) throw new Error("the document has no pages");

// Reading order, regardless of how the generator ordered the stream.
for (const line of page.lines) {
  console.log(`${line.y1.toFixed(0).padStart(4)}  ${line.text}`);
}

// Key/value anchoring: the value to the right of a label on the same line.
const labelled = (label: string) => {
  const anchor = page.words.find((w) => w.text === label);
  if (!anchor) return undefined;
  return page.words
    .filter((w) => w.line === anchor.line && w.x0 > anchor.x1)
    .map((w) => w.text)
    .join(" ");
};
console.log(labelled("Invoice"), labelled("Total"));

// Column detection: words whose left edge lines up with a header cell.
const amountHeader = page.words.find((w) => w.text === "Amount" && w.bold);
const amounts = amountHeader
  ? page.words.filter(
      (w) => Math.abs(w.x0 - amountHeader.x0) < 2 && w.y1 < amountHeader.y0
    )
  : [];
```

Runs in the same order of time as `extractTextPerPage` (single-digit milliseconds for a typical invoice page). For scanned pages use `extractLayoutWithOcrPerPage` from the OCR package, which fills the same shape from Tesseract word boxes: a page is OCR'd when its native text is under `minTextLength` or covers under 2% of a page that is mostly image or heavy with paths, native words stay on such a hybrid page, every page's coordinates share one frame (`contentRotation`), and raster orientation is settled by Tesseract's orientation detector before any blind retry. The OCR package README describes the cascade.

### Rendering pages

`renderPagesAsync`, on the class or as a standalone function, rasterizes pages to PNG with [PDFium](https://pdfium.googlesource.com/pdfium/). It is async only.

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown, capabilities } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));

if (capabilities().render) {
  for (const { page, width, height, dpi, data } of await pdf.renderPagesAsync({
    dpi: 150
  })) {
    console.log(
      `page ${page}: ${width}x${height} px at ${dpi} dpi, ${data.length} bytes of PNG`
    );
  }
}
```

| Option | Default    | Meaning                                                                                                                                                                                                             |
| ------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dpi`  | `300`      | Clamped to 72 through 600                                                                                                                                                                                           |
| `mode` | `"Always"` | `"Always"` renders every page. `"Auto"` renders only the pages that have no text and no image this package can decode, which are the pages only a renderer can show. `"Never"` renders nothing and returns no pages |

PDFium is not bundled. It is loaded the first time it is needed, from the library file named by the `PDFIUM_LIBRARY_PATH` environment variable when that is set, otherwise from the system's library search path. `capabilities().render` says whether it was found. When it was not, `renderPagesAsync` rejects with `PDFium library not found. Install PDFium or set PDFIUM_LIBRARY_PATH.` Every other reader works without it. The WebAssembly build cannot load a native library, so rendering is not available there.

## Other formats

This package reads PDFs. Its siblings read other formats with the same shape of API (a class that parses once, free functions, `...Async` forms, `layout()` and `invoice()`):

| Format | Package                                                                              |
| ------ | ------------------------------------------------------------------------------------ |
| xlsx   | [`@d0paminedriven/xlsxdown`](https://www.npmjs.com/package/@d0paminedriven/xlsxdown) |
| docx   | [`@d0paminedriven/docxdown`](https://www.npmjs.com/package/@d0paminedriven/docxdown) |
| pptx   | [`@d0paminedriven/pptxdown`](https://www.npmjs.com/package/@d0paminedriven/pptxdown) |

A buffer that is not a PDF is refused with `Failed to load PDF: …`.

## Parallelism

All extraction functions use [rayon](https://github.com/rayon-rs/rayon) for automatic multi-core parallelism. Per-page text extraction, image decoding, and annotation parsing run concurrently across CPU cores. The `pdfDocument` / `document()` call additionally runs text, image, annotation and form extraction concurrently via nested `rayon::join`. This is internal to the native addon — the Node.js API surface is unchanged.

In the OCR package, OCR runs on a **dedicated capped thread pool** (default 4 threads, configurable via `maxThreads`) to prevent oversubscription when running inside a libuv worker thread, and the package pins Tesseract's own OpenMP to one thread per worker (`OMP_THREAD_LIMIT=1` unless you set it) so those workers do not thrash each other. Non-OCR extraction uses the global rayon pool.

### Tuning Node.js worker threads

Node's libuv thread pool defaults to 4 threads. If you're calling multiple async extraction functions concurrently, you may want to increase it:

```bash
UV_THREADPOOL_SIZE=8 node app.js
```

## Supported formats

| Filter         | Description                   | Handling                                   |
| -------------- | ----------------------------- | ------------------------------------------ |
| DCTDecode      | JPEG-compressed images        | Decoded and re-encoded as PNG              |
| JPXDecode      | JPEG 2000 images              | Decoded and re-encoded as PNG              |
| CCITTFaxDecode | Group 3 and Group 4 fax scans | Decoded in pure Rust and re-encoded as PNG |
| FlateDecode    | Zlib-compressed raw pixels    | Decompressed, reconstructed as PNG         |
| None           | Uncompressed raw pixels       | Reconstructed as PNG                       |

| ColorSpace          | Channels                   |
| ------------------- | -------------------------- |
| DeviceRGB, CalRGB   | 3                          |
| DeviceGray, CalGray | 1                          |
| DeviceCMYK          | 4 (converted to RGB)       |
| ICCBased            | Inferred from /N parameter |

8-bit and 16-bit BitsPerComponent are both supported (16-bit downscaled to 8-bit for PNG output).

## How it works

**Layout** (`layoutPerPage`): a content-stream interpreter tracks `q`/`Q`, `cm`, the text object and text state (`BT`/`ET`, `Tm`, `Td`, `TD`, `T*`, `TL`, `Tf`, `Tc`, `Tw`, `Tz`, `Ts`) and the show operators (`Tj`, `TJ`, `'`, `"`), placing each glyph with the text rendering matrix. Advances come from `/Widths` (simple fonts), `/W` + `/DW` (Type0 composite fonts, two-byte codes), `/FontMatrix` (Type3) or Standard 14 metrics when the file omits widths; ascent/descent from the font descriptor. Glyphs become words at space glyphs, `Tw` gaps, `TJ` adjustments of 200/1000 em or more, baseline changes and horizontal gaps over 0.25 em; words become lines by baseline clustering within half the median word height. Form XObjects are recursed with a depth cap; images are recorded as the unit square under the CTM at `Do`.

Built with [lopdf](https://github.com/J-F-Liu/lopdf) (pure Rust PDF parser), [image](https://github.com/image-rs/image) (PNG/JPEG encoding), [hayro-jpeg2000](https://crates.io/crates/hayro-jpeg2000) (JPEG 2000 decoding), [hayro-ccitt](https://crates.io/crates/hayro-ccitt) (fax decoding), [rayon](https://github.com/rayon-rs/rayon) (data parallelism) and, for rendering only, [pdfium-render](https://crates.io/crates/pdfium-render) (bindings to a PDFium found at run time). Compiled to a native Node.js addon via [napi-rs](https://napi.rs), with a WebAssembly build for platforms that have no prebuilt binary.

## Supported platforms

| Platform      | Architecture(s)   |
| ------------- | ----------------- |
| macOS         | x64, ARM64        |
| Windows       | x64, ia32, ARM64  |
| Linux (glibc) | x64, ARM64, ARMv7 |
| Linux (musl)  | x64, ARM64        |
| FreeBSD       | x64               |
| Android       | ARM64, ARMv7      |
| WASI          | wasm32            |

## License

MIT
