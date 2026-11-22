# xlsdown: notes

Status: **planned** (2026-11-22). Nothing is built.

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

## Parked

- A formula's text.
- Charts, pivot tables, macros, pictures, and embedded objects.
- Excel 95 and earlier.
- Reading a workbook saved with a password.
