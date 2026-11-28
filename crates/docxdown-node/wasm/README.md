# @d0paminedriven/docxdown-wasm

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/docxdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/docxdown-node.yml)

Reads Word documents (`.docx`, and the macro-enabled and template kinds, `.docm` and `.dotx`) as WebAssembly: plain text, Markdown, a tree of paragraphs and tables, pages of positioned words, invoice fields, pictures, links and comments, form fields, notes, and what a document says about itself.

It is [`@d0paminedriven/docxdown`](https://www.npmjs.com/package/@d0paminedriven/docxdown) with no native addon behind it: the same Rust parser, built to run in Node, on a page, and in a worker runtime. Reach for it when the code has to run somewhere a native addon cannot, or when you want the same build on every machine. In Node alone, the main package loads a native addon where there is one and falls back to WebAssembly where there is not.

```sh
npm install @d0paminedriven/docxdown-wasm
yarn add @d0paminedriven/docxdown-wasm
pnpm add @d0paminedriven/docxdown-wasm
```

node &ge; 24. There is nothing else to install, and no flag to pass.

## Which entry to import

| Import                                     | What it is                                                                              | Where it runs                                  |
| ------------------------------------------ | --------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `@d0paminedriven/docxdown-wasm`            | In Node, the build with threads. To a bundler building for a browser, the build without | Node; a page (see the limits below)            |
| `@d0paminedriven/docxdown-wasm/worker`     | The parser in a Web Worker of its own, answering by a promise                           | A page. This is the one to use in a browser    |
| `@d0paminedriven/docxdown-wasm/threads`    | The build with threads                                                                  | Node, and a page that is cross-origin isolated |
| `@d0paminedriven/docxdown-wasm/threadless` | The build without threads                                                               | Anywhere WebAssembly runs                      |
| `@d0paminedriven/docxdown-wasm/workerd`    | The build without threads, started from a module you compiled                           | A worker runtime such as Cloudflare Workers    |

Every entry says which build it is in `__napiBindingTarget`: `"wasm32-wasi"` with threads, `"wasm32-wasip1"` without. It is never `"native"`.

## In Node

```ts
import { readFile } from "node:fs/promises";

import {
  DocxDown,
  __napiBindingTarget,
  docxMarkdownAsync
} from "@d0paminedriven/docxdown-wasm";

const file = await readFile("invoice.docx");

const markdown = await docxMarkdownAsync(file); // headings, lists, tables, and notes
const doc = new DocxDown(file); // parsed once, and asked as often as needed
doc.metadata().pageCount;
doc.invoice().fields.invoiceTotal?.value; // the total, where the document states one
__napiBindingTarget; // "wasm32-wasi"
```

In this build the `*Async` verbs work off the main thread.

## On a page

```ts
import { DocxWorker } from "@d0paminedriven/docxdown-wasm/worker";

const docx = new DocxWorker();

// `file` is a File, from an <input type="file"> or a drop.
const bytes = new Uint8Array(await file.arrayBuffer());

const text = await docx.docxTextAsync(bytes);
const pages = await docx.docxLayoutAsync(bytes);
const images = await docx.docxImagesAsync(bytes); // each with its bytes, a Uint8Array

docx.terminate();
```

- **The verbs are the build's own promise verbs**, by the same names and with the same answers, so code written against `docxTextAsync` and the rest runs on the worker unchanged. All thirteen are there: text, text by page, Markdown, blocks, tables, layout, invoice, images, annotations, form fields, notes, metadata, and the whole document at once.
- **The page stays free.** A document is read in the worker, and the page's own thread is not held.
- **No isolation is needed.** The page does not have to be cross-origin isolated, and the server needs no special headers. It does not even have to call the `.wasm` file `application/wasm`.
- **Bytes go in and come out as a `Uint8Array`.** A page has no `Buffer`.
- **`docx.ready`** resolves with the build's name once the worker can answer, and rejects with the reason if it could not start. Awaiting it is optional: a call made before then is answered once the worker is ready.
- **`docx.terminate()`** ends the worker and gives its memory back, which is the only way WebAssembly memory is ever given back. Calls still waiting, and any made after, are rejected. It is also how to stop a document that is taking too long.
- **Memory** starts at 64 MiB and grows as a document needs.

The worker is two files, `dist/worker.js` and `dist/worker.wasm`, and is found beside the client that starts it. If your setup serves them from somewhere else, put the two in one folder and say where the script is:

```ts
const docx = new DocxWorker("/workers/docxdown/worker.js");
```

Each call to the worker takes the document's bytes and parses it afresh. `DocxDown`, which parses once and answers many questions, stays where the build is and does not cross to a worker yet; `docxDocumentAsync` gives everything in one answer.

### Without the worker

Importing `@d0paminedriven/docxdown-wasm` on a page, through a bundler that honors the `browser` condition, gives the build without threads on the page's own thread, `DocxDown` included. It works, with three limits:

- the page is held while a document is read, the `*Async` verbs included;
- memory starts at 250 MiB; and
- a verb that gives bytes back (`docxImages`, and `docxDocument` for a document with pictures) needs a `Buffer` on the page, which a page does not have.

The worker has none of the three.

## In a worker runtime

```ts
import wasmModule from "@d0paminedriven/docxdown-wasm/wasm.wasm";
import { instantiate } from "@d0paminedriven/docxdown-wasm/workerd";

const docx = await instantiate(wasmModule);

docx.docxText(bytes);
new docx.DocxDown(bytes).metadata();
```

This entry takes a `WebAssembly.Module` and reads no file, which is what a runtime that hands modules over at build time wants. Memory starts at 64 MiB.

## The verbs

| Verb                                               | Gives                                                                                            |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `docxText`, `docxTextPerPage`                      | The body as plain text in reading order, whole or page by page                                   |
| `docxMarkdown`                                     | The body as Markdown: headings, lists, emphasis, links, pipe tables, and the notes after it      |
| `docxBlocks`, `docxTables`                         | The body as a tree of paragraphs and tables; and the tables alone, keyed by row and column       |
| `docxLayout`                                       | Pages of positioned words. A docx records no positions, so these are estimates                   |
| `docxInvoice`                                      | Invoice fields read from those pages, each with where it was found                               |
| `docxImages`                                       | Every place a picture is used, with its bytes, its format, and its sizes                         |
| `docxAnnotations`, `docxFormFields`, `docxNotes`   | Links and comments; content controls, form fields, and mail-merge fields; footnotes and endnotes |
| `docxMetadata`                                     | What the document says about itself, its sections and paper, and counts of what it holds         |
| `docxDocument`                                     | Everything above in one object                                                                   |
| `new DocxDown(bytes)`, `DocxDown.openAsync(bytes)` | A document parsed once, to ask more than one thing of                                            |

Every verb, and every method of `DocxDown`, has an `Async` twin that returns a promise.

A file that is not a Word document (a workbook, a legacy `.doc`, a document protected with a password) is refused by name, and one that cannot be read at all as `malformed document`.

## What has been tried

The tests run both builds in Node, by the names above, and run the worker on a page in Chrome, holding its answers against what the same build gives in Node.

Not yet tried: Firefox and Safari, a bundler's own handling of the worker's file, the build with threads on a page, and the `workerd` entry inside a worker runtime (it is tested in Node).

## Size

The package is about 1.6 MB packed. Each build is about 1.3 MB, 0.46 MB gzipped. The build without threads is in the package twice (once for the worker), and the worker's script is 0.5 MB, 88 kB gzipped.

## License

MIT
