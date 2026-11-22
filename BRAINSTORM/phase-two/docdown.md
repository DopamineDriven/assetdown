# docdown: notes

Status: **planned** (2026-11-22). Nothing is built.

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

## Parked

- Word 6 and Word 95.
- Footnotes, comments, and tracked changes.
- Text boxes and drawings.
- Reading a document saved with a password.
