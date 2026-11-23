# docdown: notes

Status: **built** (2026-11-23), not yet released. `docdown` and `docdown-node` are written and tested; see [What is built](#5-what-is-built).

Some documents are in the format Word wrote from 1997 to 2003: a compound file with the text in one stream and what describes it in another. `docdown` reads them. Its companion is [xlsdown.md](xlsdown.md), and both stand on the same compound-file reader.

## 1. Decisions

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | A crate of its own | **Decided** (2026-11-22). `docdown`, with `docdown-node` | Your call |
| 2 | The compound file | `cfbdown`, as row 2 of [xlsdown.md](xlsdown.md) says | One reader for three crates |
| 3 | Which Word | Word 97 to 2003. A file of Word 6 or Word 95 is named for what it is and refused | The older layout keeps its text and its properties differently |
| 4 | The model and the pages | **Recommended.** The paragraphs, runs, rows, paper, and running heads `rtfdown` has, set on pages by the flow it uses. Both move from `rtfdown` into `contentdown`, with no answer changing, and `rtfdown` and `docdown` fill the one model | A `.doc` and an RTF file are two spellings of the same Word document. Two copies of the model and of the page-setting would drift |
| 5 | Text | Read through the piece table, each piece in the encoding it states: one byte to a character in the document's code page, or two | That is where Word 97 and later keep the order of the text |
| 6 | Structure | Paragraph marks, cell and row marks, and page breaks, as the text stream has them. A field keeps its result and drops its instruction | The same as `rtfdown` does with the same marks |
| 7 | Formatting | Bold, italic, size, and font from the character properties, over the style the paragraph names. Alignment, indents, spacing, and a table's cell edges from the paragraph's properties. Paper and margins from the section's | What the page-setting needs, and no more |
| 8 | Headers and footers | The text Word keeps for them, as the running heads of the pages | As `rtfdown` |
| 9 | Embedded files and pictures | The objects under `ObjectPool`, each as it was before it was embedded, by the unwrapping `rtfdown` has (which moves to `cfbdown`). Pictures from the `Data` stream, as their bytes | An attached invoice is recovered outright |
| 10 | A password | Refused, and named | Decryption is a feature of its own |
| 11 | Invoice | `contentdown::invoice` over the pages | No new extraction code |
| 12 | Binding | `docdown-node`, with the verbs of `rtfdown-node` under its own names: `docText`, `docLayout`, `docInvoice`, `docEmbedded`, `docPictures`, `docDocument`, and a class `DocDown` | The same kind of document, the same questions |
| 13 | `mimedown` | `application/msword` routes to `docdown` (`Parser::Docdown`) | |
| 14 | New dependency | None from outside the workspace. `encoding_rs` becomes a dependency of one more crate | |
| 15 | Fixtures | None added by me. LibreOffice on this machine writes `.doc` from the public Word fixtures, for checks in scratch; the tests build their files in code | Your standing rule |

## 2. What exists to build on

- **`cfbdown`**, once it is made: see [xlsdown.md](xlsdown.md).
- **`rtfdown`**: the model of a flowed document (`Block`, `Paragraph`, `Run`, `Row`, `Paper`, `Running`), its page-setting over `contentdown::flow`, and the unwrapping of an embedded object.
- **`mimedown`**: already names a `.doc` by the `WordDocument` stream in its compound file.

## 3. The file

- **`WordDocument`** opens with the file information block: which Word wrote it, which of the two table streams it uses, whether it is encrypted, and where each structure below is.
- **The table stream** (`0Table` or `1Table`) holds the piece table, which says where each stretch of the text is and how it is encoded; the tables that find the character and paragraph properties of any position; the style sheet; the fonts; and the sections.
- **The text** is one run of characters: the body first, then footnotes, then headers and footers. A paragraph ends with a mark, a cell and a row with another, and a field is bracketed by three more.
- **`Data`** holds pictures, and **`ObjectPool`** a storage for each embedded object.

## 4. Order of work

1. `cfbdown` (the first step of `xlsdown` too).
2. The text: the file information block and the piece table, to `text()`.
3. Paragraphs, table rows and cells, page breaks, and fields, to blocks.
4. The model and the page-setting moved from `rtfdown` into `contentdown`. Then character and paragraph properties, styles, and sections, so that the pages are set.
5. Headers and footers, embedded files, and pictures.
6. The invoice, `docdown-node`, and the route in `mimedown`.

## 5. What is built

- **The shared model** (row 4): `rtfdown`'s paragraphs, runs, rows, paper, and running heads, and its page-setting, are `contentdown::document` now, and `rtfdown` and `docdown` both fill it. `rtfdown` gives the same answers as before. A placed paragraph has a third thing its distance down can be measured from: where the text has reached (`DownFrom::Text`), which is what a text box anchored to a paragraph needs.
- **`docdown`** (28 tests): `Doc::open`, `text`, `blocks`, `paper`, `running`, `layout`, `invoice`, `embedded`, `pictures`, `is_template`, `has_macros`, and `warnings`.
  - Text through the piece table, each piece one byte to a character or two.
  - Paragraphs, tabs, line breaks, page breaks, and fields (what a field shows is kept, what it is told is dropped).
  - Table rows, with the edges the file gives each cell, and more than one paragraph to a cell.
  - Alignment, indents, and space before and after; bold, italic, size, and font; hidden text left out.
  - Styles, each starting from the style it is based on, under what a paragraph and a run state themselves.
  - The first section's paper and margins; the header and footer of odd pages, and of the first page where the section says it has its own.
  - **Text boxes**, which were parked and are not any more: a look at real documents showed that many keep part of their text in them. A box's paragraphs are placed where its shape is anchored, and a box that no shape leads to is read after the text.
  - Embedded files under `ObjectPool`: a file the packager left, with its name; a file held whole in a stream; and a workbook or a document of before 2007, given as the compound file it is. The class is what the object calls itself.
  - Pictures: those set in the text, from the `Data` stream, with the size each is shown at, and those of floating shapes, from the drawing's store. A metafile is inflated. The picture that stands for an embedded object is not among them.
- **Refused by name**: a Word 6 or 95 document, an encrypted document, and a compound file that holds no Word document.
- **`docdown-node`**: `docText`, `docLayout`, `docInvoice`, `docEmbedded`, `docPictures`, and `docDocument`, each with an `Async` twin, and class `DocDown`. `docDocument` also says `isTemplate` and `hasMacros`. Its specs build `.doc` files in code (`__test__/doc-test.ts`): 18 native, 4 WebAssembly.
- **`@d0paminedriven/docdown-wasm`** (`crates/docdown-node/wasm`): the WebAssembly-only package, laid out as `rtfdown-node/wasm` is, with both builds, the worker entry (`DocWorker`, with every promise verb), and its tests: 25, of which 4 run on a page in Chrome.
- **`mimedown`**: `application/msword` names `docdown` as its parser.
- **Checked against LibreOffice**: over a set of real documents, the words `docdown` gives were counted against the words of LibreOffice's ODT output, and the counts agreed for every one. (Its plain text export leaves out framed text, so it is not the one to count against.)
- **New dependency**: `flate2`, which the workspace already had, for the compressed metafiles.

Not read yet, beyond what is parked below: sections after the first (their paper, and a break that does not start a page is set as one that does), tab stops, tables inside tables (their text is kept, in the cell that holds them), paragraphs the file frames at a place of their own (they are in the flow), the text boxes of headers, and text a reviewer deleted with changes tracked (it is read as text).

## Parked

- Word 6 and Word 95.
- Footnotes, comments, and tracked changes.
- Drawings: shapes are not drawn, and only their text and their pictures are read.
- Reading a document saved with a password.
