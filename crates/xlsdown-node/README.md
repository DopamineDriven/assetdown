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
| `invoice(opts?)`          | Invoice fields read from those pages, with the total and the line items taken from the formulas, each with where it was found                                                                                                                                              |
| `metadata()`              | What the workbook says about itself (title, author, who saved it last, when it was created and saved, the application and its version, company), and counts of what it holds, sheet by sheet and in total, with whether it holds macros and which date system it counts in |

`spreadsheetCells(buffer, opts?)` and `spreadsheetCellsAsync(buffer, opts?)` are the first of these as functions, for a workbook that is read once.

The options choose the sheets: `sheets` names them, `hiddenSheets` includes the hidden ones, and `hiddenCells` (on by default) keeps the cells of hidden rows and columns.

## Cells

A cell is a union on `type`: `Text`, `Number`, `Date`, `Time`, `Boolean`, `Error`, or `Blank`. Its `value` is typed to match. `text` is what the cell shows, rendered through its number format; `raw` is what the file stores; `format` is the number-format code.

A number under a date format is a `Date`, with an ISO date as its value and the serial in `raw`. The workbook's date system (1900 or 1904) is honored.

A format code that is not rendered leaves `formatSupported` false. The cell's `text` is then the stored spelling for a number, and for a date or a time the day and the clock written plainly (`2015-08-31 12:00:00`). A code marked only with its language (`[$-409]mmmm d, yyyy`) is rendered where the language changes nothing: any English, or a code that names no month, day, or half of the day.

**Formulas.** A formula cell gives the value Excel last left beside it, and the formula's text in `formula`, without the `=` it opens with: `SUM(B2:B9)*Rate`, `'Unit prices'!A1`. That is what lets a total be checked against the cells it adds up. A cell that shares a formula with others has it written for itself, with the references moved to it, and `sharedFormula` names the cell the formula is kept for. A formula that fills a range together is on each cell of the range. The spaces a formula was typed with are not kept.

## Invoices

`invoice()` reads the pages by their labels, as an invoice in a PDF is read, and then asks the formulas, which a page does not have.

**The total.** The formulas decide it. Of the formulas that no other formula reads, one shown as money comes before one that is not, and then the one that reaches the most cells is the total. It becomes `fields.invoiceTotal`. Where the labels on the page gave the same amount, the two agree; where they gave another, the formulas have it and a warning says both. The labels are never asked which formula to take: a label that reads as a total is often on a part of the bill. Two things leave the labels' reading as it was: a formula total of nought (a form not filled in yet), and a formula total shown as a plain number (hours added up, a count) beside a labeled amount shown as money.

**The items.** The total's formula is followed down. A formula that comes to a sum of cells is a subtotal, and what it adds is looked at in turn; anything else (a number typed in, a quantity times a price, a tax worked out from a subtotal) is a line, and each line is one of `fields.items`. A sum is still a sum when it is rounded, written out as text (`TEXT(SUM(B2:B9),"0.00")`), or kept blank until there is something to show (`IF(SUM(B2:B9)>0,SUM(B2:B9),"")`). An item has its `amount`, a `description` from the text of its row (or failing that the heading over its column), and a `quantity` and `unitPrice` where its amount is one cell times another: the price is the one shown as money, or failing that the quantity is the one that is itself a sum (hours added up, at a rate). What the total takes away is an item below nought. Every field says where its cell is on the page, and an amount is given as its cell shows it (1250.38 for a cell that holds 1250.3756 and shows two places).

So the items come to the total, and a warning says when they do not. A total that is worked out some other way than by adding cells up has no items, and a warning says so.

```ts
const { fields, warnings } = workbook.invoice();
fields.invoiceTotal?.value; // 66
fields.items.map((item) => [item.description?.value, item.amount?.value]);
// [["Bolt", 20], ["Seal", 15.5], ["Labor", 24.5], ["Tax", 6]]
warnings; // [] when the items come to the total and the labels agree
```

## Errors

A workbook that cannot be read throws, and rejects from the `Async` twins, with one of three kinds of message:

- `unsupported format: …` for a workbook saved with a password of its owner's choosing (`an encrypted workbook`), a workbook of Excel 5 or 95, a worksheet of Excel 2, 3, or 4, and a compound file that holds no workbook;
- `limit exceeded: …` for a workbook with more cells or merged ranges than one file may hold; and
- `malformed workbook: …` for anything else, such as bytes that are not a compound file.

## Locked workbooks

Excel encrypts a workbook with a password of its own, `VelvetSweatshop`, whenever it must encrypt one and has been given none: when a workbook's structure is protected, or it is saved to be opened read-only. Such a file opens in Excel without a question, and it is read here without one too, like any other workbook. Both kinds of RC4 lock are opened: the one of Excel 97 and 2000, and the "CryptoAPI" one of Excel 2002 and 2003.

A workbook locked with a password of its owner's choosing is refused, as `unsupported format: an encrypted workbook`. So is one hidden the older way, by XOR.

## Not read yet

Charts, pivot tables, and pictures (they are counted), and the formula of a table of what-ifs (its text is empty). A workbook locked with a password of its owner's choosing is refused; there is no option to give one.

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
