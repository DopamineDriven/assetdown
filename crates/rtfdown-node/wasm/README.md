# @d0paminedriven/rtfdown-wasm

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/rtfdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/rtfdown-node.yml)

Reads RTF documents as WebAssembly: plain text, pages of positioned words, invoice fields, the files a document carries, and its pictures.

It is [`@d0paminedriven/rtfdown`](https://www.npmjs.com/package/@d0paminedriven/rtfdown) with no native addon behind it: the same Rust parser, built to run in Node, on a page, and in a worker runtime. Reach for it when the code has to run somewhere a native addon cannot, or when you want the same build on every machine. In Node alone, the main package loads a native addon where there is one and falls back to WebAssembly where there is not.

```sh
npm install @d0paminedriven/rtfdown-wasm
yarn add @d0paminedriven/rtfdown-wasm
pnpm add @d0paminedriven/rtfdown-wasm
```

node &ge; 24. There is nothing else to install, and no flag to pass.

## Which entry to import

| Import | What it is | Where it runs |
| --- | --- | --- |
| `@d0paminedriven/rtfdown-wasm` | In Node, the build with threads. To a bundler building for a browser, the build without | Node; a page (see the limits below) |
| `@d0paminedriven/rtfdown-wasm/worker` | The parser in a Web Worker of its own, with every verb answered by a promise | A page. This is the one to use in a browser |
| `@d0paminedriven/rtfdown-wasm/threads` | The build with threads | Node, and a page that is cross-origin isolated |
| `@d0paminedriven/rtfdown-wasm/threadless` | The build without threads | Anywhere WebAssembly runs |
| `@d0paminedriven/rtfdown-wasm/workerd` | The build without threads, started from a module you compiled | A worker runtime such as Cloudflare Workers |

Every entry says which build it is in `__napiBindingTarget`: `"wasm32-wasi"` with threads, `"wasm32-wasip1"` without. It is never `"native"`.

## In Node

```ts
import { readFile } from "node:fs/promises";

import {
  __napiBindingTarget,
  rtfInvoiceAsync,
  rtfText
} from "@d0paminedriven/rtfdown-wasm";

const file = await readFile("invoice.rtf");

rtfText(file); // the document's text
(await rtfInvoiceAsync(file)).fields.invoiceId?.value; // the invoice's number, where it has one
__napiBindingTarget; // "wasm32-wasi"
```

In this build the `*Async` verbs work off the main thread.

## On a page

```ts
import { RtfWorker } from "@d0paminedriven/rtfdown-wasm/worker";

const rtf = new RtfWorker();

// `file` is a File, from an <input type="file"> or a drop.
const bytes = new Uint8Array(await file.arrayBuffer());

const text = await rtf.rtfTextAsync(bytes);
const { fields } = await rtf.rtfInvoiceAsync(bytes);
const attached = await rtf.rtfEmbeddedAsync(bytes); // each with `data: Uint8Array`

rtf.terminate();
```

- **The verbs are the build's own promise verbs**, by the same names and with the same answers, so code written against `rtfTextAsync` and the rest runs on the worker unchanged.
- **The page stays free.** The document is read in the worker, and the page's own thread is not held.
- **No isolation is needed.** The page does not have to be cross-origin isolated, and the server needs no special headers. It does not even have to call the `.wasm` file `application/wasm`.
- **Bytes go in and come out as `Uint8Array`.** A page has no `Buffer`.
- **`rtf.ready`** resolves with the build's name once the worker can answer, and rejects with the reason if it could not start. Awaiting it is optional: a call made before then is answered once the worker is ready.
- **`rtf.terminate()`** ends the worker and gives its memory back, which is the only way WebAssembly memory is ever given back. Calls still waiting, and any made after, are rejected. It is also how to stop a document that is taking too long.
- **Memory** starts at 64 MiB and grows as a document needs.

The worker is two files, `dist/worker.js` and `dist/worker.wasm`, and is found beside the client that starts it. If your setup serves them from somewhere else, put the two in one folder and say where the script is:

```ts
const rtf = new RtfWorker("/workers/rtfdown/worker.js");
```

### Without the worker

Importing `@d0paminedriven/rtfdown-wasm` on a page, through a bundler that honors the `browser` condition, gives the build without threads on the page's own thread. It works, with three limits:

- the page is held while a document is read, the `*Async` verbs included;
- a verb that gives bytes back (`rtfEmbedded`, `rtfPictures`, and `rtfDocument` for a document that carries either) throws unless the page has a global `Buffer`; and
- memory starts at 250 MiB.

The worker has none of the three.

## In a worker runtime

```ts
import { instantiate } from "@d0paminedriven/rtfdown-wasm/workerd";
import wasmModule from "@d0paminedriven/rtfdown-wasm/wasm.wasm";

const rtf = await instantiate(wasmModule);

rtf.rtfText(bytes);
```

This entry takes a `WebAssembly.Module` and reads no file, which is what a runtime that hands modules over at build time wants. Memory starts at 64 MiB. The limit on bytes above holds here too, where the runtime has no `Buffer`.

## The verbs

Each takes a document's bytes, and each has an `Async` twin that returns a promise.

| Verb | Gives |
| --- | --- |
| `rtfText` | The text: a line to a paragraph, and a line to a table's row with a tab between its cells |
| `rtfLayout` | The pages: words and lines with their boxes, in points. An RTF file records a flow of text and not where it lands, so the positions are worked out, except for a paragraph the file itself places |
| `rtfInvoice` | Invoice fields read from those pages, each with where it was found |
| `rtfEmbedded` | The files the document carries, each as it was before it was embedded |
| `rtfPictures` | The pictures, as their bytes, with the format and the size each is shown at |
| `rtfDocument` | All of the above in one object |

`RtfDown` is the same as a class, for a document that is asked more than one thing: it is parsed once, in the constructor or in `RtfDown.openAsync`. The worker does not have it yet.

## What has been tried

The tests run both builds in Node, by the names above, and run the worker on a page in Chrome, holding its answers against what the same build gives in Node.

Not yet tried: Firefox and Safari, a bundler's own handling of the worker's file, the build with threads on a page, and the `workerd` entry inside a worker runtime (it is tested in Node).

## Size

The package is about 1.3 MB packed. Each build is about 0.8 MB, the build without threads is in the package twice (once for the worker), and the worker's script is 0.5 MB, 84 kB gzipped.

## License

MIT
