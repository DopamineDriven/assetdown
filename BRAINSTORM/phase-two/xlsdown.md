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
| 5 | Formulas | The value Excel last computed, which the file keeps beside each formula, and the formula's text, written out from its tokens. The text was added on 2026-11-23, at your word: it lets a total be checked against its line items, and says which cells they are | A reader of the sheet wants the value; an audit wants the formula |
| 6 | Dates | The workbook's date system (1900 or 1904) and the cell's number format, as in `xlsxdown` | The same rule, in the same code |
| 7 | A password | Excel's own password is tried, since Excel opens such a file without asking; any other is refused, and named: `unsupported format: an encrypted workbook` | A password option is a feature of its own |
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
- **Formulas** (their text since 2026-11-23, after 0.1.0): `crates/xlsdown/src/formula.rs` writes a formula out from the tokens the file keeps it as, in the spelling `xlsxdown` gives. Operators, brackets, references with which parts stay put, ranges of whole rows and columns, functions by name (a table of them, with how many arguments each fixed one takes), the workbook's names, other sheets and ranges of sheets (quoted where the name needs it), sheets and names of other workbooks, add-in functions, and lists of constants. A cell that shares a formula has it written for itself and names the cell it is kept for, within the budget `xlsxdown` has for such text; a formula that fills a range together is on each of its cells. What cannot be written out (a table of what-ifs, a token not known) leaves the text empty, and the sheet says how many. Spaces typed into a formula are not kept. Checked on real workbooks against LibreOffice's conversion: every formula was written out, and the only differences were two things LibreOffice rewrites (a reference to a deleted cell of another sheet, and an argument left out).
- **Refused by name**: a workbook locked with a password of its owner's choosing, an Excel 5 or 95 workbook, and an Excel 2, 3, or 4 worksheet.
- **`xlsdown-node`**: `XlsDown` with the verbs of `XlsxDown`, and `spreadsheetCells` with its async twin. Its specs build `.xls` files in code (`__test__/xls-test.ts`): 17 native, 5 WebAssembly.
- **`@d0paminedriven/xlsdown-wasm`** (`crates/xlsdown-node/wasm`): the WebAssembly-only package, laid out as `rtfdown-node/wasm` is, with both builds, the worker entry (`XlsWorker`), and its tests: 21, of which 4 run on a page in Chrome. The worker answers `spreadsheetCellsAsync`, which is the one verb the binding has as a function; the rest are methods of `XlsDown`, which does not cross to a worker.
- **`mimedown`**: `application/vnd.ms-excel` names `xlsdown` as its parser.
- **Checked against LibreOffice**: the public `.xlsx` fixtures written out as `.xls` in scratch read the same, cell for cell, apart from what LibreOffice itself changed in writing them.

- **What the workbook says about itself** (2026-11-23, after 0.1.0 was published): its title, subject, author, keywords, comments, category, language, who saved it last, its revision, when it was created and saved, the application and its version, and the company. They are in two property streams beside the `Workbook` stream, and `cfbdown` reads them (`CompoundFile::summary`), so `docdown` can use the same reader. Text is read in the code page its stream states, and left out where the page is not known and the text is more than plain ASCII. A moment is given to the second, in universal time, as the file keeps it. `cfbdown` depends on `encoding_rs` for this.
- **Tables** are counted (the two records Excel 2003 and later keep a table in).
- **`xlsdown-node`'s specs** hold its async methods to what `xlsxdown-node`'s are held to: each resolves to what its twin returns under every choice of sheets, many readers at once agree, a buffer overwritten after the call does no harm, and a rejected call leaves the rest answering. 29 native specs.
- **A workbook's total and line items, from its formulas** (2026-11-24, after 0.2.0; in `xlsxdown`, so `.xlsx` and `.xls` share it). A page has labels; a workbook also has formulas, and they say what is added to what.
  - `crates/xlsxdown/src/formula.rs`, `references`: the cells and ranges a formula's text reads, each with its sheet where it names one. Other workbooks and runs of sheets are passed over.
  - `crates/xlsxdown/src/audit.rs`, `Workbook::audit`: the total is, of the formulas nothing else reads on the sheets that are shown, the one that reaches the most cells (then one shown as money, then the lowest on its sheet). It is followed down: a formula that only adds cells up (`SUM`, `+`, `-`, a rounding of the whole, the kind of `SUBTOTAL` that is a sum) is a subtotal, and anything else is a line. What is taken away is a line below nought; lines of nought are left out; other sheets are followed, hidden ones included. A line that multiplies a total (hours in all, times a rate) gives way to the rows that come to the same under another sum. A total that is one cell times another is the one line itself; a total worked out any other way has no lines. Each line has a description (the text of its row, or the heading over its column) with the cells it was read from, and, where it is one cell times another and one of the two is shown as money, which is how many and which is the price.
  - `crates/xlsxdown/src/invoice.rs`, `Workbook::invoice`: the pages are read by their labels as before, then a formula total that is not nought becomes `invoice_total`, and its lines become `items`, each field with the box of its cell on its sheet's page. A warning says when the labels gave another total, when the lines do not come to the total, and when the total has no lines to read. The total is surest when the labels read it too, then when its lines come to it. A total below nought is a credit note, reported as an amount as the label reader reports one, with the lines turned to match. Amounts are given to fifteen figures, as a spreadsheet shows a number in full.
  - Two limits guard against a file made to be slow: how many cells the formulas read between them, and how many cells the following looks at. Past either, the workbook is read by its labels alone.
  - `contentdown` gained `document_confidence` (the reader's own rule, now callable after a key field changes) and no longer takes a thirty-first of April for a date.
- **Number formats** (2026-11-24, in `xlsxdown`, so both readers): a code marked only with its language (`[$-409]`, `[$-en-GB]`, and the two marks for the reader's own long date and time) is rendered where the language changes nothing, which is any English, or a code that names no month, day, or half of the day. Another calendar or set of digits is still not rendered. A date or a time whose code is not rendered is shown plainly (`2015-08-31 12:00:00`) and no longer as the count of days it is stored as; the cell still says its code was not rendered, and `raw` still holds what is stored. Not rendered yet: conditions (`[<=9999999]`, which phone-number formats use), fractions, scientific notation, elapsed time, and fractions of a second.
- **Locked workbooks** (2026-11-24, after 0.3.0): `crates/xlsdown/src/crypt.rs`. Excel locks a workbook with its own password, `VelvetSweatshop`, when it must encrypt one and is given none (a protected structure, a file saved to be opened read-only), and opens such a file without asking; so does `xlsdown::open`. The record that states the lock is read (`Lock`), the password is checked against its verifier, and the records are decrypted before anything reads them. Both RC4 kinds: the key made by MD5 (Excel 97 and 2000), and by SHA-1 ("CryptoAPI", Excel 2002 and 2003). The stream is encrypted as if all of it were, with a fresh key for every 1024 bytes; record headers, seven records, and where each sheet's records start are left as they are, and their places in the key's stream are passed over. `md-5` (already in the lock file, through `lopdf`) and `sha1` are the two new dependencies; RC4 is written here.
  - How it was checked: the MD5 kind against files LibreOffice locked (the verifier, and three records past the first 1024 bytes, are in the tests as known answers), and privately against real workbooks, cell for cell beside LibreOffice's own unlocked copies. The SHA-1 kind has no real file behind it: LibreOffice does not write it, so its tests lock and unlock workbooks built in code, which shows the two halves agree and not that Excel would agree with them. A wrong password cannot slip through either kind, because the verifier is checked first.
  - Not opened: a password of the owner's choosing (there is no option to give one), and the older hiding by XOR.
- **What a retest against a system of record changed** (2026-11-24, after `xlsxdown` 0.6.0; all in `xlsxdown`, so both readers). The first cut read a formula by what was left of its text once the references were taken out, which a guard or a wrapper defeated. Now:
  - `crates/xlsxdown/src/audit/parse.rs` reads a formula into the little of its grammar that matters (references, numbers, text, signs, calls) and answers two questions: does it come to a sum of cells, and is it one cell times another. A sum is seen through a rounding, `TEXT`, `VALUE`, `N`, a joining to words, `IFERROR` with nothing as its fallback, and an `IF` whose other answer shows nothing, or whose two answers are the same sum written two ways. What an `IF` asks about is read but is not added. A minus before a sum or a bracket takes away everything in it.
  - A total written out as text (`TEXT(SUM(…),…)`, alone or joined to a currency) is one of the formulas, worth the sum of the cells it adds up. Before, it was left out, and one of its own lines was taken for the total.
  - The total is chosen in this order: a formula shown as money before one that is not, then the one that reaches the most cells, then the lowest on its sheet. Before, reach came first, and a column of hours could beat the amounts.
  - **Tried and taken out again (released in 0.7.0, removed the next day):** asking the formulas first for one that comes to the amount the labels gave. Scored against a system of record it made amounts worse, and run here over every workbook with the rule on and off, it was that rule and no other that moved the totals. A label that reads as a total is often on a part of the bill, and where labels and formulas part, the formulas are right far more often. The labels now only confirm (the total is surest when they read the same amount) and never choose.
  - `invoice` leaves the labels' reading as it was for a formula total under half a cent (adding in binary leaves a little of a nought), and for one shown as a plain number beside a labeled amount shown as money, with a warning.
  - An amount is given as its cell shows it where the cell shows cents (`as_shown` in `invoice.rs`); otherwise to fifteen figures, as before.
  - Of two cells multiplied with the formats not saying which is the price, the one that is a sum of cells is how many.
  - `format.rs`: a language mark that also names the everyday calendar or the plain digits (`[$-10409]`) renders like one that does not. Other calendars and digits are still not rendered.
  - Left as they are, on purpose: two items that cancel (a charge and the credit that undoes it) are both on the sheet, so both are items. Not done: elapsed time (`[h]:mm:ss`), and conditions (`[<=9999999]`, with digits spread across literals, which the phone-number format needs).
  - Found while checking, and older than this work: the label reader takes seconds on a sheet of a hundred thousand cells and more (a list, not an invoice). The time is in `contentdown`, not in the audit.

Not read yet, beyond what is parked below: whether a file is a template (an `.xlt` holds nothing that says so).

## Parked

- Charts, pivot tables, macros, pictures, and embedded objects.
- Excel 95 and earlier.
- Reading a workbook locked with a password of its owner's choosing (an option to give one), and one hidden by XOR.
