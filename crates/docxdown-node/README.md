# @d0paminedriven/docxdown

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/docxdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/docxdown-node.yml)

Reads Word documents (`.docx`, `.docm`, `.dotx`) in Node: plain text, Markdown, a tree of blocks, tables, pictures, links and comments, form fields, notes, metadata and invoice fields.

It is a Rust parser shipped as a native addon, with a WebAssembly build for platforms that have none. There is nothing else to install and nothing to configure: no Word, no LibreOffice.

```sh
npm install @d0paminedriven/docxdown
yarn add @d0paminedriven/docxdown
pnpm add @d0paminedriven/docxdown
```

node &ge; 24

## Quick start

```ts
import { readFile } from "node:fs/promises";

import { DocxDown } from "@d0paminedriven/docxdown";

const doc = new DocxDown(await readFile("invoice.docx"));

doc.markdown();
doc.metadata().sections[0]?.paperSize; // { type: "LETTER", width: 612, height: 792 }
doc.tables()[0]?.rows["2"]?.["D"]?.text; // "20.00"
doc.invoice().fields.invoiceId?.value; // "4711"
```

For a short invoice written from Markdown, `markdown()` returns it again, with the title and author lines the document opens with:

```text
**Invoice 4711**

Trial

# Invoice 4711

Bill to **Acme Corp**, see the [terms](<https://example.com/terms>).[^1]

| Item | Qty | Unit price | Total |
| --- | --- | --- | --- |
| Widget | 2 | 10.00 | 20.00 |
| Gadget | 1 | 5.50 | 5.50 |

1. First step
2. Second step
    - nested bullet

Total due: 25.50 USD

[^1]: Net thirty days.
```

The document is parsed once, in the constructor, and every method reads from the parsed document. `await DocxDown.openAsync(buffer)` does the parsing off the JavaScript thread.

## Readers

| Method           | Returns                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------- |
| `metadata()`     | Title, author and dates; each section with its paper size; page, word and other counts      |
| `text()`         | The body as plain text, in reading order                                                    |
| `textPerPage()`  | `{ page, text }` for each page                                                              |
| `blocks()`       | The body as a tree of paragraphs and tables                                                 |
| `markdown()`     | Headings, lists, emphasis, links, pipe tables, and the notes after the body                 |
| `tables()`       | Each table, keyed by row and then column                                                    |
| `images()`       | Each picture where it is placed, with its bytes                                             |
| `annotations()`  | Hyperlinks and comments                                                                     |
| `formFields()`   | Content controls, legacy form fields and merge fields, with their values                    |
| `notes()`        | Footnotes and endnotes                                                                      |
| `layout()`       | Pages of positioned words, by estimate                                                      |
| `invoice(opts?)` | Invoice fields, read from those pages                                                       |
| `document()`     | Everything above except `layout` and `invoice`, in one object, with the headers and footers |

Each reader comes in four forms:

```ts
import {
  DocxDown,
  docxTables,
  docxTablesAsync
} from "@d0paminedriven/docxdown";

doc.tables(); // on a parsed document
await doc.tablesAsync(); // the same, on a worker thread
docxTables(buffer); // parse and read in one call
await docxTablesAsync(buffer); // the same, on a worker thread
```

The free functions are `docxMetadata`, `docxText`, `docxTextPerPage`, `docxBlocks`, `docxMarkdown`, `docxTables`, `docxImages`, `docxAnnotations`, `docxFormFields`, `docxNotes`, `docxLayout`, `docxInvoice` and `docxDocument`.

## What counts as text

The reader takes text wherever the document keeps it, and skips only what a reader of the document would not see.

| Taken                                       | Skipped                                                   |
| ------------------------------------------- | --------------------------------------------------------- |
| The body, tables, and tables inside cells   | Deleted text of a tracked change                          |
| Text boxes, once                            | The second copy of a text box kept for older applications |
| Content controls, hyperlinks, field results | Field codes such as `PAGE` or `MERGEFIELD`                |
| Tracked insertions                          |                                                           |

A text box's paragraphs follow the paragraph the box is anchored in.

## Blocks

`blocks()` returns a union keyed on `type`, so a check on `type` narrows it:

```ts
import type { Block } from "@d0paminedriven/docxdown";

function headings(blocks: Block[]) {
  return blocks.flatMap((block) =>
    block.type === "paragraph" && block.heading ? [block.text] : []
  );
}
```

A `paragraph` has its plain `text` beside its `inlines`, so a caller who only wants text never walks the runs. It also has `style`, `heading` (1 to 9), `list` (`{ level, ordered, marker }`, the marker as it stands on the page: `2.`, `b)`, `•`), `align`, `page` and `section`.

A `table` has `columns` (widths in points) and `rows` of `cells`. A cell holds `blocks` of its own, so a table inside a cell is a table.

Inlines are a union on `type` as well:

| `type`       | Fields                                                                                                                                                                |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `text`       | `text`, and `bold`, `italic`, `underline`, `strike`, `vertical`, `size`, `font`. Formatting is resolved through the styles, and a flag is present only when it is set |
| `link`       | `uri` for an address, `dest` for a bookmark, `text`, `inlines`, `page`                                                                                                |
| `image`      | `image`, `name`, `displayWidth`, `displayHeight`, `altText`, `page`                                                                                                   |
| `noteRef`    | `note`: the note's place in `notes()`                                                                                                                                 |
| `commentRef` | `comment`: the comment's id                                                                                                                                           |
| `break`      | `kind`: `line`, `page` or `column`                                                                                                                                    |
| `tab`        |                                                                                                                                                                       |

## Tables

```ts
const [table] = doc.tables();

table?.rows["2"]?.["D"];
// { ref: "D2", row: 1, column: 3, colSpan: 1, rowSpan: 1, text: "20.00" }
```

Rows are keyed from `"1"` and columns by letter, the way a sheet refers to a cell and the way `spreadsheetGrid()` in `@d0paminedriven/xlsxdown` keys a sheet. A cell merged into another has no key; the cell it is merged into carries the span, and `merges` names the range (`A1:B2`). A table inside a cell is listed after the table that holds it, with `parent` set to that table's `index`.

## Images and annotations

```ts
doc.images()[0];
// {
//   page: 1, part: "body", imageIndex: 0,
//   name: "word/media/image1.jpg", mime: "image/jpeg",
//   width: 606, height: 631,
//   displayWidth: 210, displayHeight: 218.7,
//   linked: false, data: <Buffer>
// }

doc.annotations()[0];
// { page: 1, part: "body", subtype: "Link", uri: "https://example.com/terms", text: "terms" }
```

`images()` has one entry for each place a picture is used. `data` is the bytes as the document stores them, not re-encoded, and `mime` says what they are. `width` and `height` are pixels, read from the picture's own header, and are absent for a vector picture such as an EMF. `displayWidth` and `displayHeight` are the size it is shown at, in points.

A link made in Word, whether stored as a hyperlink or as an older `HYPERLINK` field, is one `Link` annotation with the `text` it covers. A comment is a `Comment` annotation with its `content`, `author`, `date`, and the `text` it is attached to.

Both lists cover the body, notes, comments, headers and footers; `part` says which. `page` is absent in a header or a footer, which belong to no one page.

## Pages

A docx records a flow of text, not pages. Two things in the file say where a page ended: a break the author asked for, and a marker Word leaves at the start of each page when it saves. `page` on a block, a picture or an annotation counts those.

`metadata()` reports three numbers:

| Field           | Meaning                                                                                    |
| --------------- | ------------------------------------------------------------------------------------------ |
| `pageCount`     | The count the application recorded at the last save; the counted one when it recorded none |
| `pagesRecorded` | The recorded count, when there is one                                                      |
| `pagesCounted`  | The pages the reader counted, which is what every `page` value counts in                   |

The two can differ:

- A file that no layout engine has saved carries no markers. Everything in it is page 1.
- A page that starts with an empty paragraph can go unmarked, so the count can fall one short there.

## Layout and invoice fields

`layout()` returns each page as words and lines with boxes in points, the shape `@d0paminedriven/pdfdown` returns for a PDF page. Since a docx records no positions, they are estimated: a paragraph is set left to right and wrapped between its section's margins, a table's cells inside the columns of its grid, and the page's header and footer above and below.

```ts
doc.layout()[0]?.lines[5];
// { text: "Widget 2 10.00 20.00", x0: 77, y0: 572.6, x1: 535, y1: 587, firstWord: 17, wordCount: 4 }
```

| Kept                                            | Not kept                       |
| ----------------------------------------------- | ------------------------------ |
| Reading order                                   | True positions                 |
| The cells of a row on one line, columns aligned | Where Word really broke a line |
| A label beside its value                        | A font's real widths           |
| Bold, and the font size                         |                                |

`invoice()` reads those pages with the same extractor, options and result shape as the other packages.

## Tracked changes

A document is read as it stands with every change accepted: insertions kept, deletions dropped. `metadata().hasTrackedChanges` says whether it has any, and the `view` option reads it as it was before them:

```ts
new DocxDown(buffer, { view: "original" });
```

The option is on the constructor and `openAsync`. The free functions read the final view.

## Errors

The constructor and the free functions throw, and the async forms reject, with a message that starts with its category:

| Message                 | When                                                                                                                                                      |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unsupported format: …` | The file is readable but is not a Word document. A workbook is named as one, and a legacy `.doc` or a password-protected document as an OLE compound file |
| `limit exceeded: …`     | The package or the document is past a size or nesting limit                                                                                               |
| `malformed document: …` | Anything else that cannot be read                                                                                                                         |

A `.docm` is read like a `.docx`: its macros are never read or run, and `metadata().hasMacros` is true.

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
