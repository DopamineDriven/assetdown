# @d0paminedriven/mimedown-wasm

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/mimedown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/mimedown-node.yml)

Says what a file is from its bytes, as WebAssembly: its media type, its usual extension, and the parser that reads it; then what the file says of itself (a picture's size, frames, and color, a document's pages, a sound's or a film's length), the shape of a sound, a still from a film, and where a film's keyframes are.

It is [`@d0paminedriven/mimedown`](https://www.npmjs.com/package/@d0paminedriven/mimedown) with no native addon behind it: the same Rust code, built to run in Node, on a page, and in a worker runtime. Reach for it when the code has to run somewhere a native addon cannot, or when you want the same build on every machine. In Node alone, the main package loads a native addon where there is one and falls back to WebAssembly where there is not.

```sh
npm install @d0paminedriven/mimedown-wasm
yarn add @d0paminedriven/mimedown-wasm
pnpm add @d0paminedriven/mimedown-wasm
```

node &ge; 26. There is nothing else to install, and no flag to pass. On Node 24 the package loads and answers, and then the engine itself fails (`Fatal process out of memory: Zone`) once it has read a dozen or so PDFs: its compiler cannot optimize one very large function of the PDF reader. Node 26 can.

## Which entry to import

| Import                                     | What it is                                                                              | Where it runs                                  |
| ------------------------------------------ | --------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `@d0paminedriven/mimedown-wasm`            | In Node, the build with threads. To a bundler building for a browser, the build without | Node; a page (see the limits below)            |
| `@d0paminedriven/mimedown-wasm/worker`     | The same verbs in a Web Worker of its own, each answering by a promise                  | A page. This is the one to use in a browser    |
| `@d0paminedriven/mimedown-wasm/threads`    | The build with threads                                                                  | Node, and a page that is cross-origin isolated |
| `@d0paminedriven/mimedown-wasm/threadless` | The build without threads                                                               | Anywhere WebAssembly runs                      |
| `@d0paminedriven/mimedown-wasm/workerd`    | The build without threads, started from a module you compiled                           | A worker runtime such as Cloudflare Workers    |

Every entry says which build it is in `__napiBindingTarget`: `"wasm32-wasi"` with threads, `"wasm32-wasip1"` without. It is never `"native"`.

## In Node

```ts
import { readFile } from "node:fs/promises";

import {
  MimeDown,
  __napiBindingTarget,
  sniffAsync,
  specsAsync
} from "@d0paminedriven/mimedown-wasm";

const file = await readFile("clip.webp");

const { mime, ext } = await sniffAsync(file); // "image/webp", "webp"
const found = await specsAsync(file);
if (found.facts.type === "image") {
  found.facts.image.frames; // more than one when the picture moves
}
const held = new MimeDown(file); // copied once, and asked as often as needed
await held.specsAsync();
__napiBindingTarget; // "wasm32-wasi"
```

In this build the `*Async` verbs work off the main thread.

## On a page

```ts
import { MimeWorker } from "@d0paminedriven/mimedown-wasm/worker";

const mime = new MimeWorker();

// `file` is a File, from an <input type="file"> or a drop. Only its head is read.
const piece = async (offset: number, length: number) => ({
  offset,
  bytes: new Uint8Array(await file.slice(offset, offset + length).arrayBuffer())
});
const pieces = [await piece(0, 4096)];
let found = await mime.specsFromRangesAsync(pieces, file.size);
// An answer that stops short says which range would complete it.
while (found.needs !== null) {
  pieces.push(await piece(found.needs.offset, found.needs.length));
  found = await mime.specsFromRangesAsync(pieces, file.size);
}

found.mime; // what the file is, whatever its name says
if (found.facts.type === "image") found.facts.image.animated;

mime.terminate();
```

- **The verbs are the build's own promise verbs**, by the same names and with the same answers: `sniffAsync`, `sniffFromRangesAsync`, `specsAsync`, `specsFromRangesAsync`, `waveformAsync`, `posterAsync`, and `keyframesAsync`, and the tables, `mediaTypesOfAsync`, `mediaTypeOfPathAsync`, `extensionsOfAsync`, and `builtWithAsync`. Code written against them runs on the worker unchanged.
- **A large file need not be read whole.** The two `FromRanges` verbs take the head of a file and then whatever range an answer's `needs` names, with every piece fetched so far. With the file's size given, as above, a range always counts from the start of the file.
- **The page stays free.** The work is done in the worker, and the page's own thread is not held.
- **No isolation is needed.** The page does not have to be cross-origin isolated, and the server needs no special headers. It does not even have to call the `.wasm` file `application/wasm`.
- **Bytes go in and come out as a `Uint8Array`**, a poster's picture included. What goes to the worker is copied, and only a view's own bytes are: a view of part of a larger buffer does not take the rest of that buffer with it.
- **`mime.ready`** resolves with the build's name once the worker can answer, and rejects with the reason if it could not start. Awaiting it is optional: a call made before then is answered once the worker is ready.
- **`mime.terminate()`** ends the worker and gives its memory back, which is the only way WebAssembly memory is ever given back. Calls still waiting, and any made after, are rejected. It is also how to stop a file that is taking too long.
- **Memory** starts at 64 MiB and grows as a file needs.

The worker is two files, `dist/worker.js` and `dist/worker.wasm`, and is found beside the client that starts it. If your setup serves them from somewhere else, put the two in one folder and say where the script is:

```ts
const mime = new MimeWorker("/workers/mimedown/worker.js");
```

Every verb the package has as a function is on the worker, the tables included, so a page that has the worker needs no second copy of the build on its own thread. One thing does not cross to it yet: `MimeDown`, which keeps a file for more than one question, stays where the build is.

### Without the worker

Importing `@d0paminedriven/mimedown-wasm` on a page, through a bundler that honors the `browser` condition, gives the build without threads on the page's own thread, with `MimeDown` and the verbs that answer at once included. It works, with two limits:

- the page is held while a file is read, the `*Async` verbs included; and
- memory starts at 250 MiB.

The worker has neither.

## In a worker runtime

```ts
import wasmModule from "@d0paminedriven/mimedown-wasm/wasm.wasm";
import { instantiate } from "@d0paminedriven/mimedown-wasm/workerd";

const mime = await instantiate(wasmModule);

mime.sniff(bytes).mime;
mime.specsFromRanges([{ offset: 0, bytes: head }], size);
```

This entry takes a `WebAssembly.Module` and reads no file, which is what a runtime that hands modules over at build time wants. Memory starts at 64 MiB.

## The verbs

| Verb                                                           | Gives                                                                                                                                                                                  |
| -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sniff`, `sniffFromRanges`                                     | What a file is: its media type, the usual extension, the parser that reads it, what gave it away, and in `needs` the range that would settle it when the bytes stop short              |
| `specs`, `specsFromRanges`                                     | The same, and what the file says of itself: a picture's size, frames, alpha, color, and what a camera wrote down; a document's pages and title; a sound's or a film's length and codec |
| `waveform`                                                     | The shape of the sound in an audio file or a film: the least, the greatest, the peak, and the RMS of each channel, as typed arrays                                                     |
| `poster`                                                       | A still from a film: one keyframe, decoded and written as a JPEG, a PNG, or a WebP                                                                                                     |
| `keyframes`                                                    | Where a film's keyframes are. Nothing is decoded                                                                                                                                       |
| `new MimeDown(bytes)`                                          | A file copied once and kept, to ask more than one thing of                                                                                                                             |
| `mediaTypesOf`, `mediaTypeOfPath`, `extensionsOf`, `builtWith` | The tables between a type and its extensions, and which parsers the build has: here, all four                                                                                          |

Every verb has an `Async` twin that returns a promise, the tables included, and so has each method of `MimeDown`.

A picture's `frames` is how many frames it is made of: more than one for an animated PNG, GIF, WebP, or AVIF, and one for a still. An icon's sizes and a TIFF's pages are not frames, and are counted in the format's own `detail`. An animated JPEG XL says that it moves and not how many frames it has, so it gives `animated: true` beside `frames: 1`.

## What has been tried

The tests run both builds in Node, by the names above, and run the worker on a page in Chrome, holding its answers against what the same build gives in Node.

Not yet tried: Firefox and Safari, a bundler's own handling of the worker's file, the build with threads on a page, and the `workerd` entry inside a worker runtime (it is tested in Node).

## Size

The package is about 7.5 MB packed. Each build is about 6.6 MB, 2.4 MB gzipped, because it holds every parser the main package has (PDF, Office, audio, and video). The build without threads is in the package twice (once for the worker), and the worker's script is 0.5 MB, 88 kB gzipped.

## License

MIT
