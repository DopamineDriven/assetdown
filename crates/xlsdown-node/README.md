# @d0paminedriven/xlsdown

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/xlsdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/xlsdown-node.yml)

Reads Excel workbooks of 1997 to 2003 (`.xls`) in Node: typed cells with their display text, sheet geometry, pages of positioned words, invoice fields, and counts of what a workbook holds.

It is a Rust parser shipped as a native addon, with a WebAssembly build for platforms that have none. There is nothing else to install and nothing to configure: no Excel, no LibreOffice.

```sh
npm install @d0paminedriven/xlsdown
yarn add @d0paminedriven/xlsdown
pnpm add @d0paminedriven/xlsdown
```

node &ge; 24

The shapes are those of [`@d0paminedriven/xlsxdown`](https://www.npmjs.com/package/@d0paminedriven/xlsxdown), so code written for one kind of workbook reads the other. For a build with no native addon at all (a page, a worker runtime), see [`@d0paminedriven/xlsdown-wasm`](https://www.npmjs.com/package/@d0paminedriven/xlsdown-wasm).

## Quick start

```ts
import { readFile } from "node:fs/promises";

import { XlsDown } from "@d0paminedriven/xlsdown";

const workbook = new XlsDown(await readFile("costs.xls"));

const [sheet] = workbook.spreadsheetCells();
sheet?.cells[0]; // { type: "Text", ref: "A1", value: "Total", text: "Total", ... }
sheet?.cells.find((cell) => cell.formula !== undefined)?.formula; // "SUM(B2:B9)"

const meta = workbook.metadata();
meta.sheetCount;
meta.author; // who wrote it, where the file says
meta.modified; // when it was last saved: "2026-11-15T10:30:00Z"
workbook.invoice().fields.invoiceId?.value; // the invoice's number, where it has one
```

## Readers

`XlsDown` parses a workbook once, in its constructor or in `XlsDown.openAsync`, and is then asked as often as needed. Every method has an `Async` twin that works off the JavaScript thread.

| Method                    | Gives                                                                                                                                                                                                                                                                      |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `spreadsheetCells(opts?)` | The cells of the selected sheets, each typed, with its display text, its number format, and its box in the sheet                                                                                                                                                           |
| `spreadsheetGrid(opts?)`  | The same cells keyed by row and column                                                                                                                                                                                                                                     |
| `layout(opts?)`           | Each sheet set on a page of positioned words                                                                                                                                                                                                                               |
| `invoice(opts?)`          | Invoice fields read from those pages, each with where it was found                                                                                                                                                                                                         |
| `metadata()`              | What the workbook says about itself (title, author, who saved it last, when it was created and saved, the application and its version, company), and counts of what it holds, sheet by sheet and in total, with whether it holds macros and which date system it counts in |

`spreadsheetCells(buffer, opts?)` and `spreadsheetCellsAsync(buffer, opts?)` are the first of these as functions, for a workbook that is read once.

The options choose the sheets: `sheets` names them, `hiddenSheets` includes the hidden ones, and `hiddenCells` (on by default) keeps the cells of hidden rows and columns.

## Cells

A cell is a union on `type`: `Text`, `Number`, `Date`, `Time`, `Boolean`, `Error`, or `Blank`. Its `value` is typed to match. `text` is what the cell shows, rendered through its number format; `raw` is what the file stores; `format` is the number-format code.

A number under a date format is a `Date`, with an ISO date as its value and the serial in `raw`. The workbook's date system (1900 or 1904) is honored.

**Formulas.** A formula cell gives the value Excel last left beside it, and the formula's text in `formula`, without the `=` it opens with: `SUM(B2:B9)*Rate`, `'Unit prices'!A1`. That is what lets a total be checked against the cells it adds up. A cell that shares a formula with others has it written for itself, with the references moved to it, and `sharedFormula` names the cell the formula is kept for. A formula that fills a range together is on each cell of the range. The spaces a formula was typed with are not kept.

## Errors

A workbook that cannot be read throws, and rejects from the `Async` twins, with one of three kinds of message:

- `unsupported format: …` for a workbook saved with a password, a workbook of Excel 5 or 95, a worksheet of Excel 2, 3, or 4, and a compound file that holds no workbook;
- `limit exceeded: …` for a workbook with more cells or merged ranges than one file may hold; and
- `malformed workbook: …` for anything else, such as bytes that are not a compound file.

## Not read yet

Charts, pivot tables, and pictures (they are counted), and the formula of a table of what-ifs (its text is empty). A workbook saved with a password is refused.

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
