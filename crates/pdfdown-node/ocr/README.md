# @d0paminedriven/pdfdown-ocr

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/pdfdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/pdfdown-node.yml)

Rust-powered PDF extraction for Node.js with Tesseract OCR fallback for image-only pages. A superset of [`@d0paminedriven/pdfdown`](https://www.npmjs.com/package/@d0paminedriven/pdfdown) -- includes all base extraction APIs (text, positioned text, images, annotations, form fields, attachments, invoice fields, structured text, metadata, page rendering) plus OCR.

**System requirement:** [Tesseract](https://github.com/tesseract-ocr/tesseract) 5.x must be installed on the host.

## Install

```sh
npm install @d0paminedriven/pdfdown-ocr
yarn add @d0paminedriven/pdfdown-ocr
pnpm add @d0paminedriven/pdfdown-ocr
```

node &ge; 24. The package is an ES module.

### Tesseract setup

```bash
# Ubuntu/Debian (22.04 ships pre v5 > -- use the PPA for 5.x)
sudo add-apt-repository ppa:alex-p/tesseract-ocr5
sudo apt update
sudo apt install tesseract-ocr tesseract-ocr-eng -y
# Optional: all language packs
# sudo apt install tesseract-ocr-all

# macOS
brew install tesseract

# Arch
sudo pacman -S tesseract tesseract-data-eng
```

Verify with `tesseract --version` -- you should see 5.x.

### Tessdata auto-detection

The package automatically detects the tessdata directory at runtime by parsing the output of `tesseract --list-langs`. The detected path is cached for the lifetime of the process using a `OnceLock<Option<String>>` -- no global environment mutation, fully thread-safe.

**Resolution order:**

1. `TESSDATA_PREFIX` environment variable (if set, used as-is -- no auto-detection runs)
2. Auto-detection via `tesseract --list-langs` (parses the path from `List of available languages in "/path/to/tessdata/"`)
3. Tesseract's compiled-in default (if neither of the above yields a path)

Most users will not need to set `TESSDATA_PREFIX` at all. The auto-detection handles standard installations on Ubuntu (`/usr/share/tesseract-ocr/5/tessdata/`), macOS Homebrew (`/opt/homebrew/share/tessdata/`), Arch, and any other layout where `tesseract` is on `PATH`.

Set `TESSDATA_PREFIX` explicitly only if:

- Tesseract is not on `PATH` but the tessdata directory exists elsewhere
- You want to override the detected path (e.g., pointing to a custom-trained data directory)

```bash
# Override example (not usually needed)
export TESSDATA_PREFIX="/opt/custom/tessdata"
```

## API

This package exports everything from `@d0paminedriven/pdfdown` (text, positioned text, images, annotations, AcroForm fields, attachments, embedded e-invoice extraction, invoice fields, structured text, metadata with page and paper sizes, page rendering, `capabilities()` -- sync and async), plus the OCR-specific APIs below. `PdfDocumentOcr` carries the same `pageBoxes`, `form`, `formFields`, `totalFormFields` and `formFieldPages` as `PdfDocument`; form extraction is read-only and needs neither Tesseract nor PDFium. See the [base package docs](https://www.npmjs.com/package/@d0paminedriven/pdfdown) for the full base API.

### OCR standalone functions

```typescript
// Per-page OCR text extraction
export declare function extractTextWithOcrPerPage(
  buffer: Buffer,
  opts?: OcrOptions
): Array<OcrPageText>;

export declare function extractTextWithOcrPerPageAsync(
  buffer: Buffer,
  opts?: OcrOptions
): Promise<Array<OcrPageText>>;

// Full document extraction with OCR text fallback
export declare function pdfDocumentOcr(
  buffer: Buffer,
  opts?: OcrOptions
): PdfDocumentOcr;

export declare function pdfDocumentOcrAsync(
  buffer: Buffer,
  opts?: OcrOptions
): Promise<PdfDocumentOcr>;

// Positioned words and lines with the same three-tier cascade
export declare function extractLayoutWithOcrPerPage(
  buffer: Buffer,
  opts?: OcrOptions
): Array<PageLayout>;

export declare function extractLayoutWithOcrPerPageAsync(
  buffer: Buffer,
  opts?: OcrOptions
): Promise<Array<PageLayout>>;
```

### `PdfDown` class (includes OCR methods)

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer);
  static openAsync(buffer: Buffer): Promise<PdfDown>;

  // ── Base methods ──
  textPerPage(): Array<PageText>;
  textPerPageAsync(): Promise<Array<PageText>>;
  structuredText(): Array<StructuredPageText>;
  structuredTextAsync(): Promise<Array<StructuredPageText>>;
  layout(): Array<PageLayout>;
  layoutAsync(): Promise<Array<PageLayout>>;
  imagesPerPage(): Array<PageImage>;
  imagesPerPageAsync(): Promise<Array<PageImage>>;
  annotationsPerPage(): Array<PageAnnotation>;
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>;
  formFieldsPerPage(): Array<PageFormField>;
  formFieldsPerPageAsync(): Promise<Array<PageFormField>>;
  attachments(): Array<Attachment>;
  attachmentsAsync(): Promise<Array<Attachment>>;
  invoice(opts?: InvoiceOptions): InvoiceResult;
  invoiceAsync(opts?: InvoiceOptions): Promise<InvoiceResult>;
  metadata(): PdfMeta;
  metadataAsync(): Promise<PdfMeta>;
  document(): PdfDocument;
  documentAsync(): Promise<PdfDocument>;
  renderPagesAsync(opts?: RenderOptions): Promise<Array<RenderedPage>>;

  // ── OCR methods ──
  textWithOcrPerPage(opts?: OcrOptions): Array<OcrPageText>;
  textWithOcrPerPageAsync(opts?: OcrOptions): Promise<Array<OcrPageText>>;
  documentOcr(opts?: OcrOptions): PdfDocumentOcr;
  documentOcrAsync(opts?: OcrOptions): Promise<PdfDocumentOcr>;
  layoutWithOcr(opts?: OcrOptions): Array<PageLayout>;
  layoutWithOcrAsync(opts?: OcrOptions): Promise<Array<PageLayout>>;
}
```

### Types

```typescript
// The deepest cascade tier that produced a page's text.
export type TextSource = "Native" | "Ocr" | "Rendered";

export interface OcrPageText {
  page: number;
  text: string;
  source: TextSource;
}

export interface OcrStructuredPageText {
  page: number;
  header: string;
  body: string;
  footer: string;
  source: TextSource;
}

export type RenderMode = "Auto" | "Never" | "Always";

export interface OcrOptions {
  lang?: string; // Tesseract language code, default "eng"
  minTextLength?: number; // non-whitespace char threshold before OCR fallback, default 1
  maxThreads?: number; // cap on Rayon threads for OCR parallelism, default 4, clamped to [1, available CPUs]
  render?: RenderMode; // the render tier, default "Auto"; see "The render tier and PDFium"
  renderDpi?: number; // resolution pages are rendered at, default 300, clamped to 72..600
  pdfiumPath?: string; // the PDFium library file to load, for a host where it is not on the search path
}

export interface PdfDocumentOcr {
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
  text: Array<OcrPageText>;
  structuredText: Array<OcrStructuredPageText>;
  images: Array<PageImage>;
  annotations: Array<PageAnnotation>;
  form?: FormInfo;
  formFields: Array<PageFormField>;
  totalFormFields: number;
  formFieldPages: Array<number>;
}
```

## Usage

> **Use the async API for OCR.** The sync variants block the Node.js event loop for the duration of OCR processing, which can be significant for multi-page scanned documents.

### Standalone

```typescript
import { readFile } from "node:fs/promises";

import { extractTextWithOcrPerPageAsync } from "@d0paminedriven/pdfdown-ocr";

const pdf = await readFile("scanned-document.pdf");
const pages = await extractTextWithOcrPerPageAsync(pdf, {
  lang: "eng",
  minTextLength: 10
});

for (const { page, text, source } of pages) {
  console.log(`Page ${page} [${source}]: ${text.slice(0, 100)}...`);
}
```

### Class-based (parse once, extract many)

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown-ocr";

const pdf = new PdfDown(await readFile("scanned-document.pdf"));

// OCR text extraction
const pages = await pdf.textWithOcrPerPageAsync({
  lang: "eng",
  minTextLength: 10
});

// All base methods work too
const images = await pdf.imagesPerPageAsync();
const meta = pdf.metadata();
```

### Extract everything with OCR in one call

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown-ocr";

const pdf = new PdfDown(await readFile("scanned-document.pdf"));
const result = await pdf.documentOcrAsync({ minTextLength: 10 });

// result.text         — OcrPageText[] (page, text, source per page)
// result.structuredText — OcrStructuredPageText[] (header/body/footer + source per page)
// result.images       — PageImage[] (decoded PNGs with dimensions and color space)
// result.annotations  — PageAnnotation[] (links, destinations, rects)
// result.pageCount, result.version, result.creator, ...
```

### Combined: OCR text + images for multimodal pipelines

```typescript
import { readFile } from "node:fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown-ocr";

const pdf = new PdfDown(await readFile("scanned-document.pdf"));

const [ocrText, images] = await Promise.all([
  pdf.textWithOcrPerPageAsync({ minTextLength: 10 }),
  pdf.imagesPerPageAsync()
]);

const imagesByPage = Map.groupBy(images, (img) => img.page);

for (const { page, text, source } of ocrText) {
  const pageImages = (imagesByPage.get(page) ?? []).map((img) => ({
    dataUrl: `data:image/png;base64,${img.data.toString("base64")}`,
    width: img.width,
    height: img.height
  }));
  // Send { page, text, source, images: pageImages } to your embedding pipeline
}
```

### `document()` vs `documentOcr()`

Both methods extract everything from a PDF in a single call. The difference is how text is extracted:

| Method                                 | Text extraction          | Return type      | Use when                                 |
| -------------------------------------- | ------------------------ | ---------------- | ---------------------------------------- |
| `document()` / `documentAsync()`       | Native PDF text only     | `PdfDocument`    | PDF has selectable text                  |
| `documentOcr()` / `documentOcrAsync()` | Native with OCR fallback | `PdfDocumentOcr` | PDF may contain scanned/image-only pages |

`PdfDocumentOcr` uses `OcrPageText` (with `source: "Native" | "Ocr" | "Rendered"`) and `OcrStructuredPageText` (with header/body/footer split plus source) instead of the base `PageText` and `StructuredPageText` types. Images, annotations, metadata, page boxes and the read-only AcroForm data (`form`, `formFields`, `totalFormFields`, `formFieldPages`; see the base package docs) are identical in both.

### Positioned text with OCR

`extractLayoutWithOcrPerPage` returns the base package's `PageLayout` shape (words, lines and image placements with boxes in PDF points) for every page, choosing the source per page exactly like the text cascade:

1. **Native** -- glyphs placed from the content stream, with `fontName` / `fontSize` / `bold` on every word.
2. **Ocr** -- the page's embedded raster images are OCR'd and each Tesseract word box is mapped through the image's `Do` placement onto the page. Words carry `confidence` (0..1) instead of font metadata.
3. **Rendered** -- with PDFium, the page is rasterised as displayed at `renderDpi` and OCR'd; word boxes are mapped back onto the page box in the page's own frame, the one native words use, and `contentRotation` includes `/Rotate`.

Copier and fax scans (`CCITTFaxDecode`, Group 3 and Group 4) and bi-level scans kept as `JBIG2Decode` images or stencil masks are decoded in pure Rust, so they are read at tier 2 without PDFium. When the upright pass scores a mean Tesseract confidence under 55 (a landscape sheet scanned in portrait comes back as punctuation at 35-45), the raster is OCR'd again turned 90°, 270° and 180° and the best orientation wins. The page then reports `contentRotation`, the turn carrying the most confident OCR text across its images (the tiles of a tiled scan vote together), and every word, line and image placement is expressed in that turned frame, with `width`/`height` swapped for a quarter turn. A warning names each turned image with more than a few words and who decided the turn (the orientation detector or the confidence retry); such an image that reads upright at a different turn than the page is named too, since its words read sideways in the page frame. The render tier reads the page as displayed, so on a `/Rotate` page its `contentRotation` includes `/Rotate`; for any page, displayed coordinates are the reported frame turned clockwise by `rotate - contentRotation`. The same retry applies to `extractTextWithOcrPerPage`.

```typescript
import { readFileSync } from "node:fs";

import { extractLayoutWithOcrPerPage } from "@d0paminedriven/pdfdown-ocr";

for (const page of extractLayoutWithOcrPerPage(readFileSync("scan.pdf"), {
  render: "Auto"
})) {
  console.log(`page ${page.page} (${page.source})`);
  for (const line of page.lines) {
    const confidence = page.words
      .slice(line.firstWord, line.firstWord + line.wordCount)
      .map((w) => w.confidence ?? 1);
    console.log(`  ${Math.min(...confidence).toFixed(2)}  ${line.text}`);
  }
}
```

### The render tier and PDFium

Tier 3 of the cascade needs [PDFium](https://pdfium.googlesource.com/pdfium/), which is not bundled. It is looked for once per process, the first time anything asks for it (an OCR call, `renderPagesAsync` or `capabilities()`), and loaded from the first of these that works:

1. `pdfiumPath` in that call's `OcrOptions`: the library file itself (`libpdfium.so`, `libpdfium.dylib`)
2. The library file named by the `PDFIUM_LIBRARY_PATH` environment variable
3. The system's library search path

A `pdfiumPath` passed after that first look is ignored, so pass it on your first call or use the environment variable.

`capabilities()` returns `{ ocr: true, render }`, where `render` says whether PDFium was found. Without it the cascade stops at tier 2 and nothing fails: a page only a renderer could read comes back with whatever tiers 1 and 2 found.

`render` chooses how the tier is used:

| `render`   | What happens                                                                                                                                                |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `"Auto"`   | The default. A page is rendered when the cascade calls for it (see "How it works")                                                                          |
| `"Never"`  | No page is rendered, whether or not PDFium is there                                                                                                         |
| `"Always"` | Every page is rendered and OCR'd first, and what was read replaces the page's own text. A render that reads nothing falls back to the cascade for that page |

`renderPagesAsync`, from the base package, returns the rendered pages themselves as PNG; see the base README's "Rendering pages".

## Invoices from scans

`invoice()` and `invoiceAsync()` read the header fields of a PDF from its pages (see the base README's "Invoices from pages" section). In this package they run the OCR cascade with default options on the **first and the last page only**, the two pages the header track reads, so a long scan does not cost a whole-document OCR; `source` then reports `Ocr`, `Rendered` or `Mixed` and every OCR'd field's confidence is scaled by its words' Tesseract confidence. For other OCR options, call `layoutWithOcr(opts)` yourself and hand the pages to `invoiceFromLayout(pages)`.

## How it works

1. **Text extraction (the cascade):** Each page is read natively first. A page goes to OCR when its native text is under `minTextLength` non-whitespace characters, or when its native words cover under 2% of the page while images cover more than half of it or the content stream paints 200 or more path operators (a scan with a stamp on it, or text printed as outlines); a few words on an otherwise empty cover sheet stay native. Tier 2 decodes the embedded rasters and feeds them to Tesseract; tier 3, with PDFium, renders the page when the page has no rasters but painted paths, or when tier 2 found nothing or only a weak result on a path-heavy page. On a hybrid page the native words stay and OCR adds only what they do not cover (duplicates dropped at IoU > 0.5); `source` reports the deepest tier that ran (`Native`, `Ocr` or `Rendered`), even when OCR added no words; OCR words carry `confidence`, native words do not, and a warning counts the native words kept. Orientation is decided per raster: an upright pass with a mean Tesseract confidence of 80 or more stands; below that the raster is shown to Tesseract's orientation detector (OSD), and only when OSD cannot decide and the upright pass scored under 55 is the raster read again at 90°, 270° and 180° with the best score winning.

2. **Structured text:** After text extraction, repeated header/footer lines are detected across pages using frequency analysis (requires 3+ pages). Each page's text is split into `header`, `body`, and `footer` sections. For OCR results, the `source` tag is preserved so you know whether each page's content came from native extraction or OCR.

3. **Parallelism:** OCR runs on a dedicated capped Rayon thread pool (default 4 threads, configurable via `maxThreads`) to prevent CPU oversubscription. Text extraction, image extraction, annotation extraction and form extraction run concurrently via `rayon::join` when using `documentOcr` / `documentOcrAsync`. The package entry point also sets `OMP_THREAD_LIMIT=1` before the native module loads, because libtesseract otherwise starts one OpenMP thread per core inside every recognise call and the workers thrash each other (measured on a 12-core machine over nine scanned pages: 13.7 s with the limit, 196 s without it). Parallelism therefore comes from `maxThreads`, not from OpenMP; set `OMP_THREAD_LIMIT` or `OMP_NUM_THREADS` yourself before importing the package to override this.

4. **Tessdata discovery:** On first OCR invocation, the tessdata path is resolved once and cached in a `OnceLock`. The `TESSDATA_PREFIX` environment variable is checked first; if unset, `tesseract --list-langs` is executed and its output is parsed to extract the path. No environment variables are mutated -- the path is passed directly to Tesseract's init function.

## Supported platforms

Prebuilt binaries are provided for:

| Platform      | Architecture(s) |
| ------------- | --------------- |
| macOS         | ARM64           |
| Linux (glibc) | x64, ARM64      |

Intel macOS is not built: the OCR package links the system Tesseract, and Homebrew no longer ships Intel bottles, so there is no supported way to obtain the library on that platform. The base `@d0paminedriven/pdfdown` package still ships an Intel macOS binary, and builds for the other platforms and WebAssembly as well.

## Relationship to `@d0paminedriven/pdfdown`

The same binding, built with its `ocr` feature, which adds the Tesseract cascade on top of the base readers. This package is a strict superset -- you can use it as a drop-in replacement for the base package if you need OCR capabilities. The two are released together at one version.

## License

MIT
