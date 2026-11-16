# pdfdown port: plan

Status: plan with your decisions of 2026-11-14 folded in. **Not started; waiting for your go.** Nothing in the workspace has been changed. `~/rust/napi-rs` was only read, plus `cargo` builds and tests that wrote to a scratch target directory.

Names of new functions and types in the snippets are proposals. Snippets marked "today" are quoted from pdfdown as it is.

## 1. Decisions

| # | Question | Decided | In your words |
| --- | --- | --- | --- |
| 1 | How is the OCR npm package built? | One binding crate, `pdfdown-node`, with an `ocr` feature | "we can not do that, that's fine, it's just more complexity" |
| 2 | Does `@d0paminedriven/pdfdown` keep opening workbooks? | No. pdfdown holds no xlsx functionality; what it has today transfers to `xlsxdown` and is not lost (section 4.9) | "that's solely for xlsxdown to absorb. so it shouldn't be lost, just transferred over" |
| 3 | Is the page model its own crate? | Yes, named `pdfdown-meta` | "layoutdown could become frankenstein" |
| 4 | Where does invoice extraction live? | Every format package exposes `invoice()` itself, so nothing extra is installed. Behind them, the format-agnostic extractor is one Rust crate (`contentdown`, or the name you choose) that each format crate consumes | "I would not be thrilled to have to install separate invoice variants for each document type"; "we could make a crate for just that. contentdown or something" |
| 5 | Can OCR be built without `render`? | Yes. `render` stays optional, and nothing about it is removed | "ocr should optionally accept render" |
| 6 | Must `index.d.ts` match 0.12.0? | No. A minor version bump follows the migration | "the index.d.ts need not be exact" |

Still open, each with the default I will use unless you say otherwise:

| Open point | Default |
| --- | --- |
| What `pdfdown-meta` holds beyond the page model (section 2) | Page model only |
| Paper-size tolerance: 3% or 3.5% (section 5.2) | 3% |
| Rolling total: excludes or includes the current page (section 5.1) | Excludes, so it is the page's start offset |
| New `revisionCount` field (section 5.3) | Not added unless you want it |
| Image output when nothing is asked for (section 5.4) | Source format: original bytes for JPEG and JPEG 2000, PNG for the rest |

## 2. Target layout

```text
crates/
├── ooxmldown/            exists
├── xlsxdown/             exists
├── xlsxdown-node/        exists      publishes @d0paminedriven/xlsxdown
├── contentdown/          written     the page of words, the line builder, invoice extraction
│   └── src/   lib.rs  layout.rs  words.rs  invoice.rs  invoice/
├── pdfdown-meta/         placeholder PDF metadata, to be written
│   └── src/   lib.rs
├── pdfdown/              new
│   └── src/   lib.rs  pdf.rs  error.rs  text.rs  images.rs  forms.rs  meta.rs
│              attachments.rs  document.rs  render.rs  layout/  invoice.rs
├── pdfdown-ocr/          new
│   └── src/   lib.rs  (ocr.rs and its six test modules)
└── pdfdown-node/         new         publishes @d0paminedriven/pdfdown
    ├── src/   lib.rs  types.rs  tasks.rs  ocr.rs (feature "ocr")
    ├── package.json
    └── ocr/package.json              publishes @d0paminedriven/pdfdown-ocr
```

What each crate depends on:

```text
contentdown    roxmltree
pdfdown-meta   lopdf (when written)
pdfdown        contentdown, pdfdown-meta, lopdf, image, flate2, hayro-ccitt,
               hayro-jbig2, hayro-jpeg2000, rayon, thiserror
                                                    feature "render": pdfium-render
pdfdown-ocr    pdfdown, contentdown, lopdf, image, rayon, tesseract-rs
                                                    feature "render": pdfdown/render
pdfdown-node   pdfdown, contentdown, napi, napi-derive (build: napi-build)
               feature "render": pdfdown/render     feature "ocr": pdfdown-ocr
xlsxdown       contentdown, ooxmldown, roxmltree, thiserror
```

What goes in `pdfdown-meta`, to keep it from growing into a catch-all:

| | Items | Why |
| --- | --- | --- |
| In | `PageLayout`, `TextWord`, `TextLine`, `PlacedImage`, `TextSource`, `Word`, `FontMeta`, `words_to_lines` | The page model that both `pdfdown` and `pdfdown-ocr` produce |
| Your call | `PageBox`, `BoxType`, the new `PaperSize` and its matcher | Self-contained page geometry with no parsing; the crate's name would fit them |
| Out | `PdfMeta` | It embeds `FormInfo`, which would drag every form type in |
| Out | Forms, invoice, attachment and image types | They belong to the code that produces them |

Rule for the crate: plain data and pure functions over it. No lopdf, no parsing, no dependencies.

Where each part of pdfdown goes:

| pdfdown path | Destination | Notes |
| --- | --- | --- |
| `types/layout.rs`; `Word` and `words_to_lines` from `core/layout/words.rs`; `FontMeta` from `core/layout/interpreter.rs` | `pdfdown-meta` | `glyphs_to_words` stays in `pdfdown`: it reads PDF glyphs |
| `core/attachments.rs`, `forms.rs`, `images.rs`, `meta.rs`, `text.rs`, `layout/*` | `pdfdown` | |
| `core/document.rs` | `pdfdown`; `extract_all_with_ocr` to `pdfdown-ocr` | |
| `core/render.rs` | `pdfdown`, feature `render` | Off by default, so a plain Rust user does not pull in pdfium |
| `core/invoice/**` | `pdfdown::invoice` | Moves as it is, minus the one spreadsheet branch (section 4.7) |
| `core/ocr.rs` and its six test modules | `pdfdown-ocr` | Also `pdf_invoice_pages` (OCR on the first and last page), which is in `lib.rs` today |
| `core/spreadsheet/**` | `ooxmldown` and `xlsxdown` | Reading is already there. The workbook-to-page projection (about 200 lines) transfers to `xlsxdown` as its own unit (section 4.9) |
| `types/mod.rs`, `forms.rs`, `invoice.rs` | Plain structs in `pdfdown` and `pdfdown-ocr`; napi mirrors in `pdfdown-node` | |
| `lib.rs` | `pdfdown-node` | `PdfState` and `load_doc` become `pdfdown::Pdf` |

## 3. Manifests

These are yours to write; the sketches show what each needs.

Root `Cargo.toml`, to add under `[workspace.dependencies]`, plus the four crates in `members`:

```toml
pdfium-render = { version = "0.9.5", default-features = false, features = [
  "pdfium_latest",
  "image_025",
  "thread_safe"
] }
# 0.4 defaults to `build-tesseract`, which downloads and compiles Leptonica and Tesseract from
# source at build time. Link the system libraries instead (libtesseract-dev, libleptonica-dev).
tesseract-rs = { version = "0.4.0", default-features = false, features = [
  "use-system-tesseract"
] }

# JBIG2 image decoding (pure Rust, no unsafe); section 5.4
hayro-jbig2 = { version = "0.4.0" }

pdfdown      = { path = "crates/pdfdown" }
pdfdown-meta = { path = "crates/pdfdown-meta" }
pdfdown-ocr  = { path = "crates/pdfdown-ocr" }
```

`crates/pdfdown-meta/Cargo.toml`:

```toml
[package]
description            = "Plain data types for PDF pages: the positioned-text page model shared by pdfdown and pdfdown-ocr"
edition.workspace      = true
license.workspace      = true
name                   = "pdfdown-meta"
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

# No [dependencies]: plain data and pure functions over it, nothing else.
```

`crates/pdfdown/Cargo.toml` (the `[package]` table follows the same pattern):

```toml
[dependencies]
flate2.workspace         = true
hayro-ccitt.workspace    = true
hayro-jbig2.workspace    = true
hayro-jpeg2000.workspace = true
image.workspace          = true
lopdf.workspace          = true
pdfdown-meta.workspace   = true
pdfium-render            = { workspace = true, optional = true }
rayon.workspace          = true
roxmltree.workspace      = true # e-invoice XML in the invoice module
thiserror.workspace      = true

[features]
render = ["dep:pdfium-render"]
```

`crates/pdfdown-ocr/Cargo.toml`:

```toml
[dependencies]
image.workspace        = true
lopdf.workspace        = true
pdfdown.workspace      = true
pdfdown-meta.workspace = true
rayon.workspace        = true
tesseract-rs.workspace = true

[features]
render = ["pdfdown/render"]
```

`crates/pdfdown-node/Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
napi.workspace         = true
napi-derive.workspace  = true
pdfdown.workspace      = true
pdfdown-meta.workspace = true
pdfdown-ocr            = { workspace = true, optional = true }

[features]
ocr    = ["dep:pdfdown-ocr"]
render = ["pdfdown/render", "pdfdown-ocr?/render"]

[build-dependencies]
napi-build.workspace = true
```

The build commands stay what they are today: `napi build --features render` for the base package, and `napi build --features ocr,render --package-json-path ocr/package.json --output-dir ocr` for the OCR package.

Versioning: every crate versions on its own, so a fix in one does not force a release of the others.

- No `version` goes in `[workspace.package]`, and no member writes `version.workspace = true`. The keys that are inherited (`edition`, `license`, `repository`, `rust-version`) are metadata only.
- `pdfdown` and `pdfdown-ocr` continue from 0.12.0, where the npm packages are today. `pdfdown-meta` is new and starts at 0.1.0.
- The two npm packages keep separate `package.json` files, so their versions can move separately. One coupling is built in: the OCR package contains `pdfdown`, so a fix in `pdfdown` reaches OCR users only when the OCR package is released too. A fix in `pdfdown-ocr` alone needs only the OCR package.

Notes on the root manifest:

- `optional = true` cannot be written in `[workspace.dependencies]`; it goes on the member, as above.
- The comment `# CII / UBL e-invoice XML (pure Rust, WASI-safe)` now sits above `lopdf`; it belongs to `roxmltree`.
- `xlsxdown-node = { path = ... }` is listed, but nothing can depend on it, since it is a `cdylib` only.

## 4. The changes, in code

### 4.1 Types leave napi behind

Today (`types/layout.rs`), one struct serves both the core and the JS boundary:

```rust
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct TextWord {
  pub text: String,
  pub x0: f64,
  // ...
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum TextSource { Native, Ocr, Rendered, Spreadsheet }
```

After, the library keeps the same struct without the attribute:

```rust
// pdfdown-meta/src/layout.rs
#[derive(Debug, Clone, PartialEq)]
pub struct TextWord {
  pub text: String,
  pub x0: f64,
  // ...
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSource { Native, Ocr, Rendered }
```

and the binding carries a mirror with a conversion:

```rust
// pdfdown-node/src/types.rs
#[napi(object)]
pub struct TextWord {
  pub text: String,
  pub x0: f64,
  // ...
}

impl From<pdfdown_meta::TextWord> for TextWord {
  fn from(word: pdfdown_meta::TextWord) -> Self {
    TextWord { text: word.text, x0: word.x0 /* ... */ }
  }
}

#[napi(string_enum)]
pub enum TextSource { Native, Ocr, Rendered }

impl From<pdfdown_meta::TextSource> for TextSource {
  fn from(source: pdfdown_meta::TextSource) -> Self {
    match source {
      pdfdown_meta::TextSource::Native => TextSource::Native,
      // ...
    }
  }
}
```

Scale: 36 object shapes, 8 string enums and one tagged union. Conversion from JS is needed only for the types JS passes in (`PageLayout` and its parts, `InvoiceOptions`, `OcrOptions`, `RenderOptions`).

`PageFormField` is the one that needs care. It is a napi structured enum stamped by the `page_form_field!` macro, which already works around a napi-derive quirk with optional fields. The library keeps a plain enum; the macro moves to the binding and stamps the mirror and its conversion.

### 4.2 The `Raw*` twins disappear

Today (`types/mod.rs`), each type that carries bytes exists twice:

```rust
#[napi(object)]
pub struct PageImage { pub page: u32, /* ... */ pub data: Buffer }

pub struct RawPageImage { pub page: u32, /* ... */ pub data: Vec<u8> } // for worker threads

impl From<RawPageImage> for PageImage { /* field by field */ }
```

After, the library type is the raw one, and the pair becomes library plus mirror:

```rust
// pdfdown
pub struct PageImage { pub page: u32, /* ... */ pub data: Vec<u8> }

// pdfdown-node
#[napi(object)]
pub struct PageImage { pub page: u32, /* ... */ pub data: Buffer }

impl From<pdfdown::PageImage> for PageImage { /* field by field; data.into() */ }
```

The same happens to `RawAttachment`, `RawPdfDocument`, `RawPdfDocumentOcr` and `RawRenderedPage`.

### 4.3 Entry point and errors

Today, the parsed document and the loader live in the binding (`lib.rs`), and core functions return napi's `Result` without ever failing:

```rust
pub struct PdfState {
  doc: Arc<Document>,
  raw: Arc<Vec<u8>>,
}

fn load_doc(buf: &[u8]) -> Result<Document> {
  Document::load_mem(buf).map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))
}

// core/text.rs
pub(crate) fn extract_text(doc: &Document) -> Result<Vec<PageText>> { /* always Ok */ }
```

After, the library has an entry point of its own, the way `Workbook::open` is for `xlsxdown`:

```rust
// pdfdown/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
  #[error("Failed to load PDF: {0}")]
  Load(#[from] lopdf::Error),
}

// pdfdown/src/pdf.rs
pub struct Pdf {
  document: lopdf::Document,
  bytes: Vec<u8>, // OCR and render re-read the original bytes
}

impl Pdf {
  pub fn open(bytes: Vec<u8>) -> Result<Self, PdfError> { /* ... */ }
  pub fn document(&self) -> &lopdf::Document { &self.document }
  pub fn bytes(&self) -> &[u8] { &self.bytes }

  pub fn text(&self) -> Vec<PageText> { /* infallible */ }
  pub fn structured_text(&self) -> Vec<StructuredPageText> { /* ... */ }
  pub fn layout(&self) -> Vec<PageLayout> { /* ... */ }
  // images, annotations, metadata, attachments, form_fields, document, invoice
}
```

The message `Failed to load PDF: {e}` is kept, so JS callers see the same error text.

### 4.4 Items OCR needs become public

`pdfdown-ocr` is a separate crate, so what it reaches into has to be `pub`. Today:

```rust
// core/layout/mod.rs
pub(crate) struct NativeLayout { /* ... */ }
pub(crate) fn native_layout(doc: &Document, page_num: u32, page_id: ObjectId) -> NativeLayout
```

After:

```rust
// pdfdown/src/layout/mod.rs
pub struct NativeLayout { /* ... */ }
pub fn native_layout(doc: &Document, page_num: u32, page_id: ObjectId) -> NativeLayout
```

The full list:

| Module | Items |
| --- | --- |
| `images` | `collect_page_decoded_images_named`, `page_path_op_count`, `page_has_form_xobjects`, `page_has_image_xobjects`, `extract_images_raw` |
| `layout` | `NativeLayout`, `PageGeometry`, `native_layout`, `page_geometry`, `page_layout`, `extract_layout` |
| `text` | `detect_headers_footers`, `strip_footer_artifacts` |
| `render` | `render_page_to_image_from_bytes`, `ensure_pdfium_with_path`, `normalize_dpi`, the render-mode constants |
| `forms`, `meta`, `document` | `extract_forms`, `form_field_pages`, `extract_metadata_with_form`, `extract_annotations` |

### 4.5 The class and its tasks

Today (`lib.rs`), the class holds a PDF or a workbook and every method first checks which:

```rust
#[napi]
pub struct PdfDown {
  inner: Arc<ParsedDocument>, // enum: Pdf(PdfState) | Workbook(WorkbookState)
}

#[napi]
pub fn text_per_page(&self) -> Result<Vec<PageText>> {
  extract_text(&self.pdf("textPerPage")?.doc)
}

pub struct SharedExtractTextTask(Arc<ParsedDocument>);

#[napi]
impl Task for SharedExtractTextTask {
  type Output = Vec<PageText>;
  type JsValue = Vec<PageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_text(&pdf_of(&self.0, "textPerPageAsync")?.doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
```

After, with decision 2 the class only ever holds a PDF, so `ParsedDocument`, `pdf_of`, `workbook_of` and the unsupported-operation errors go away:

```rust
#[napi]
pub struct PdfDown {
  inner: Arc<Pdf>,
}

#[napi]
impl PdfDown {
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    let pdf = Pdf::open(buffer.to_vec()).map_err(|e| Error::from_reason(e.to_string()))?;
    Ok(PdfDown { inner: Arc::new(pdf) })
  }

  #[napi]
  pub fn text_per_page(&self) -> Vec<PageText> {
    self.inner.text().into_iter().map(PageText::from).collect()
  }

  #[napi]
  pub fn text_per_page_async(&self) -> AsyncTask<SharedExtractTextTask> {
    AsyncTask::new(SharedExtractTextTask(Arc::clone(&self.inner)))
  }
}

pub struct SharedExtractTextTask(Arc<Pdf>);

#[napi]
impl Task for SharedExtractTextTask {
  type Output = Vec<pdfdown::PageText>; // plain Rust on the worker thread
  type JsValue = Vec<PageText>;         // napi mirror

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.0.text())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(PageText::from).collect())
  }
}
```

The task is declared before the method that returns it, in the same crate. That is what keeps the generated typing `Promise<Array<PageText>>` (section 9.2).

The OCR methods stay behind the feature, in the same crate, as today:

```rust
// pdfdown-node/src/ocr.rs
#[cfg(feature = "ocr")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn text_with_ocr_per_page(&self, opts: Option<OcrOptions>) -> Result<Vec<OcrPageText>> { /* ... */ }
  // layout_with_ocr, document_ocr, and the three Async twins
}
```

### 4.6 The OCR crate and `render`

Today (`core/ocr.rs`), behind `cfg(feature = "ocr")` in the one crate:

```rust
pub(crate) fn extract_layout_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
  only_pages: Option<&[u32]>,
) -> Result<Vec<PageLayout>>
```

After, the same function is public in its own crate. The `cfg(feature = "ocr")` gates disappear, because the crate is the feature. The `cfg(feature = "render")` gates stay exactly as they are, with `render` forwarding to `pdfdown/render`:

```rust
// pdfdown-ocr/src/lib.rs
pub fn extract_layout_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
  only_pages: Option<&[u32]>,
) -> Vec<PageLayout>

#[cfg(feature = "render")]
fn render_page_words(/* ... */) { /* unchanged */ }
```

The only signature change is the return type: this function never produces an error today, so the napi `Result` goes.

`OcrOptions` keeps its two shapes (with and without the render fields), in the binding, as today.

### 4.7 The invoice module

It moves as a directory, `core/invoice/` to `pdfdown/src/invoice/`, with imports updated and no change to its logic:

```rust
// before
use crate::core::attachments::extract_attachments;
use crate::types::{EmbeddedInvoiceXml, InvoiceOptions, InvoiceResult, InvoiceSource, PageLayout};

// after
use crate::attachments::extract_attachments;
use crate::invoice::types::{EmbeddedInvoiceXml, InvoiceOptions, InvoiceResult, InvoiceSource};
use pdfdown_meta::PageLayout;
```

One branch changes. The extractor reports what kind of pages it read, and one arm names workbook pages:

```rust
// today (core/invoice/layout.rs)
match tiers.as_slice() {
  [] => InvoiceSource::None,
  [TextSource::Native] => InvoiceSource::Native,
  [TextSource::Ocr] => InvoiceSource::Ocr,
  [TextSource::Rendered] => InvoiceSource::Rendered,
  [TextSource::Spreadsheet] => InvoiceSource::Spreadsheet, // goes
  _ => InvoiceSource::Mixed,
}
```

`TextSource::Spreadsheet` is no longer in `pdfdown-meta`, so that arm goes. Whether `InvoiceSource` keeps a value for pages that did not come from a PDF is the open point in section 4.9.

### 4.8 What leaves the npm package

The shape of the change to `@d0paminedriven/pdfdown`'s typings (illustrative, not the exact generated text):

```diff
 export declare class PdfDown {
   constructor(buffer: Buffer)
   static openAsync(buffer: Buffer): Promise<PdfDown>
-  get format(): DocumentFormat
-  spreadsheetCells(opts?: SpreadsheetOptions): Array<SheetCells>
-  spreadsheetCellsAsync(opts?: SpreadsheetOptions): Promise<Array<SheetCells>>
-  layout(opts?: SpreadsheetOptions): Array<PageLayout>
+  layout(): Array<PageLayout>
   textPerPage(): Array<PageText>
   // ...
 }
-export declare function spreadsheetCells(buffer: Buffer, opts?: SpreadsheetOptions): Array<SheetCells>
-export declare function layoutFromXlsx(buffer: Buffer, opts?: SpreadsheetOptions): Array<PageLayout>
-export declare function detectFormat(buffer: Buffer): FormatDetection
-export interface SheetCells { /* ... */ }
-export interface SpreadsheetOptions { /* ... */ }
```

Each removed function also loses its `Async` twin. Passing a workbook to `new PdfDown(...)` becomes a load error.

Three smaller things go with them, because they only ever described workbook pages: `TextWord.cell`, `PageLayout.sheet`, and the `'Spreadsheet'` value of `TextSource`. They are already out of `pdfdown-meta`.

### 4.9 What transfers to xlsxdown

Nothing xlsx is dropped; it moves to the package that owns the format.

| In pdfdown today | Goes to | State |
| --- | --- | --- |
| The workbook reader and `spreadsheetCells` | `xlsxdown`, `XlsxDown.spreadsheetCells` | Done |
| `layoutFromXlsx` and `PdfDown.layout()` on a workbook: each sheet projected to a page of positioned words and lines, each word carrying its cell reference | `xlsxdown`, as `Workbook::layout` and `XlsxDown.layout()` | Unit 6 |
| `PdfDown.invoice()` on a workbook | Open point below | |
| `detectFormat` | Not transferred: routing happens before any parser is called (below) | Closed |

The projection is about 200 lines of pdfdown's `spreadsheet/project.rs` that never made it across, because it needed the page types. In `xlsxdown` it gets types of its own, and this is where the sheet name and cell reference live:

```rust
// xlsxdown/src/layout.rs
pub struct SheetPage {
  pub page: u32,
  /// The sheet this logical page projects.
  pub sheet: String,
  pub width: f64,
  pub height: f64,
  pub words: Vec<SheetWord>,
  pub lines: Vec<SheetLine>,
  pub warnings: Vec<String>,
}

pub struct SheetWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub line: u32,
  pub font_size: Option<f64>,
  pub font_name: Option<String>,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  /// The reference of the cell this word was laid out from (`"C3"`); the anchor for a merged range.
  pub cell: String,
}

impl Workbook {
  pub fn layout(&self, options: &SheetOptions) -> Vec<SheetPage> { /* ... */ }
}
```

The projection groups words into lines with the same clustering `words_to_lines` does. `xlsxdown` gets its own copy of that function (about 90 lines) so it does not depend on a PDF crate. Check for the unit: output compared with pdfdown 0.12.0's `layoutFromXlsx` on the corpus workbooks, as was done for the cells.

**Invoice extraction: one extractor crate, surfaced by every format package.** The concern is what a consumer has to install, and a Rust crate is not something a consumer installs. It is compiled into each package's binary.

| | What exists | What a consumer sees |
| --- | --- | --- |
| npm packages | `@d0paminedriven/pdfdown`, `@d0paminedriven/xlsxdown`, later docx | One install per document type; each has `invoice()` |
| Rust crates behind them | `pdfdown`, `xlsxdown`, and the shared `contentdown` | Nothing |

```ts
new PdfDown(buffer).invoice();  // pdfdown hands its pages to contentdown
new XlsxDown(buffer).invoice(); // xlsxdown lays the sheets out and hands those pages to contentdown
```

`contentdown` is the format-agnostic part: pdfdown's 3,800-line extractor reads a page of positioned words and never asks where the page came from. It holds no PDF and no workbook code, so document types are not mixed in it; each format crate turns its own document into pages and calls it.

```text
contentdown    roxmltree                       the page of words, the line builder, the invoice extractor
pdfdown        contentdown, lopdf, ...         invoice() = this PDF's attachments and pages, handed to contentdown
xlsxdown       contentdown, ooxmldown          layout() and invoice() for a workbook
```

- One function in pdfdown's invoice module touches lopdf (it lists a PDF's attachments to look for embedded XML). That one stays in `pdfdown`; the rest moves unchanged.
- To keep the name from becoming a catch-all, the rule for the crate would be: it reads pages of positioned text and returns structured content, and it never opens a file.

Its manifest, with the page of words living in it. The whole invoice module uses one external crate, and nothing else in pdfdown uses that crate, so `roxmltree` moves from `pdfdown`'s dependencies to here:

```toml
[package]
description            = "Structured content from pages of positioned text: the page model, the line builder and invoice extraction"
edition.workspace      = true
license.workspace      = true
name                   = "contentdown"
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[dependencies]
roxmltree.workspace = true # e-invoice XML (CII / UBL)
```

Root manifest: `contentdown = { path = "crates/contentdown" }` and the member entry. Consumers each add `contentdown.workspace = true`: `pdfdown`, `pdfdown-ocr`, `pdfdown-node` and `xlsxdown`. If the page of words stays in `pdfdown-meta` instead, this manifest gains `pdfdown-meta.workspace = true`.

**Where the page of words lives: in `contentdown`, done.** The extractor's input is the page type, so it sits next to the code that reads it, and `xlsxdown` does not depend on a crate named for PDF to build a page:

| Crate | Today | After |
| --- | --- | --- |
| `contentdown` | Does not exist | The page of words, the line builder (the three files written in unit 1, moved), and the invoice extractor |
| `pdfdown-meta` | The page of words | PDF metadata, which is what its name says: version, page count, Info strings, encryption, linearization, page boxes, paper size. Also the base for the small metadata package you floated |

If you would rather not carry `pdfdown-meta` at all for now, it can go and the metadata stays in `pdfdown`.

**Workbook layout: rows, columns and metadata.** For `xlsxdown`'s unit, the shape you described, a map of rows to a map of columns, so a whole row or column can be taken at once:

```ts
// today: one flat array per sheet
const sheet = new XlsxDown(buffer).spreadsheetCells()[0];
sheet.cells; // Array<SheetCell>

// proposed, alongside it
sheet.grid;                      // Record<string, Record<string, SheetCell>>, row then column
sheet.grid["12"]["C"];           // one cell
Object.values(sheet.grid["12"]); // a whole row
workbook.column("Invoice", "C"); // a whole column, straight from Rust
```

- napi turns a Rust map into a plain object, typed `Record<K, V>`, not a JS `Map`. `new Map(Object.entries(grid))` converts it if a `Map` is wanted.
- Rust already stores a sheet's cells keyed by row and column, so rows and columns come out in order with no extra work.
- Metadata the parser already has per sheet, and `spreadsheetCells()` already returns: name, index, hidden, dimension, print area, merged ranges, width and height, column and row sizes, warnings. Per workbook it knows the date system (1900 or 1904). Not read yet: the document properties (title, author, dates), which live in the package and would be read through `ooxmldown`.
- `layout()` would return the same sheet metadata next to its words and lines.

**Routing: decided, done by the caller.** A quick sniff of the incoming document gives its MIME type, and the caller hands it to the matching parser: `pdfdown`, `xlsxdown`, later `docxdown`. No parser needs to recognize another's format, so `detectFormat` is not transferred.

```ts
// the shape of the caller's router; each parser offers the same verbs for its own format
const parser = mime === "application/pdf" ? new PdfDown(buffer) : new XlsxDown(buffer);
const invoice = parser.invoice();
```

- Each parser offers the same verbs for its own format, invoices included.
- One limit of sniffing by first bytes: a PDF starts with `%PDF-`, but xlsx, docx and pptx are all zip files and start with the same bytes. Telling them apart means looking inside the package. `ooxmldown` already does that (`Package::kind`), so it can be surfaced from a binding if the sniff you use does not already distinguish them.

## 5. Additions for the minor release

### 5.1 Body length and rolling offset

For tracking where each page sits in the text during ingestion, for example mapping a chunk in a document store back to its page.

```rust
// pdfdown
pub struct StructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
  /// Length of `body` in UTF-16 code units: equal to `body.length` in JS.
  pub body_length: u32,
  /// Rolling total: the sum of `body_length` over all earlier pages.
  pub body_offset: u32,
}

fn utf16_len(text: &str) -> u32 {
  u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

let mut offset = 0u32;
for page in &mut pages {
  page.body_length = utf16_len(&page.body);
  page.body_offset = offset;
  offset = offset.saturating_add(page.body_length);
}
```

What JS sees:

```ts
interface StructuredPageText {
  page: number;
  header: string;
  body: string;
  footer: string;
  bodyLength: number; // === body.length
  bodyOffset: number; // 0 for the first page
}

// the pair the script computes by hand today
const offsets = [p.bodyOffset, p.bodyOffset + p.bodyLength] as const; // [start, end)
```

Why UTF-16 and not Rust's `len()`:

| Text | JS `length` | UTF-8 bytes |
| --- | ---: | ---: |
| `две головы` | 10 | 19 |
| `naïve` | 5 | 6 |
| `😀 ok` | 5 | 7 |

Notes:

- `bodyOffset` assumes the bodies are joined with nothing between them. A caller who joins with a separator adds `pageIndex * separator.length`.
- This is not a speed gain. The loop in the script takes about 0.1 ms for a 1,000-page document, and Rust has to walk the text once to count. The gain is one consistent answer for every caller.

### 5.2 Paper size

One new field, `paperSize`, on `PageBox`. `pageBoxes` already groups pages by size, so the document's size is the first entry and mixed-size documents get one value per group.

```rust
// library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSize {
  A0, A1, A2, A3, A4, A5, A6, A7, A8, A9, A10,
  B0, B1, B2, B3, B4, B5, B6, B7, B8, B9, B10,
  Letter, Legal, Tabloid, Executive, Statement,
  Unknown,
}

/// Portrait sizes in points: (size, short side, long side).
const SIZES: [(PaperSize, f64, f64); 27] = [
  (PaperSize::A0, 2384.0, 3370.0),
  (PaperSize::A4, 595.0, 842.0),
  (PaperSize::Letter, 612.0, 792.0),
  // ... the other 24, from the enum's doc comments
];

const TOLERANCE: f64 = 0.03;

/// The nearest standard size, ignoring orientation, or `Unknown` when even the nearest is more
/// than `TOLERANCE` away on either side.
pub fn paper_size(width: f64, height: f64) -> PaperSize {
  let (short, long) = if width <= height { (width, height) } else { (height, width) };
  SIZES
    .iter()
    .map(|&(size, w, h)| (size, ((short - w).abs() / w).max((long - h).abs() / h)))
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .filter(|&(_, error)| error <= TOLERANCE)
    .map_or(PaperSize::Unknown, |(size, _)| size)
}

pub struct PageBox {
  // ... existing fields
  pub paper_size: PaperSize,
}
```

```rust
// pdfdown-node: the spelling option gives the names in your enum while the variants stay idiomatic
#[napi(string_enum = "UPPERCASE")]
pub enum PaperSize { A0, /* ... */ Letter, Legal, Tabloid, Executive, Statement, Unknown }
```

What JS sees, which is the shape a Prisma enum has, so it can be stored as it is:

```ts
type PaperSize = "A0" | "A1" | /* ... */ "B10" | "LETTER" | "LEGAL" | "TABLOID" | "EXECUTIVE" | "STATEMENT" | "UNKNOWN";

const documentPaperSize = meta.pageBoxes[0]?.paperSize ?? "UNKNOWN";
```

Why a string enum and not the structured form (`{ type: 'A4' }`): the structured form is for variants that carry different data, like `PageFormField`. No paper size carries any, so every value would be an object wrapping one string.

Checked:

- All 27 sizes in the enum match ISO 216 (millimetres converted at 72 / 25.4, rounded) and the inch definitions of the five North American sizes.
- The installed napi-derive 3.7.1 accepts `string_enum = "UPPERCASE"`. pdfdown's other enums are PascalCase (`'Unknown'`), so this one would differ from them.
- Tolerance: past about 2.9% a page can be within tolerance of two sizes at once (A5 and Statement first, then A4 and Letter, A3 and Tabloid at 3.1%). Nearest still wins, but 3% is the natural ceiling.

Where this comes from and where it goes:

- The list is the `DocumentPaperSize` enum in `packages/db/prisma/schema/document.prisma`, value for value. The `UPPERCASE` spelling makes `paperSize` assignable to that column with no mapping.
- It will be recorded for PDF now and for docx once `docxdown` exists. A docx states its page size directly (in twentieths of a point), so the same table and the same matcher apply; only the source of the width and height differs.
- Built for 0.14.0. `PaperSize` and `paper_size()` are in `contentdown` (`src/paper.rs`), which took over the page model from `pdfdown-meta`; `pdfdown` re-exports `PaperSize` and fills `PageBox.paper_size`, and `pdfdown-node` mirrors it as a union keyed on `type` (`{ type: 'A4'; width: number; height: number }`, `{ type: 'UNKNOWN' }`; `#[napi(discriminant_case = "UPPERCASE")]`) and not as the string enum sketched above, so that `UTR<PaperSize, "type">` applies. `width` and `height` are the standard's portrait dimensions, from `PaperSize::dimensions()`; the column value is `paperSize.type`. Only the `UNKNOWN` member carries `closestTo: [smaller, larger]`, the standard sizes on either side of the page by area from `paper_neighbors()`, with `null` on a side that is past the end of the table (below A10 or above B0). The reasoning below, written when the home was `pdfdown-meta`, is why they are not in `pdfdown` itself.
- `PaperSize` and `paper_size()` therefore go in `pdfdown-meta` and not in `pdfdown`: they are plain data and a pure function, and that keeps them reachable without pulling in a PDF parser. When `docxdown` starts, either it depends on `pdfdown-meta` for them, or those 60 or so lines move somewhere both can reach. `PageBox` stays in `pdfdown` and gains the field.

### 5.3 Linearization, and optionally revisions

lopdf 0.47 reports whether a file is linearized. pdfdown's current check looks in the trailer, where lopdf never stores that key, so `isLinearized` is false for every document today.

```rust
// today (core/meta.rs)
let is_linearized = doc.trailer.get(b"Linearized").is_ok();

// after, with lopdf 0.47
let is_linearized = doc.linearization.is_some();
```

lopdf 0.47 also counts revisions. A count above 1 means the file was saved again after it was first written, which may be worth knowing about an invoice. Optional:

```rust
pub struct PdfMeta {
  // ... existing fields
  /// 1 for a file written once; more when it was updated and saved again.
  pub revision_count: u32,
}

let revision_count = u32::try_from(doc.revision_count()).unwrap_or(u32::MAX);
```

### 5.4 Images: JBIG2, and keeping the source format

**JBIG2.** Black-and-white scans are often stored as JBIG2. pdfdown has no decoder for it, so those images are silently dropped today. `hayro-jbig2` (crates.io, Apache-2.0 OR MIT, pure Rust) decodes them, and slots in next to the JPEG 2000 path:

```rust
// today (core/images.rs): the three decoders
let dynamic_img = if filter == "DCTDecode" {
  image::load_from_memory_with_format(content, ImageFormat::Jpeg).ok()?
} else if filter == "JPXDecode" {
  decode_jpx(content)?
} else if let Some(p) = ccitt {
  decode_ccitt(content, width, height, p)?
} else {
  decode_raw_pixels(content, width, height, bpc, color_space)?
};

// after: one more branch
} else if filter == "JBIG2Decode" {
  decode_jbig2(content, jbig2_globals.as_deref())?

/// `globals` is the PDF's shared `/JBIG2Globals` stream, when the image names one.
fn decode_jbig2(content: &[u8], globals: Option<&[u8]>) -> Option<DynamicImage> {
  let settings = hayro_jbig2::DecodeSettings::default();
  let globals = match globals {
    Some(data) => Some(hayro_jbig2::Globals::new(data, &settings).ok()?),
    None => None,
  };
  let page = hayro_jbig2::Page::new_embedded(content, globals.as_ref(), &settings).ok()?;
  DynamicImage::from_decoder(page).ok()
}
```

**As built.** JBIG2 is decoded, in `crates/pdfdown/src/images.rs`, with these differences from the sketch:

- **The premise was half right.** A JBIG2 stencil mask was dropped, but a JBIG2 image that states a gray color space was not: its compressed bytes were returned as if they were pixels. Both are decoded now.
- **A codec, not a filter name.** `decode_xobject_to_dynamic_image` takes a `Codec` (`Jpeg`, `Jpx`, `Ccitt(CcittParams)`, `Jbig2(Jbig2Params)`, `Raw`) in place of the filter name and the optional fax parameters. `codec_of` reads it from the image dictionary once.
- **Its own sink.** The decoder is driven through its `Decoder` trait into gray rows, as the fax decoder is, and not through its bridge to the `image` crate, because polarity is ours to decide: ink is black unless `/Decode [1 0]` swaps it. A stencil mask comes out the same way, as what it paints, and is reported as `DeviceGray`.
- **Globals** come from the stream `/DecodeParms /JBIG2Globals` names, with that stream's own filter undone.
- **Limits.** A stream that states more than a gigapixel is not decoded. One whose size differs from the dictionary's is resampled to the dictionary's.
- **Tests** are in `images/jbig2_tests.rs`, on streams written by hand: a generic region and, for globals, a halftone region whose patterns live in the shared stream. No fixture was added.
- **Not done:** a JBIG2 image with an `Indexed` color space is decoded as ink on paper, without its palette. JBIG2 images inside form XObjects are not collected, like any other image there.

**Source format.** Today every image is decoded and written out again as PNG, including the ones that were already a JPEG or JPEG 2000 file inside the PDF. Proposed: return those bytes untouched, and say what the bytes are.

```rust
// library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageImageFormat { Png, Jpeg, Jp2 } // not `ImageFormat`: the `image` crate has one

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageOutput {
  /// Original bytes for JPEG and JPEG 2000; PNG for everything else.
  #[default]
  Source,
  /// PNG for everything, as today.
  Png,
}

pub struct PageImage {
  // ... existing fields
  pub data: Vec<u8>,
  /// What `data` is. `filter` still says how the PDF stored the image.
  pub format: PageImageFormat,
}

let (data, format) = match (filter.as_str(), has_soft_mask, output) {
  ("DCTDecode", false, ImageOutput::Source) => (payload, PageImageFormat::Jpeg),
  ("JPXDecode", false, ImageOutput::Source) => (payload, PageImageFormat::Jp2),
  _ => (encode_to_png(/* ... */)?, PageImageFormat::Png),
};
```

```ts
interface PageImage {
  // ... existing fields
  data: Buffer;
  format: "Png" | "Jpeg" | "Jp2"; // no more sniffing the bytes
  filter: string;
}
```

What each kind of stored image becomes:

| Stored in the PDF as | A file format of its own? | Returned as |
| --- | --- | --- |
| JPEG (`DCTDecode`) | Yes | The original bytes, unless it has a soft mask |
| JPEG 2000 (`JPXDecode`) | Yes | The original bytes, unless it has a soft mask |
| CCITT fax, JBIG2 | No | PNG |
| Raw samples (`FlateDecode` and the rest) | No | PNG |

Notes:

- A PDF never contains a PNG or a WebP as such. A PNG is unpacked into raw samples on the way in, with its transparency split into a separate mask, and PDF has no WebP filter. So for those, PNG out is the faithful result.
- JPEG has no alpha channel, so an image with a soft mask falls back to PNG, which merges the two as today.
- Most browsers cannot display JPEG 2000, which is why `ImageOutput::Png` stays available.
- This one is a real speed and size gain: a passed-through image skips a decode and a PNG compression, and a JPEG is far smaller than the PNG of the same picture.
- Other output formats: the `image` crate's `webp` feature writes lossless WebP only, and `avif` needs a C library to read. Neither is planned; each would be a new feature in the root manifest.

### 5.5 What the `Document` model needs from pdfdown

The columns of `model Document` in `document.prisma`, against what pdfdown returns:

| Column | From pdfdown | Status |
| --- | --- | --- |
| `pageCount` | `metadata().pageCount` | Available today |
| `pdfVersion` | `metadata().version` | Available today |
| `pageWidth`, `pageHeight` | `metadata().pageBoxes[0].width` / `.height` | Available today |
| `pageSizeUniform` | `metadata().pageBoxes.length === 1` | Available today |
| `imageCount`, `annotCount` | `document().totalImages` / `.totalAnnotations` | Available today |
| `textPreview` | First 500 characters of `structuredText()` bodies | Available today |
| `isLinearized` | `metadata().isLinearized` | Wrong today (always false); fixed in section 5.3 |
| `paperSize` | `metadata().pageBoxes[0].paperSize` | Section 5.2 |
| `wordCount` | Sum of `layout()[n].words.length` | Derivable today; no field of its own |
| `isSearchable` | True when any page has native words | Derivable today; no field of its own |
| `title`, `author`, `subject`, `keywords` | Nothing | **Gap.** pdfdown reads only `Creator`, `Producer` and the two dates from the Info dictionary |
| `isEncrypted` | Nothing | **Gap.** lopdf knows (`is_encrypted`, `was_encrypted`); pdfdown does not report it |
| `language` | Nothing | Not planned; a PDF's `/Lang` entry is often absent or wrong |

Candidate additions to `PdfMeta`, cheap because they read the same dictionary pdfdown already opens:

```rust
pub struct PdfMeta {
  // ... existing fields
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  /// The Info dictionary's `/Keywords`, as written: one string, usually comma-separated.
  pub keywords: Option<String>,
  /// True when the file was encrypted on disk, whether or not it opened without a password.
  pub is_encrypted: bool,
}
```

None of these is in the plan until you say so.

## 6. Dependency versions

| Crate | pdfdown today | Workspace root | Latest on crates.io |
| --- | --- | --- | --- |
| flate2 | 1.1.10 | 1.1.11 | 1.1.11 |
| hayro-ccitt | 0.5.0 | 0.5.0 | 0.5.0 |
| hayro-jpeg2000 | 0.4.1 | 0.5.0 | 0.5.0 |
| hayro-jbig2 | not used | 0.4.0 | 0.4.0 |
| image | 0.25.10 | 0.25.10 | 0.25.10 |
| lopdf | 0.46.0 | 0.47.0 | 0.47.0 |
| rayon | 1.12.0 | 1.12.0 | 1.12.0 |
| roxmltree | 0.21.2 | 0.21.2 | 0.21.2 |
| napi | 3.15.0, `napi6` | 3.16.2, `napi10` | 3.16.2 |
| napi-derive | 3.7 | 3.7.1 | 3.7.1 |
| napi-build | 2.6.0 | 2.6.0 | 2.6.0 |
| pdfium-render | 0.9.4 | not listed | 0.9.5 |
| tesseract-rs | 0.4.0 | not listed | 0.4.0 |

A scratch copy of pdfdown's `src/` against the right-hand versions:

| Check | Result |
| --- | --- |
| `cargo check` with no features, `render`, and `ocr,render` | All compile, no source changes |
| `cargo test`, no features | 172 passed, 0 failed |
| `cargo test --features ocr,render` | 194 passed, 0 failed |
| Baseline: same tests at pdfdown's own lockfile | 194 passed, 0 failed |

The same source built at both sets of versions and run over a sample of the private corpus (the counts are in the chat, not recorded here):

| Output | Old versions against new |
| --- | --- |
| Documents that load | Same; the same few fail in both |
| Metadata, page counts, annotations, form fields | Identical |
| Layout (words and positions) | Identical |
| Text | A small number of documents gained text; none lost any |
| Images, pixels | Identical except JPEG 2000 images, where the largest difference is 1 level out of 255 in at most 0.01% of samples |
| Images, bytes | Most PNG files differ, with identical pixels: the newer compressor writes slightly smaller files |
| Time | No measurable difference |

What lopdf 0.47's release notes mean for pdfdown:

| Change in 0.47 | Effect on pdfdown | Action |
| --- | --- | --- |
| Linearization parameters and revision count (#638) | Makes `isLinearized` answerable; today it is always false | One-line fix, section 5.3 |
| Text-extraction and content-stream fixes (inline images #628 #629, comments in object streams #643, earlier-revision objects #642 #648) | More text recovered from some documents | None; comes with the version |
| Rebuild the cross-reference table when the page tree does not load (#636) | Could open more damaged files; it rescued none in the sample | None |
| Decompression bounded by default at 256 MiB (#656, #625, #635) | pdfdown's own limit on page content is lower (64 MiB), so nothing changes in practice | None |
| Inherited `/Resources` from `get_page_resources` (#654) | pdfdown walks the parent chain itself | None; its own helper is fine |
| Dates without a UT offset (#652) | pdfdown parses dates itself | None |
| `/Trapped` in metadata (#634) | Available, not exposed | None unless you want the field |
| Encryption-reading fix (#624) | May open more encrypted files | None |
| Writer, merge, save and bookmark items | pdfdown only reads | None |

## 7. Order of work

One unit at a time, each stopping for your review.

1. Done: the page model and line builder (first written in `pdfdown-meta`, now in `contentdown`).
2. Done: `contentdown`, with pdfdown's invoice module moved in. 52 tests: the 46 from pdfdown's invoice module and 6 for the line builder.
3. Done: `xlsxdown` and `xlsxdown-node`. `Workbook::layout`, `Workbook::invoice` and `sheet_layout` in the library; `layout()`, `invoice()`, `spreadsheetGrid()` and their `Async` twins on `XlsxDown`. Checked against pdfdown built from its source: layout identical and invoice identical apart from the source label on every public fixture and every corpus workbook; the grid holds exactly the cells of the flat list. 270 JS tests pass natively, and the 10 WASI tests pass on a fresh WebAssembly build.
4. Done: `pdfdown`. PDF reading, `Pdf`, `PdfError`, plain types, the `render` feature. 106 tests, the same 106 pdfdown has outside its invoice, spreadsheet and OCR modules; lint clean with and without `render`. PDF metadata stays in `pdfdown`; `pdfdown-meta` is an empty placeholder.
5. Done: the Rust side of `pdfdown-node`, with and without its `ocr` and `render` features. Compiles and lints clean in all four combinations. Checked against pdfdown built from its own source at the same dependency versions: every reader gave identical output on all 2,695 sampled corpus PDFs (images and the document aggregate on 337 of them), and the OCR build gave identical OCR text, OCR layout and invoices on 16 documents. Not done here: the JS side of the crate (`yarn build`, typings, tests), which is yours to configure.
6. Done: `pdfdown-ocr`. The cascade, its six test modules (22 tests) and the OCR flavour of `extract_all`; lint clean with and without `render`.
7. The additions in section 5.
8. Done: JS tests for `pdfdown-node`, written on the `WorkupService` classes (not carried over from the ava suites): 48 public tests for the base package and 19 for OCR; a private-corpus spec that reads all 10,793 PDFs through the base package, and a sampled one for OCR. Still to do: CI.

The additions come after unit 3 so the output comparison in unit 3 checks the port and nothing else.

Fixtures that Rust tests read and that must move with them: `__test__/fixtures/einvoice/cii-sample.xml`, `ubl-sample.xml`, and `ocr/__test__/fixtures/multipage-ocr.pdf`.

How unit 5 is being done, for whoever picks it up mid-way:

- `src/lib.rs` is pdfdown's `lib.rs` carried across with the workbook code removed. A small `readers.rs` gives the library's readers the names and return shapes the original file called, so the body of `lib.rs` changes very little.
- The binding keeps its own `PdfState` (document plus original bytes behind `Arc`s), as the original did. Switching it to `pdfdown::Pdf` is a later tidy-up.
- Mirror types: `types.rs`, `layout.rs`, `forms.rs` are pdfdown's attribute-carrying type files with conversions from the library types added; `invoice.rs` is generated by the same script that wrote `xlsxdown-node`'s.
- The generator scripts are in the session scratchpad under `gen/` and are not part of the repo.

## 8. Consequences for CI

- The `xlsxdown-node` workflow triggers on changes to the root `Cargo.toml` and `Cargo.lock`, and its lint job runs `cargo clippy --workspace --all-targets` and `cargo test --workspace`.
- Once `pdfdown-ocr` is a member, both commands compile it. `tesseract-rs` finds the system library with pkg-config in its build script and stops the build when it is missing, so clippy fails too, not only the tests.
- The lint job therefore needs either `--exclude pdfdown-ocr`, or Tesseract and Leptonica installed on the runner. I would exclude it there and lint it in the OCR workflow, which installs them anyway.
- `pdfdown-node` builds without Tesseract unless `--features ocr` is passed, so only `pdfdown-ocr` itself needs excluding.

## 9. Evidence

### 9.1 pdfdown today

One crate (`cdylib`), 15,614 source lines and 5,172 test lines, building two npm packages.

| Area | Source lines | Test lines | External crates |
| --- | ---: | ---: | --- |
| PDF reading: `attachments`, `document`, `forms`, `images`, `layout/*`, `meta`, `text` | 4,679 | 2,472 | lopdf, image, flate2, hayro-ccitt, hayro-jpeg2000, rayon |
| `render` | 167 | 0 | pdfium-render, image |
| `invoice/*` | 3,812 | 1,562 | roxmltree (lopdf in one function) |
| `ocr` | 1,188 | 440 | tesseract-rs, image, rayon, lopdf |
| `spreadsheet/*` (already ported) | 2,778 | 637 | zip, roxmltree |
| `lib.rs` (all exports) | 1,827 | 0 | napi, napi-derive, lopdf |
| `types/*` | 1,148 | 61 | napi, napi-derive |

| npm package | Cargo features | Targets | Build |
| --- | --- | --- | --- |
| `@d0paminedriven/pdfdown` | `render` | 14, including WebAssembly | `napi build --features render` |
| `@d0paminedriven/pdfdown-ocr` | `ocr,render` | 3 (macOS arm64, Linux x64 and arm64, glibc) | same crate, `--package-json-path ocr/package.json --output-dir ocr` |

- **The core is almost napi-free already.** Only three lines under `core/` mention napi, all `use napi::Result`, on functions that never return an error. The real coupling is `crate::types`.
- **The OCR package is a superset of the base package.** It adds six free functions and six methods on the same `PdfDown` class.
- **`ocr` without `render`** is supported today through 24 `cfg` gates in `ocr.rs` and two shapes of `OcrOptions`. Both are kept (decision 5).

### 9.2 napi across crates

Why decision 1 went the way it did. A scratch workspace with `shared` (a plain library with `#[napi]` items) and `addon` (the `cdylib`, depending on `shared`); napi 3.16.2, napi-derive 3.7.1, the CLI from `xlsxdown-node`.

| Question | Result |
| --- | --- |
| A `#[napi]` function, object type and class defined in `shared` | Exported at run time and present in the generated `index.d.ts` |
| A function in `addon` taking `&Class` from `shared` | Works |
| Methods added to a class defined in `addon` by a `macro_rules!` from `shared` | Works; the methods are merged into the class in `index.d.ts` |
| An async method in `addon` returning `AsyncTask<T>` where `T`'s `Task` impl is in `shared` | Works at run time, but is typed `Promise<unknown>` |
| The same async function defined in `shared` next to its task | Typed correctly (`Promise<number>`) |
| All of the above on `wasm32-wasip1-threads`, and with `lto = true`, `strip = "symbols"` | Same |
| A `cdylib` with no napi code of its own that depends on `shared` and turns on a feature there | The native addon has the right exports, but the CLI generated no `index.d.ts`, and its WebAssembly loader exported nothing |

Rust also does not allow adding methods to a type from another crate, so a separate `pdfdown-ocr-node` could not add the six OCR methods to a `PdfDown` class defined in `pdfdown-node`.

## Parked

Ideas noted so they are not lost, deliberately not part of this plan.

**`documentdown`: a crate to take over what `@d0paminedriven/metadata` does.** Planned for after this port, using tokio and possibly reqwest. From that package's README, the responsibilities it would assume:

| Today in `@d0paminedriven/metadata` (TypeScript) | Where the Rust already exists or would come from |
| --- | --- |
| Sniff a buffer or the head of a URL and classify it | New. For zip-based Office files, `ooxmldown` already tells a workbook from a document from a deck |
| PDF facts: version, pages, linearization, encryption, Info strings, dates | lopdf's metadata-only loader, about 24 times faster than a full load on a corpus sample (figures in the chat). It lacks page boxes, so paper size and page dimensions need a full load |
| DOCX / PPTX / XLSX core and app properties | `ooxmldown` opens the package; reading `docProps` is new |
| Image headers: dimensions, color model, EXIF, ICC | New |
| Hardened remote fetch: range requests, deadlines, quarantine | New; this is the tokio and reqwest part |

This also gives the earlier idea, a small metadata-only PDF package with a WebAssembly build, its home. `pdfdown-meta` stays what it is, the page model.

Undecided (2026-11-14): the alternative is a dedicated pass to fix `@d0paminedriven/metadata` where it is, since a lot of work has gone into it. The two are not exclusive: that package could keep its fetching and image-header code and hand a full PDF buffer to `pdfdown` for the part it gets wrong.

Two things to weigh if the crate does start, noted now so they are not rediscovered:

- reqwest brings TLS into the build, and TLS is the usual source of cross-compilation trouble on musl and the less common targets.
- The WebAssembly target these packages build for has no sockets, so the fetching half cannot run there. Keeping the parsing as "bytes in, facts out" and the fetching as a separate layer would let the parsing half run everywhere, with the host (Node or the browser) doing the fetch where Rust cannot.

## Not verified

- Anything on Windows or macOS; all checks ran on Linux x64, plus `wasm32-wasip1-threads` for the napi experiment.
- pdfdown's JS test suites on the newer dependency versions.
- The snippets in section 5 are plans. None of them has been compiled. Section 4 is now real code, and where the two differ the code is right.
- The render tier. PDFium is not installed on the machine the checks ran on, so `capabilities().render` was false and no page was rasterized, in either build.
- `pdfdown-node` as a package: it was built into a scratch directory and loaded directly. `yarn build`, the generated typings and JS tests in the crate itself have not been run.
- WebAssembly for `pdfdown-node`. `xlsxdown-node` was rebuilt and tested for WebAssembly; `pdfdown-node` was not.
