# @d0paminedriven/pptxdown

[![CI](https://github.com/DopamineDriven/assetdown/actions/workflows/pptxdown-node.yml/badge.svg?branch=main)](https://github.com/DopamineDriven/assetdown/actions/workflows/pptxdown-node.yml)

Reads PowerPoint decks (`.pptx`, `.pptm`, `.potx`, `.ppsx`) in Node: plain text, Markdown, each slide's shapes, tables, pictures, links and comments, speaker notes, charts as data, audio and video clips, metadata and invoice fields.

It is a Rust parser shipped as a native addon, with a WebAssembly build for platforms that have none. There is nothing else to install and nothing to configure: no PowerPoint, no LibreOffice.

```sh
npm install @d0paminedriven/pptxdown
yarn add @d0paminedriven/pptxdown
pnpm add @d0paminedriven/pptxdown
```

node &ge; 24

## Quick start

```ts
import { readFile } from "node:fs/promises";

import { PptxDown } from "@d0paminedriven/pptxdown";

const deck = new PptxDown(await readFile("deck.pptx"));

deck.metadata().slideCount; // 13
deck.slides()[0]?.title; // "Horizon-Mediated Episodic Memory"
deck.markdown();
deck.tables()[0]?.rows["2"]?.["B"]?.text;
deck.layout()[0]?.lines[0]?.text; // "CONVERSATION MEMORY SUBSYSTEM"
```

For a slide with a title, a label in bold above it, a sentence, a picture and a line at the bottom, `markdown()` writes the title as the heading and the rest under it from the top of the slide down:

```text
## Horizon-Mediated Episodic Memory

**CONVERSATION MEMORY SUBSYSTEM**

HMEM turns every conversation into a self-condensing archive.

![ellipse](<ppt/media/image-1-1.png>)

Live in apps/ws-server
```

The deck is parsed once, in the constructor, and every method reads from the parsed deck. `await PptxDown.openAsync(buffer)` does the parsing off the JavaScript thread.

## Readers

| Method             | Returns                                                                                                |
| ------------------ | ------------------------------------------------------------------------------------------------------ |
| `metadata()`       | Title, author and dates; the slide's size and the paper it is nearest to; slide, word and other counts |
| `text()`           | The deck as plain text, slide by slide                                                                 |
| `textPerPage()`    | `{ page, text }` for each slide                                                                        |
| `structuredText()` | `{ page, header, body, footer }` for each slide: what repeats on every slide, apart from the rest      |
| `slides()`         | Each slide with its title, its layout and the shapes on it                                             |
| `markdown()`       | A heading for each slide, then lists, emphasis, links, pipe tables, pictures and notes                 |
| `tables()`         | Each table, keyed by row and then column                                                               |
| `images()`         | Each picture where it is placed, with its bytes and its box                                            |
| `annotations()`    | Hyperlinks and comments                                                                                |
| `notes()`          | Speaker notes, for each slide that has any                                                             |
| `charts()`         | Each chart as data: series, categories and values                                                      |
| `media()`          | Each audio or video clip where it is placed, with its bytes                                            |
| `layout()`         | Slides as pages of positioned words and picture boxes                                                  |
| `invoice(opts?)`   | Invoice fields, read from those pages                                                                  |
| `document()`       | Everything above except `layout` and `invoice`, in one object                                          |

Each reader comes in four forms:

```ts
import {
  PptxDown,
  pptxTables,
  pptxTablesAsync
} from "@d0paminedriven/pptxdown";

deck.tables(); // on a parsed deck
await deck.tablesAsync(); // the same, on a worker thread
pptxTables(buffer); // parse and read in one call
await pptxTablesAsync(buffer); // the same, on a worker thread
```

The free functions are `pptxMetadata`, `pptxText`, `pptxTextPerPage`, `pptxStructuredText`, `pptxSlides`, `pptxMarkdown`, `pptxTables`, `pptxImages`, `pptxAnnotations`, `pptxNotes`, `pptxCharts`, `pptxMedia`, `pptxLayout`, `pptxInvoice` and `pptxDocument`.

## A slide is a page

Every reader that names a `page` means the slide's number, from 1, in the order the deck shows its slides. That order is the deck's own list, not the names of the files inside it. Unlike a Word document, a deck records its pages, so the number is exact.

A hidden slide keeps its place and its number, and is returned with `hidden: true`. To leave hidden slides out:

```ts
new PptxDown(buffer, { hiddenSlides: false });
```

The slides after a hidden one keep their numbers, and `metadata().slideCount` still counts it. The option is on the constructor and `openAsync`. The free functions read every slide.

## What counts as text

The reader takes text wherever a slide keeps it, and skips only what someone looking at the slide would not see.

| Taken                                                           | Skipped                                                                    |
| --------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Text boxes, placeholders, table cells, and shapes inside groups | Prompt text a layout holds for an empty placeholder ("Click to add title") |
| SmartArt text                                                   | The second copy of a shape kept for older applications                     |
| The text of fields as last saved (a slide number, a date)       | Anything on a layout or a master that the slide does not fill              |
| Equations, as their characters                                  |                                                                            |

A slide is read the way it is looked at: the title first, then the rest from top to bottom, and left to right where shapes sit side by side. The file stores shapes in the order they were stacked, so a caption added last would otherwise come last; each shape keeps that order as `stacking`.

`text()` and `textPerPage()` leave out the date, footer and slide number that repeat on every slide. `structuredText()` returns them apart from the body, as `header` when they sit in the top half of the slide and `footer` when they sit in the bottom half, the way `@d0paminedriven/pdfdown` splits a PDF page. A deck of one slide has nothing that repeats, so all of it is `body`.

Speaker notes are not part of a slide's text. They are in `notes()`, on each slide of `slides()`, and at the end of each slide in `markdown()`.

## Slides and shapes

`slides()` returns each slide with its `page`, `title`, `layout` (the name of the layout it is built on, such as `Title and Content`), `section`, `hidden`, `text`, `notes` and `shapes`.

A shape is a union keyed on `type`, so a check on `type` narrows it:

```ts
import type { Shape } from "@d0paminedriven/pptxdown";

function titles(shapes: Shape[]) {
  return shapes.flatMap((shape) =>
    shape.type === "text" && shape.placeholder === "title" ? [shape.text] : []
  );
}
```

| `type`    | Fields of its own                                                                                |
| --------- | ------------------------------------------------------------------------------------------------ |
| `text`    | `text`, `paragraphs`                                                                             |
| `picture` | `image`, `name`, and `vector` and `background` when they apply                                   |
| `table`   | `columns` (widths in points), `rows` of `cells`                                                  |
| `chart`   | `title`, `series`                                                                                |
| `diagram` | SmartArt: `text`, `shapes` as the application drew it, or `nodes` when the deck keeps no drawing |
| `media`   | `media`, `name`, `kind` (`audio` or `video`), `mime`, `poster`                                   |
| `group`   | `shapes`                                                                                         |

Every shape also has `frame`, `stacking`, `shapeName`, `altText`, `placeholder`, and `uri` or `dest` when the whole shape is a link.

`frame` is the shape's box in points, measured from the slide's top-left corner as PowerPoint measures it: `{ x, y, width, height, rotation }`. A shape inside a group is placed on the slide, not within the group. `rotation` is reported and not applied.

A text shape has its plain `text` beside its `paragraphs`, so a caller who only wants text never walks the runs. A paragraph has `text`, `level`, `bullet` (as it stands on the slide: `•`, `2.`, `b)`), `ordered`, `align` and `runs`. A run has `text` and, when they are set, `bold`, `italic`, `underline`, `strike`, `size`, `font`, `uri` or `dest` for a link, and `field`.

### Layouts and masters

A slide is built on a layout, and a layout on a master. Many placeholders say nothing about where they are or how large their text is; the layout or the master says it for them. The reader follows that chain, so a title has its box and its size, and a body paragraph has its bullet, whether or not the slide states them.

A numbered list is counted and written out (`1.`, `2.`, `a)`) as PowerPoint shows it. A list starts at the number it says it starts at, which is how one is carried on from the slide before or picked up after a line typed by hand. A paragraph with nothing in it shows no number and takes none, and text with no number ends the list.

## Tables

```ts
const [table] = deck.tables();

table?.rows["1"]?.["A"];
// { ref: "A1", row: 0, column: 0, colSpan: 2, rowSpan: 1, text: "Item" }
table?.merges; // ["A1:B1"]
```

Rows are keyed from `"1"` and columns by letter, the way a sheet refers to a cell and the way `@d0paminedriven/docxdown` and `@d0paminedriven/xlsxdown` key theirs. A cell merged into another has no key; the cell it is merged into carries the span, and `merges` names the range (`A1:B2`). Each table has the `page` it is on and its `frame`.

## Images, annotations, charts and clips

```ts
deck.images()[0];
// {
//   page: 1, imageIndex: 0,
//   name: "ppt/media/image-1-1.png", mime: "image/png",
//   width: 640, height: 640,
//   frame: { x: 704, y: 320, width: 160, height: 160, rotation: 0 },
//   altText: "ellipse",
//   linked: false, background: false, data: <Buffer>
// }

deck.annotations()[0];
// {
//   page: 1, part: "slide", subtype: "Link",
//   uri: "https://example.com/terms", text: "terms",
//   rect: [100, 290, 400, 490]
// }
```

`images()` has one entry for each place a picture is used. `data` is the bytes as the deck stores them, not re-encoded, and `mime` says what they are. `width` and `height` are pixels, read from the picture's own header. `frame` is where the picture is on the slide, and so the size it is shown at. A slide's background picture has `background: true`. When the deck keeps an SVG beside a raster copy, the raster copy is `data` and the SVG is `vector`.

A link is on a run of text or on a whole shape. It is one `Link` annotation with the `text` it covers, however many runs it is written as. `uri` is an address outside the deck; `dest` is a place inside it, the number of a slide or a jump such as `nextslide`. `rect` is the box of the shape the link is in, as `[x0, y0, x1, y1]` in the page frame `layout()` uses. A comment, in either of the two forms PowerPoint has written them, is a `Comment` annotation with its `content`, `author` and `date`.

`charts()` returns each chart as data: its `title`, and for each series its `kind` (`bar`, `line`, `pie`, and so on), `name`, `categories` and `values`, which are the values the application last showed. Nothing is drawn, and the workbook kept behind a chart is not opened.

`media()` has one entry for each place an audio or a video clip is used, with its `kind`, `mime`, bytes, `frame` and the `poster` picture shown for it. A linked clip, which lives outside the file, has `linked: true` and no bytes.

## Layout and invoice fields

`layout()` returns each slide as words and lines with boxes in points, the shape `@d0paminedriven/pdfdown` returns for a PDF page: y runs up from the slide's bottom edge.

```ts
deck.layout()[0]?.lines[0];
// { text: "CONVERSATION MEMORY SUBSYSTEM", x0: 64, y0: 461.6, x1: 238, y1: 476, firstWord: 0, wordCount: 3 }
```

A slide records where every shape is, so most of this is the file's own.

| The file's own                                                | Estimated                         |
| ------------------------------------------------------------- | --------------------------------- |
| Which slide a word is on                                      | Where a line wraps inside its box |
| The box every shape, table cell and picture is in             | A font's real widths              |
| Font size, bold and italic, through the layout and the master |                                   |

A character is taken to be half its font size wide and a line 1.2 times its size tall. Text the estimate makes taller than the room left on the slide is set closer together, so every word stays on its page. Each page also has `images`, the boxes of the slide's pictures.

`invoice()` reads those pages with the same extractor, options and result shape as the other packages.

## Slide size and paper

`metadata()` gives the slide's `width` and `height` in points, its `aspectRatio` (`16:9`, `4:3`), `sizeKind` (what the file says the slides are sized for, such as `screen16x9` or `letter`) and `paperSize`, the same union the other packages return.

```ts
deck.metadata().paperSize;
// { type: "UNKNOWN", closestTo: [{ type: "A4", width: 595, height: 842 }, { type: "LEGAL", width: 612, height: 1008 }] }
```

A slide sized for a screen is no paper size, so most decks are `UNKNOWN`, with the two standard sizes the slide lies between by area. A deck sized for paper reports that paper.

Counts come in pairs where the application recorded one of its own: `slideCount` and `slidesRecorded`, `wordCount` and `wordsRecorded`. The reader counts words from the text it returns; the application counts by rules of its own, so the two can differ.

## Errors

The constructor and the free functions throw, and the async forms reject, with a message that starts with its category:

| Message                     | When                                                                                                                                                                    |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unsupported format: …`     | The file is readable but is not a presentation. A workbook or a Word document is named as one, and a legacy `.ppt` or a password-protected deck as an OLE compound file |
| `limit exceeded: …`         | The package or the deck is past a size or nesting limit                                                                                                                 |
| `malformed presentation: …` | Anything else that cannot be read                                                                                                                                       |

A part of a deck that cannot be read (one slide, one chart) is left out and named in `metadata().warnings`; the rest is returned.

A `.pptm` is read like a `.pptx`: its macros are never read or run, and `metadata().hasMacros` is true.

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
