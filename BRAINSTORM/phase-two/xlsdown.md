# xlsdown: notes

Status: **built** (2026-11-23), not yet released. `cfbdown`, `xlsdown`, and `xlsdown-node` are written and tested; see [What is built](#5-what-is-built).

Some workbooks are in the format Excel wrote from 1997 to 2003: a compound file with the workbook inside it as one stream of records. `xlsdown` reads them into the model `xlsxdown` already gives, so that everything after the reading is shared. Its companion is [docdown.md](docdown.md), and both stand on the same compound-file reader.

## 1. Decisions

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | A crate of its own | **Decided** (2026-11-22). `xlsdown`, with `xlsdown-node` | Your call |
| 2 | The compound file | A small shared crate, `cfbdown`: the streams and storages of a compound file, by name. The reader inside `rtfdown` moves into it, and `rtfdown`, `xlsdown`, and `docdown` use it | Three crates would each carry the same reader. [rtfdown.md](rtfdown.md) said it would move when a second crate needed it |
| 3 | Which Excel | The record layout of Excel 97 to 2003 (BIFF8). An older workbook is named for what it is and refused | The older layouts differ in most records |
| 4 | The model | `xlsxdown`'s own: `Workbook`, `Sheet`, `Cell`, `CellValue`, `Xf`. `xlsdown` depends on `xlsxdown` and fills it | Number formats, sheet geometry, the pages, and the invoice are written once, against that model |
| 5 | Formulas | The value Excel last computed, which the file keeps beside each formula. The formula's text is not given at first; a cell says that it has one | The text is stored as tokens that need a decompiler of their own. The value is what a reader of the sheet wants |
| 6 | Dates | The workbook's date system (1900 or 1904) and the cell's number format, as in `xlsxdown` | The same rule, in the same code |
| 7 | A password | Refused, and named: `unsupported format: an encrypted workbook` | Decryption is a feature of its own |
| 8 | Binding | `xlsdown-node`, with the verbs and shapes of `xlsxdown-node` (`spreadsheetCells` and the rest) and a class `XlsDown` | Code written for one kind of workbook reads the other |
| 9 | `mimedown` | `application/vnd.ms-excel` routes to `xlsdown` (`Parser::Xlsdown`) | |
| 10 | New dependency | None from outside the workspace | |
| 11 | Fixtures | None added by me. LibreOffice on this machine writes `.xls` from the public `.xlsx` fixtures, for checks in scratch; the tests build their records in code | Your standing rule |

## 2. What exists to build on

- **`rtfdown/src/cfb.rs`**: reads a compound file's streams, with sectors of 512 and of 4,096 bytes, the mini stream, and a guard against a chain that leads back on itself. It becomes `cfbdown`, and gains storages inside storages, which a Word file needs.
- **`xlsxdown`**: the model, the number-format renderer (`format_code`), the sheet's geometry and pages (`sheet_layout`), and the invoice read from them.
- **`mimedown`**: already names a `.xls` by the `Workbook` or `Book` stream in its compound file.

## 3. The file

The `Workbook` stream is a run of records, each a number, a length, and that many bytes. The workbook's own come first, then each sheet's, each run opened by `BOF` and closed by `EOF`.

- **The workbook's**: the sheets and their names (`BOUNDSHEET`), the shared strings (`SST`, which runs on across `CONTINUE` records and may change how a string is stored in the middle of it), fonts (`FONT`), number formats (`FORMAT`), cell formats (`XF`), and the date system (`DATEMODE`).
- **A sheet's**: its extent (`DIMENSIONS`), rows and columns (`ROW`, `COLINFO`), cells (`NUMBER`, `RK`, `MULRK`, `LABELSST`, `LABEL`, `BOOLERR`, `BLANK`, `MULBLANK`), formulas with their last value (`FORMULA`, and `STRING` after one that gave text), and merged ranges (`MERGEDCELLS`).

A record no step reads is passed over by its length.

## 4. Order of work

1. `cfbdown`, with `rtfdown` moved onto it. No answer changes.
2. The workbook's own records: sheets, shared strings, fonts, formats, and the date system.
3. A sheet's cells, with merged ranges, column widths, and row heights.
4. `xlsxdown`'s model filled from them, so that cells, geometry, pages, and the invoice follow.
5. `xlsdown-node`, and the route in `mimedown`.

## 5. What is built

- **`cfbdown`** (18 tests): `CompoundFile::open`, a stream by its path (`ObjectPool/_1/Package`, names matched whatever their case), what a storage holds, every stream by its own name (what `rtfdown` asks for), and a storage handed back as a compound file of its own. `compound` writes a file, with the entries of each storage in the order the format looks them up in. The writer is public because every crate's tests build their files with it, and `docdown` needs it to give back an embedded workbook.
- **`xlsxdown`**: `Cell::styled` and `Stored` are public. They are how a number is typed and shown through its number format, and `xlsdown` calls them, so the two readers cannot drift. `Workbook` has `has_macros` and `is_template`.
- **`xlsdown`** (24 tests): `xlsdown::open(bytes)` gives an `xlsxdown::Workbook`. Read: sheets with how each is shown, shared strings (across records, with rich text and phonetic notes passed over), fonts, number formats, cell formats, the date system, numbers in each of the three ways a sheet keeps them, text, booleans, errors, formulas by their last value, the sheet's extent, column widths, row heights, hidden rows and columns, merged ranges, print areas, and counts of pictures, charts, links, comments, and pivot tables. A chart's own records inside a sheet are not the sheet's cells.
- **Formulas**: a cell with a formula of its own has `formula` present and empty; one that follows a shared formula has none and names the cell that holds it. Counts follow from that.
- **Refused by name**: an encrypted workbook, an Excel 5 or 95 workbook, and an Excel 2, 3, or 4 worksheet.
- **`xlsdown-node`**: `XlsDown` with the verbs of `XlsxDown`, and `spreadsheetCells` with its async twin. Its specs build `.xls` files in code (`__test__/xls-test.ts`): 17 native, 5 WebAssembly.
- **`@d0paminedriven/xlsdown-wasm`** (`crates/xlsdown-node/wasm`): the WebAssembly-only package, laid out as `rtfdown-node/wasm` is, with both builds, the worker entry (`XlsWorker`), and its tests: 21, of which 4 run on a page in Chrome. The worker answers `spreadsheetCellsAsync`, which is the one verb the binding has as a function; the rest are methods of `XlsDown`, which does not cross to a worker.
- **`mimedown`**: `application/vnd.ms-excel` names `xlsdown` as its parser.
- **Checked against LibreOffice**: the public `.xlsx` fixtures written out as `.xls` in scratch read the same, cell for cell, apart from what LibreOffice itself changed in writing them.

Not read yet, beyond what is parked below: what the workbook says about itself (title, author, and dates are in a property stream of their own), tables, and whether a file is a template (an `.xlt` holds nothing that says so).

## Parked

- A formula's text.
- Charts, pivot tables, macros, pictures, and embedded objects.
- Excel 95 and earlier.
- Reading a workbook saved with a password.
