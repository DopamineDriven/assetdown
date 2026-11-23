# @d0paminedriven/docdown

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/docdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/docdown-node.yml)

Reads Word documents of 1997 to 2003 (`.doc`) in Node: plain text, pages of positioned words, invoice fields, the files a document carries, and its pictures.

It is a Rust parser shipped as a native addon, with a WebAssembly build for platforms that have none. There is nothing else to install and nothing to configure: no Word, no LibreOffice.

```sh
npm install @d0paminedriven/docdown
yarn add @d0paminedriven/docdown
pnpm add @d0paminedriven/docdown
```

node &ge; 24

For a build with no native addon at all (a page, a worker runtime), see [`@d0paminedriven/docdown-wasm`](https://www.npmjs.com/package/@d0paminedriven/docdown-wasm). For `.docx`, see [`@d0paminedriven/docxdown`](https://www.npmjs.com/package/@d0paminedriven/docxdown).

## Quick start

```ts
import { readFile } from "node:fs/promises";

import { DocDown, docText } from "@d0paminedriven/docdown";

const file = await readFile("invoice.doc");

docText(file); // the document's text

const doc = new DocDown(file); // parsed once, and asked as often as needed
doc.layout()[0]?.lines[0]?.text; // the first line of the first page
doc.invoice().fields.invoiceId?.value; // the invoice's number, where it has one
doc.embedded(); // the files it carries, each with `data: Buffer`
```

## The verbs

Each takes a document's bytes, and each has an `Async` twin that returns a promise and works off the JavaScript thread.

| Verb | Gives |
| --- | --- |
| `docText` | The text: a line to a paragraph, and a line to a table's row with a tab between its cells |
| `docLayout` | The pages: words and lines with their boxes, in points |
| `docInvoice` | Invoice fields read from those pages, each with where it was found |
| `docEmbedded` | The files the document carries, each as it was before it was embedded |
| `docPictures` | The pictures, as their bytes, with the format and the size each is shown at |
| `docDocument` | All of the above in one object, with whether the file is a template and whether it holds macros |

`DocDown` is the same as a class, for a document that is asked more than one thing: it is parsed once, in the constructor or in `DocDown.openAsync`.

## What is read

- **Text**, in the order the file keeps it. A field gives what it shows and not what it is told. Hidden text is left out.
- **Tables**, a row at a time, with the edges the file gives each cell.
- **Text boxes.** The text of a box is put where its shape is anchored, in the text and on the page.
- **How text is set**: bold, italic, size, and font, and a paragraph's alignment, indents, and spacing, with the document's styles under what a paragraph and a run state themselves.
- **Paper and margins**, from the first section.
- **Headers and footers.** They are on the pages `docLayout` gives, and not in `docText`.
- **Embedded files**: a file dropped into the document, with its name; a PDF or a document of 2007 or later; and a workbook or a document of before 2007, given as the compound file it is. `class` is what the object calls itself (`Package`, `Excel.Sheet.8`, `AcroExch.Document.7`).
- **Pictures**, set in the text or floating over it: `png`, `jpeg`, `tiff`, `emf`, `wmf`, `dib`, or `macpict`. A metafile is given as its bytes and is not drawn.

## Pages are worked out

A file of this kind records a flow of text and not where anything lands, so `docLayout` sets it: a paragraph wrapped between the margins the file states, the cells of a row between their edges, and a page turned at a page break and at the foot of a page. Every page says so in its `warnings`. Where Word really broke a line, and a font's real widths, are not known.

## Errors

A document that cannot be read throws, and rejects from the `Async` twins, with one of three kinds of message:

- `unsupported format: …` for a document of Word 6 or 95, a document saved with a password, and a compound file that holds no Word document;
- `limit exceeded: …` for a document that states more characters than any holds; and
- `malformed document: …` for anything else, such as bytes that are not a compound file.

## Not read yet

Footnotes, endnotes, and comments; sections after the first (their paper, and a break that does not start a page); tab stops; tables inside tables (their text is kept, in the cell that holds them); and what a document says about itself (title, author, dates). A document saved with a password is refused.

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
