# @d0paminedriven/xlsxdown-wasm

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/xlsxdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/xlsxdown-node.yml)

Reads Excel workbooks (`.xlsx`, and the macro-enabled and template kinds, `.xlsm`, `.xltx`, and `.xltm`) as WebAssembly: typed cells with their display text and their formulas, sheet geometry, pages, invoice fields, and counts of what a workbook holds.

It is [`@d0paminedriven/xlsxdown`](https://www.npmjs.com/package/@d0paminedriven/xlsxdown) with no native addon behind it: the same Rust parser, built to run in Node, on a page, and in a worker runtime. Reach for it when the code has to run somewhere a native addon cannot, or when you want the same build on every machine. In Node alone, the main package loads a native addon where there is one and falls back to WebAssembly where there is not.

```sh
npm install @d0paminedriven/xlsxdown-wasm
yarn add @d0paminedriven/xlsxdown-wasm
pnpm add @d0paminedriven/xlsxdown-wasm
```

node &ge; 24. There is nothing else to install, and no flag to pass.

## Which entry to import

| Import                                     | What it is                                                                              | Where it runs                                  |
| ------------------------------------------ | --------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `@d0paminedriven/xlsxdown-wasm`            | In Node, the build with threads. To a bundler building for a browser, the build without | Node; a page (see the limits below)            |
| `@d0paminedriven/xlsxdown-wasm/worker`     | The parser in a Web Worker of its own, answering by a promise                           | A page. This is the one to use in a browser    |
| `@d0paminedriven/xlsxdown-wasm/threads`    | The build with threads                                                                  | Node, and a page that is cross-origin isolated |
| `@d0paminedriven/xlsxdown-wasm/threadless` | The build without threads                                                               | Anywhere WebAssembly runs                      |
| `@d0paminedriven/xlsxdown-wasm/workerd`    | The build without threads, started from a module you compiled                           | A worker runtime such as Cloudflare Workers    |

Every entry says which build it is in `__napiBindingTarget`: `"wasm32-wasi"` with threads, `"wasm32-wasip1"` without. It is never `"native"`.

## In Node

```ts
import { readFile } from "node:fs/promises";

import {
  XlsxDown,
  __napiBindingTarget,
  spreadsheetCellsAsync
} from "@d0paminedriven/xlsxdown-wasm";

const file = await readFile("costs.xlsx");

const sheets = await spreadsheetCellsAsync(file); // every visible sheet's cells
const workbook = new XlsxDown(file); // parsed once, and asked as often as needed
workbook.metadata().sheetCount;
workbook.invoice().fields.invoiceId?.value; // the invoice's number, where it has one
__napiBindingTarget; // "wasm32-wasi"
```

In this build the `*Async` verbs work off the main thread.

## On a page

```ts
import { XlsxWorker } from "@d0paminedriven/xlsxdown-wasm/worker";

const xlsx = new XlsxWorker();

// `file` is a File, from an <input type="file"> or a drop.
const bytes = new Uint8Array(await file.arrayBuffer());

const sheets = await xlsx.spreadsheetCellsAsync(bytes, { hiddenSheets: true });

xlsx.terminate();
```

- **The verb is the build's own promise verb**, by the same name and with the same answer, so code written against `spreadsheetCellsAsync` runs on the worker unchanged.
- **The page stays free.** The workbook is read in the worker, and the page's own thread is not held.
- **No isolation is needed.** The page does not have to be cross-origin isolated, and the server needs no special headers. It does not even have to call the `.wasm` file `application/wasm`.
- **Bytes go in as a `Uint8Array`.** A page has no `Buffer`.
- **`xlsx.ready`** resolves with the build's name once the worker can answer, and rejects with the reason if it could not start. Awaiting it is optional: a call made before then is answered once the worker is ready.
- **`xlsx.terminate()`** ends the worker and gives its memory back, which is the only way WebAssembly memory is ever given back. Calls still waiting, and any made after, are rejected. It is also how to stop a workbook that is taking too long.
- **Memory** starts at 64 MiB and grows as a workbook needs.

The worker is two files, `dist/worker.js` and `dist/worker.wasm`, and is found beside the client that starts it. If your setup serves them from somewhere else, put the two in one folder and say where the script is:

```ts
const xlsx = new XlsxWorker("/workers/xlsxdown/worker.js");
```

The worker answers `spreadsheetCellsAsync`, which is the one verb the package has as a function. The rest (the grid, the pages, the invoice, and the counts) are methods of `XlsxDown`, which holds a parsed workbook and does not cross to a worker yet.

### Without the worker

Importing `@d0paminedriven/xlsxdown-wasm` on a page, through a bundler that honors the `browser` condition, gives the build without threads on the page's own thread, `XlsxDown` included. It works, with two limits:

- the page is held while a workbook is read, the `*Async` verbs included; and
- memory starts at 250 MiB.

The worker has neither.

## In a worker runtime

```ts
import wasmModule from "@d0paminedriven/xlsxdown-wasm/wasm.wasm";
import { instantiate } from "@d0paminedriven/xlsxdown-wasm/workerd";

const xlsx = await instantiate(wasmModule);

xlsx.spreadsheetCells(bytes);
new xlsx.XlsxDown(bytes).metadata();
```

This entry takes a `WebAssembly.Module` and reads no file, which is what a runtime that hands modules over at build time wants. Memory starts at 64 MiB.

## The verbs

| Verb                                               | Gives                                                                                                                                                                                           |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `spreadsheetCells`                                 | The cells of the selected sheets: each typed (text, number, date, time, boolean, error), with its display text, its number format, and its box in the sheet. Its `Async` twin returns a promise |
| `new XlsxDown(bytes)`, `XlsxDown.openAsync(bytes)` | A workbook parsed once, to ask more than one thing of                                                                                                                                           |
| `.spreadsheetCells()`, `.spreadsheetGrid()`        | The cells, as a list or keyed by row and column                                                                                                                                                 |
| `.layout()`                                        | Each sheet set on a page of positioned words                                                                                                                                                    |
| `.invoice()`                                       | Invoice fields read from those pages, with the total and the line items taken from the formulas, each with where it was found                                                                   |
| `.metadata()`                                      | Counts of what the workbook holds, sheet by sheet and in total                                                                                                                                  |

Every method of `XlsxDown` has an `Async` twin.

A formula gives the value Excel last left beside it, and its text in `formula`: `SUM(B2:B9)*Rate`. A cell that follows a shared formula has it written for itself, with the references moved to it. A workbook in the strict form of the format, and a package that is not a workbook (a Word document, a presentation), is refused by name.

## What has been tried

The tests run both builds in Node, by the names above, and run the worker on a page in Chrome, holding its answers against what the same build gives in Node.

Not yet tried: Firefox and Safari, a bundler's own handling of the worker's file, the build with threads on a page, and the `workerd` entry inside a worker runtime (it is tested in Node).

## Size

The package is about 1.5 MB packed. Each build is about 1.1 MB, the build without threads is in the package twice (once for the worker), and the worker's script is 0.5 MB, 84 kB gzipped.

## License

MIT
