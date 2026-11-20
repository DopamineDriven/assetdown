# rtfdown: notes

Status: **not started** (2026-11-18). These are notes toward a plan. Rows marked decided are decided; the rest are recommendations waiting for an answer. The crate and its binding are yours to create.

Some invoices are RTF files: a word processor's export, or a mail message saved as RTF with the invoice attached inside it. `rtfdown` reads them. Its two companions have notes of their own: [tiffdown.md](tiffdown.md) and [jpegdown.md](jpegdown.md).

## 1. Decisions

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | A crate of its own | **Decided** (2026-11-18). `rtfdown`, with `rtfdown-node` | Your call |
| 2 | Scope | Bytes in. The document's text, tables, pictures, and metadata; the files embedded in it; its pages laid out by estimate, and the invoice read from them | The verbs of `docxdown`, for the same kind of document |
| 3 | One tokenizer | `mimedown`'s RTF reader (`crates/mimedown/src/document/rtf.rs`) moves here and grows. `mimedown` then gets its RTF facts from `rtfdown` behind a feature, as it gets a Word file's from `docxdown` | Two readers of the same escapes would drift apart. The cost: `mimedown` reads RTF facts with no feature today, and would name the type and no more without the new one |
| 4 | Embedded files | Returned as they are: a name, a class, and the bytes. `rtfdown` does not read them. The caller sniffs each and hands it to its parser, and the invoice answer warns when the document carries one | Each parser reads its own format. An attached PDF is `pdfdown`'s |
| 5 | The compound file | A small reader of streams inside `rtfdown`: the header, the allocation tables, the directory | An embedded object is a compound file. `mimedown`'s `cfb` lists names for sniffing and reads no stream, and a parser may not depend on `mimedown`. If legacy Office or `.msg` gets a crate later, the reader moves to a shared one then |
| 6 | Code pages | `encoding_rs`, which the workspace already has, for every code page and font character set other than 1252, the double-byte ones included | `mimedown`'s reader carries a table for 1252 only |
| 7 | Layout | Text set by `contentdown::flow` inside the page and margins the file states, as `docxdown` does. Table cells at the edges the file gives, and a text box at the box its shape states | RTF records no line breaks. It does record cell edges and shape boxes exactly |
| 8 | Invoice | `contentdown::invoice::extract_invoice` over those pages | No new extraction code |
| 9 | `mimedown` | `application/rtf` routes to `rtfdown` (`Parser::Rtfdown`) | |
| 10 | New dependency | None. `encoding_rs` becomes a dependency of one more crate | |
| 11 | Fixtures | None added by me. LibreOffice on this machine writes RTF from the public Word fixtures, for checks in scratch; an embedded object is built in code | Your standing rule |

## 2. What exists to build on

- **`mimedown`'s reader** (642 lines, with its tests). It already handles groups and their depth, destinations (the body, asides, shapes, the info group), `\ansicpg` and the 1252 table, hex escapes, `\u` with the stand-ins `\uc` counts, `\upr`, the language a run states, the info fields with their dates, the word count, the preview, and the picture count. Checked against LibreOffice and `exiftool` on real files.
- **`contentdown`**: `flow` (text set by estimate), `markdown`, `counting` (list numbers), `paper_size`, and the invoice extraction.
- **`docxdown`**, as the shape to follow. Its binding's verbs: `metadata`, `text`, `textPerPage`, `blocks`, `markdown`, `tables`, `images`, `annotations`, `formFields`, `notes`, `layout`, `invoice`, `document`.

## 3. The model

What the reader has to keep, beyond the facts `mimedown` takes today:

- Paragraphs and runs: bold, italic, size, font, alignment, indents, tab stops.
- Tables: `\trowd` to `\row`, the cell edges `\cellx` gives, merged cells, a table inside a cell.
- Lists: the label the file wrote (`\listtext`), and the older `\pn` form.
- Sections and the page: paper size, margins, columns, and headers and footers.
- Footnotes, comments, and fields (a link's address and the text shown for it).
- Text boxes and shapes, with the box each one states.
- Pictures: `\pict` as PNG, JPEG, EMF, or WMF, in hex or as `\bin` bytes.
- Embedded objects (section 4).

`annotations`, `formFields`, and `notes` have RTF counterparts and come last.

## 4. Embedded objects

An object is a group, `\object`, holding its class (`\objclass`), its data in hex (`\objdata`), and a `\result` with the picture shown in its place.

The data begins with a short header: a version, a format number, the class name, two more names, and the size of what follows. What follows depends on the class:

- **A mail attachment** (`Outlook.FileAttach`): a compound file. Its `AttachContents` stream is the attached file, byte for byte, and `AttachDesc` holds its names. Confirmed on real files on 2026-11-18: the attachment came out whole.
- **`Package`**: the packager's own layout of a label, a path, and the bytes. From the format's description; not yet seen.
- **An Office class**: the compound file is the document itself, or holds it in a `Package` stream.

A message converted from the mail store can carry a placeholder in the body (`\objattph`) and no data: the attachment was never in the file. That is said in a warning.

The picture in `\result` stands for the object and is not counted among the document's pictures.

## 5. Order of work

1. The crate, with `mimedown`'s reader moved into it and `mimedown` calling it. No answer changes.
2. Embedded files and pictures. Small, and it recovers an attached invoice outright.
3. The model: paragraphs, runs, and tables, to text and Markdown.
4. Layout, pages, and the invoice.
5. `rtfdown-node`, and the route in `mimedown`.

## Parked

- Writing RTF.
- Drawing objects other than text boxes and pictures.
- Rendering a WMF or EMF picture. It is returned as its bytes.

## Not verified

- The `Package` layout and the Office classes in section 4: read about, not run.
- How well `flow`'s estimate holds for an RTF invoice that lines its columns up with tabs.
