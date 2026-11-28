# @d0paminedriven/pdfdown-wasm

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/pdfdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/pdfdown-node.yml)

Reads PDFs as WebAssembly: the text of each page, pages of positioned words, invoice fields, pictures, annotations, form fields, attachments, and what a document says about itself.

It is [`@d0paminedriven/pdfdown`](https://www.npmjs.com/package/@d0paminedriven/pdfdown) with no native addon behind it: the same Rust parser, built to run in Node, on a page, and in a worker runtime. Reach for it when the code has to run somewhere a native addon cannot, or when you want the same build on every machine. In Node alone, the main package loads a native addon where there is one and falls back to WebAssembly where there is not.

**It reads the text a PDF holds, and does not recognize or render.** A scanned page, which is a picture of text, comes back with its picture and no words. Recognition is in [`@d0paminedriven/pdfdown-ocr`](https://www.npmjs.com/package/@d0paminedriven/pdfdown-ocr), which needs Tesseract and so a native addon. `capabilities()` says so: `{ ocr: false, render: false }`.

```sh
npm install @d0paminedriven/pdfdown-wasm
yarn add @d0paminedriven/pdfdown-wasm
pnpm add @d0paminedriven/pdfdown-wasm
```

node &ge; 26. There is nothing else to install, and no flag to pass. On Node 24 the package loads and answers, and then the engine itself fails (`Fatal process out of memory: Zone`) once it has read a dozen or so PDFs: its compiler cannot optimize one very large function of the PDF reader. Node 26 can.

## Which entry to import

| Import                                    | What it is                                                                              | Where it runs                                  |
| ----------------------------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `@d0paminedriven/pdfdown-wasm`            | In Node, the build with threads. To a bundler building for a browser, the build without | Node; a page (see the limits below)            |
| `@d0paminedriven/pdfdown-wasm/worker`     | The parser in a Web Worker of its own, answering by a promise                           | A page. This is the one to use in a browser    |
| `@d0paminedriven/pdfdown-wasm/threads`    | The build with threads                                                                  | Node, and a page that is cross-origin isolated |
| `@d0paminedriven/pdfdown-wasm/threadless` | The build without threads                                                               | Anywhere WebAssembly runs                      |
| `@d0paminedriven/pdfdown-wasm/workerd`    | The build without threads, started from a module you compiled                           | A worker runtime such as Cloudflare Workers    |

Every entry says which build it is in `__napiBindingTarget`: `"wasm32-wasi"` with threads, `"wasm32-wasip1"` without. It is never `"native"`.

## In Node

```ts
import { readFile } from "node:fs/promises";

import {
  PdfDown,
  __napiBindingTarget,
  extractTextPerPageAsync
} from "@d0paminedriven/pdfdown-wasm";

const file = await readFile("invoice.pdf");

const pages = await extractTextPerPageAsync(file); // one entry for each page
const doc = new PdfDown(file); // parsed once, and asked as often as needed
doc.metadata().pageCount;
doc.invoice().fields.invoiceTotal?.value; // the total, where the document states one
__napiBindingTarget; // "wasm32-wasi"
```

In this build the `*Async` verbs work off the main thread.

## On a page

```ts
import { PdfWorker } from "@d0paminedriven/pdfdown-wasm/worker";

const pdf = new PdfWorker();

// `file` is a File, from an <input type="file"> or a drop.
const bytes = new Uint8Array(await file.arrayBuffer());

const pages = await pdf.layoutPerPageAsync(bytes);
const invoice = await pdf.invoiceFromLayoutAsync(pages); // no second parse
const images = await pdf.extractImagesPerPageAsync(bytes); // each a PNG, as a Uint8Array

pdf.terminate();
```

- **The verbs are the build's own promise verbs**, by the same names and with the same answers, so code written against `extractTextPerPageAsync` and the rest runs on the worker unchanged. All thirteen are there, `capabilitiesAsync` among them.
- **The page stays free.** A document is read in the worker, and the page's own thread is not held.
- **No isolation is needed.** The page does not have to be cross-origin isolated, and the server needs no special headers. It does not even have to call the `.wasm` file `application/wasm`.
- **Bytes go in and come out as a `Uint8Array`.** A page has no `Buffer`.
- **`pdf.ready`** resolves with the build's name once the worker can answer, and rejects with the reason if it could not start. Awaiting it is optional: a call made before then is answered once the worker is ready.
- **`pdf.terminate()`** ends the worker and gives its memory back, which is the only way WebAssembly memory is ever given back. Calls still waiting, and any made after, are rejected. It is also how to stop a document that is taking too long.
- **Memory** starts at 64 MiB and grows as a document needs.

The worker is two files, `dist/worker.js` and `dist/worker.wasm`, and is found beside the client that starts it. If your setup serves them from somewhere else, put the two in one folder and say where the script is:

```ts
const pdf = new PdfWorker("/workers/pdfdown/worker.js");
```

Each call to the worker takes the document's bytes and parses it afresh. `PdfDown`, which parses once and answers many questions, stays where the build is and does not cross to a worker yet; `pdfDocumentAsync` gives everything in one answer.

### Without the worker

Importing `@d0paminedriven/pdfdown-wasm` on a page, through a bundler that honors the `browser` condition, gives the build without threads on the page's own thread, `PdfDown` included. It works, with three limits:

- the page is held while a document is read, the `*Async` verbs included;
- memory starts at 250 MiB; and
- a verb that gives bytes back (`extractImagesPerPage`, `attachments`, and `pdfDocument` for a document with either) needs a `Buffer` on the page, which a page does not have.

The worker has none of the three.

## In a worker runtime

```ts
import wasmModule from "@d0paminedriven/pdfdown-wasm/wasm.wasm";
import { instantiate } from "@d0paminedriven/pdfdown-wasm/workerd";

const pdf = await instantiate(wasmModule);

pdf.extractTextPerPage(bytes);
new pdf.PdfDown(bytes).metadata();
```

This entry takes a `WebAssembly.Module` and reads no file, which is what a runtime that hands modules over at build time wants. Memory starts at 64 MiB.

## The verbs

| Verb                                                    | Gives                                                                                        |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `extractTextPerPage`, `extractStructuredTextPerPage`    | The text of each page; and the same with headers and footers told from the body              |
| `layoutPerPage`                                         | Each page as positioned words and the lines they make, with where its pictures are           |
| `extractInvoice`, `invoiceFromLayout`                   | Invoice fields, each with where it was found: from a PDF's bytes, or from pages already read |
| `extractImagesPerPage`, `pdfImageCount`                 | Every picture as a PNG with its page and size; and how many there are, without decoding them |
| `extractAnnotationsPerPage`, `extractFormFieldsPerPage` | Links and notes; and form fields with what each holds                                        |
| `attachments`                                           | The files a PDF carries, such as the XML of an electronic invoice                            |
| `pdfMetadata`                                           | Version, page count, page boxes, and what the document says about itself                     |
| `pdfDocument`                                           | Everything above in one object                                                               |
| `capabilities`                                          | What the build can do: here, neither recognition nor rendering                               |
| `new PdfDown(bytes)`, `PdfDown.openAsync(bytes)`        | A document parsed once, to ask more than one thing of                                        |

Every verb, and every method of `PdfDown`, has an `Async` twin that returns a promise.

A file that cannot be read as a PDF is refused with `Failed to load PDF`.

## What has been tried

The tests run both builds in Node, by the names above, and run the worker on a page in Chrome, holding its answers against what the same build gives in Node.

Not yet tried: Firefox and Safari, a bundler's own handling of the worker's file, the build with threads on a page, and the `workerd` entry inside a worker runtime (it is tested in Node).

## Size

The package is about 4.7 MB packed. Each build is about 4.2 MB, 1.5 MB gzipped. The build without threads is in the package twice (once for the worker), and the worker's script is 0.5 MB, 88 kB gzipped.

## License

MIT
