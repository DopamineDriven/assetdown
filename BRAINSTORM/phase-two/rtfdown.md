# rtfdown: notes

Status: **steps 2 to 5 of section 5 are built** (2026-11-20): the crate, with the files embedded in a document, its pictures, its text, its pages, and the invoice read from them; the binding; and the route in `mimedown`. Sections 6 to 8 say what exists. Step 1, moving `mimedown`'s reader here, waits on row 3, which is yours to decide. Markdown, the other half of step 3, is not built.

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

## 6. As built (2026-11-19)

You said to create the crate, so I did: `crates/rtfdown`, its entries in the root `Cargo.toml`, and a manifest that depends on `thiserror` and nothing else yet. `contentdown` and `encoding_rs` go in when the text and the layout need them.

Step 2 came before step 1, because step 1 changes what `mimedown` is built from and row 3 is not decided. Until it is, the two crates each have a reader of their own: this one reads groups, objects, and pictures, and `mimedown`'s reads text and the facts it reports.

What exists:

- **`token`**: RTF as a run of tokens: a group opening or closing, a control word with its number, a symbol, a byte in hex, the bytes after `\bin`, and text.
- **`cfb`**: the streams of a compound file, each by its name. Sectors of 512 bytes and of 4,096, the small streams kept in the mini stream, and a chain that leads back on itself not followed round.
- **`Rtf::open(bytes)`**, then `embedded()`, `pictures()`, and `warnings()`.

An embedded file is given as it was before it was embedded, with the class of the object that held it:

| What the object's data is | What comes out |
| --- | --- |
| A compound file with an `AttachContents` stream (a mail attachment) | That stream |
| A compound file with a `Package` stream (a document of 2007 or later) | That stream |
| A compound file with a `CONTENTS` stream (what Acrobat embeds) | That stream |
| A compound file with an `Ole10Native` stream, or the packager's own layout | The packaged file, with the name on its label |
| Any other compound file (a document or a workbook of before 2007) | The compound file, which is the document |
| Anything else | The object's data as it is |

An attachment whose place is marked in the text and that is not in the file is said in a warning.

A picture is given as its bytes, with its format and the size it is to be shown at. The picture that stands for an embedded object is not among them, and of the two copies Word writes of a picture the newer is given. A group inside a picture group (a shape's properties, an identifier) is not read as the picture's data. A picture's format is what its bytes open as, where they open as a PNG or a JPEG: LibreOffice marks the pattern a shape is filled with as a PNG, and it is not one.

Checked on a file LibreOffice made from a public Word fixture: five pictures found of the six groups it writes, the sixth being the older copy of the first; the EMF and the two PNGs open as what they are, and the two fill patterns are given as of no known format.

28 tests. Test-first held, with one thing to say: the test that the picture standing for an object is not counted passed against the placeholder, and was first seen to matter once pictures were counted.

## 7. The text, the pages, and the invoice, as built (2026-11-20)

The manifest gained `encoding_rs` and `contentdown`, as section 3 of [config-updates.md](config-updates.md) said it would.

- **`body`**: the document's paragraphs and the rows of its tables. A run keeps its size, whether it is bold or italic, and its font's name; a paragraph how it is lined up and indented, and the space the file states above it and below it; a row the edge each cell ends at. Bytes are read in the code page the file states, or the one the run's font states, two bytes to a character where the page has such characters; a character given by its number is read with the stand-ins after it passed over. The tables of fonts, colors, and styles, the document's own facts, pictures, footnotes, a field's instruction, and any group marked to be passed over are not the document's text.
- **Headers and footers** are read apart from the text (`Rtf::running()`): the one for every page, and the first page's own where the file says its first page has one (`\titlepg`). The first the file states of each kind is the document's; one a later section states for itself is not followed, and one for left pages only is passed over.
- **`Rtf::text()`**: a line to a paragraph, and a line to a row with a tab between its cells. The headers and footers are not in it, as they are not in `docxdown`'s.
- **`Rtf::layout()`**: the pages, by estimate, as `docxdown` makes them: a paragraph wrapped between the margins the file states, the cells of a row between their edges, a page turned at a page break and at the foot of the page, with the header set half an inch from the top of each and the footer ending half an inch from its foot. Paragraphs are as far apart as the file says and no farther: the format's own default is no space, so nothing is estimated between them. Every page says its positions are estimated.
- **A paragraph the file gives a place of its own** (`\posx`, `\posy`, `\absw`, measured from the page or from the margins) is set at that place and moves nothing else. A program that writes a report writes every line this way, and for such a file the places are the file's own and no estimate. Paragraphs one after another that are placed in the same words are one frame, which is the format's rule, and are set one under the other; a frame that differs in anything, its height included, is another.
- **`Rtf::invoice(options)`**: `contentdown::invoice::extract_from_pages` over those pages. When the document carries embedded files, the first of its warnings says so: they are not read here, and the invoice may be one of them.

Checked on RTF files LibreOffice made from the fourteen public Word fixtures:

| Against | What | Result |
|---|---|---|
| LibreOffice's own text of each | words of its 13,807 that are in `text()` | 98.6%, and `text()` gives the same number of words |
| LibreOffice's PDF of each, read by `pdftotext` | words of its 13,975 that are on `layout()`'s pages | 98.7% |
| the same | pairs of words that are one above the other on both sides alike | 99.4% of 556,536 pairs |
| the same | files with the same number of pages | 8 of 14 |

The page counts differ where the heights do. The one far out is a document that is mostly pictures, set here on 2 pages against 13: a picture is given no room.

Two things about checking against LibreOffice, learned the hard way. Its text export leaves out everything in a frame, so a file made of placed paragraphs comes out nearly empty. And its PDF drops the lines of a frame that do not fit the height the file gives the frame, so the PDF has fewer words than the file. The document as LibreOffice holds it before it draws anything, saved as OpenDocument (`--convert-to odt`, then the text of `content.xml`), has them all, and is the one to count words against.

Not yet read, each of which moves words on a page that is set by estimate: tab stops the file sets (every tab goes to the next half inch), the spacing of lines, a row's stated height, the room a picture takes, and how far a header is from the edge where the file says. Text inside a text box is left out: it lies in a group marked to be passed over.

63 tests. Test-first held, with these to say:

- Two of the tests for escapes were wrong as first written (a surrogate's number, and a space that is a word's own ending) and were corrected before the reader was written.
- The test for two frames in one spot was first written against the format's rule (it took a frame stated again after `\pard` to be a new one) and passed on arrival; it was rewritten to the rule before the code was.
- One test of headers gave itself an empty paragraph by its own input and was corrected.
- The test of turning pages was changed on purpose: it counted on the space once estimated between paragraphs, which is gone.

## 8. The binding and the route, as built (2026-11-20)

`crates/rtfdown-node` is `@d0paminedriven/rtfdown`: no feature, no OCR package, and nothing a caller sets up.

| Export | Gives |
|---|---|
| `rtfText`, `rtfTextAsync` | the text |
| `rtfLayout`, `rtfLayoutAsync` | the pages: words and lines with their boxes |
| `rtfInvoice`, `rtfInvoiceAsync` | the invoice's fields, with the options every binding takes |
| `rtfEmbedded`, `rtfEmbeddedAsync` | the files the document carries: class, name, and bytes |
| `rtfPictures`, `rtfPicturesAsync` | the pictures: format, bytes, and the size each is shown at, in points |
| `rtfDocument`, `rtfDocumentAsync` | all of it in one object, with the warnings |
| `RtfDown` | the class: parsed once in its constructor or in `openAsync`, with the same verbs as methods |

`src/invoice.rs` is the copy every binding has. The specs build each document as the string it is, so the package has no fixture: 22 tests against the native addon, and 6 against the WebAssembly one. Its `ci` folder and `.github/workflows/rtfdown-node.yml` are `docxdown-node`'s with the names changed, and `version.sh` knows it.

In `mimedown`, `application/rtf` now names `rtfdown` as the package that reads it (`Parser::Rtfdown`), in the library and in the binding.

What is yours to settle in the package's manifest is in section 3 of [config-updates.md](config-updates.md).

## Parked

- Writing RTF.
- Drawing objects other than text boxes and pictures.
- Rendering a WMF or EMF picture. It is returned as its bytes.

## Not verified

- The `Package` layout and the Office classes in section 4: read about, not run. The tests build each from its description.
- A mail attachment's name: its `AttachDesc` stream holds it, in a layout not yet read, so a mail attachment comes out with no name.
- No real file with an embedded object was read by this build.
- How well `flow`'s estimate holds for an RTF invoice that lines its columns up with tabs.
- Sections: a header or a footer a later section states for itself, and a first page of its own in any section but the first.
