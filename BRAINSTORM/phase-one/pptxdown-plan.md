# pptxdown: plan

Status: **built.** The library (`crates/pptxdown`), the additions to `ooxmldown` and `contentdown`, and the binding's source, tests and CI (`crates/pptxdown-node`) are written, with every decision in section 1 taken as recommended and your three answers applied. The release is yours: a commit titled `pptxdown@0.1.0`. Where the build differs from this plan, the section says so under **As built**.

Research was done in a scratch folder: raw counts of what 16 decks hold, and a trial of the `pptx` crate, which was already in your cargo cache.

Names of functions and types in the snippets are proposals. The numbers come from 14 decks: three I generated (pandoc; the same deck saved again by LibreOffice; and one written by `ppt-rs`, the crate you pointed at), ten templates that ship with tools on this machine, and one from your notes folder. Two private decks were counted as well; no figure from them is in this file.

## 1. Decisions

Each row has what I recommended. All were taken; rows 4, 6 and 7 are the three you answered.

| # | Question | Recommended | Why |
| --- | --- | --- | --- |
| 1 | What reads the deck? | Our own reader on `ooxmldown` + `roxmltree`, as `xlsxdown` and `docxdown` are built. The `pptx` crate is not a dependency | It refused 4 of 14 decks and cannot reach table text (section 3) |
| 2 | Shape of the content | Slides, each a list of shapes as a union keyed on `type` | A text box, a table and a chart carry different fields; works with `UTR` |
| 3 | Verbs in the first release | Everything in section 6 | They fall out of one model |
| 4 | Order of shapes on a slide | The title first, then by position: top to bottom, left to right. Each shape keeps its stacking order | The file stores shapes back to front, which is not how a slide is read (section 7.3) |
| 5 | Positions | The real ones from the file. Only where lines wrap inside a box is estimated | A pptx records every shape's box, unlike a docx (section 7.12) |
| 6 | Layout and master content | Prompt text ("Click to edit…") never. Footer, date and slide-number placeholders are reported and flagged, and left out of `text()` and `markdown()` | They repeat on every slide: 12 of 14 slides in one template carry both |
| 7 | Hidden slides | Included and flagged; an option leaves them out | They are where backup detail goes. `xlsxdown` leaves hidden sheets out by default, so say if you want the same here |
| 8 | Speaker notes | On each slide and in `notes()`. Not part of `text()`; under each slide in `markdown()` | Notes are not on the slide, and they are often most of what was said |
| 9 | Charts | Data only: kind, title, series, categories and the values the file caches. The reader goes in `ooxmldown` | The same chart part is in xlsx and docx. No rendering, as you decided for xlsx |
| 10 | SmartArt | Read from the diagram's drawing part, which has shapes with boxes; from its data part when there is none | The text is not in the slide at all |
| 11 | Embedded audio and video | `media()` returns the bytes and their MIME type. `pptxdown` does not depend on `audiodown` or `videodown` | The caller sniffs and routes, as with every other type |
| 12 | `invoice()` | Included | Every parser offers the same verbs; it costs little once `layout()` exists |
| 13 | Where DrawingML text and tables live | In `pptxdown`. They move to `ooxmldown` when `xlsxdown` or `docxdown` needs them | Only a pptx uses them as its body |
| 14 | `.pptm`, `.potx`, `.ppsx` | Read their content; macros are never run and `hasMacros` says they exist | The slides are ordinary PresentationML |
| 15 | Async | `AsyncTask`, as `xlsxdown-node` and `docxdown-node` | Same family, same pattern |
| 16 | Names and version | crates `pptxdown`, `pptxdown-node`; npm `@d0paminedriven/pptxdown`; class `PptxDown`; start at `0.1.0` | As the others |

**As built.**

- Row 4: as recommended, which you agreed to.
- Row 6: the date, footer and slide-number placeholders are reported in `slides()`, left out of `text()` and `markdown()`, and set apart from the body by `structuredText()`, as pdfdown does for a PDF page. You asked for the split in a deck of two or more slides; in a deck of one slide everything is body.
- Row 7: hidden slides are included and flagged. `{ hiddenSlides: false }` on the constructor and `openAsync` leaves them out.
- Row 13: two things were shared after all, because docxdown had them already and pptxdown needed the same: `contentdown` gained `flow` (setting text by estimate), `markdown` (escaping, emphasis, links) and `counting` (roman numerals and letters), and `ooxmldown` gained `cell_ref` and `column_letters`. `docxdown` now uses them; its behavior and its tests are unchanged.

## 2. What a pptx is

```text
deck.pptx  (a zip)
├─ [Content_Types].xml
├─ _rels/.rels                       → the main part, and docProps
├─ docProps/core.xml                 title, author, dates
├─ docProps/app.xml                  slides, notes, hidden slides, words, "Widescreen"
└─ ppt/
   ├─ presentation.xml               slide order, slide size, sections
   ├─ slides/slide1.xml              one part per slide: a tree of shapes
   ├─ slideLayouts/slideLayout1.xml  what a kind of slide looks like
   ├─ slideMasters/slideMaster1.xml  what every slide looks like; text styles
   ├─ notesSlides/notesSlide1.xml    one per slide that has notes
   ├─ charts/chart1.xml              a chart, with the values it last showed
   ├─ embeddings/…xlsx               the workbook behind a chart
   ├─ diagrams/data1.xml, drawing1.xml   SmartArt: its content, and how it was drawn
   ├─ comments/, commentAuthors.xml
   ├─ theme/theme1.xml
   └─ media/image1.png, media1.mp4   pictures, audio and video as inserted
```

Four facts shape the design:

- **Slides are pages, exactly.** Their order is in `presentation.xml`, through relationships, not in the part names. Nothing has to be estimated to know which slide something is on.
- **Everything has a box.** A shape states its position and size in EMU (12,700 to the point), and the slide states its size: 720 × 540 points in the pandoc sample, 960 × 540 in every deck PowerPoint wrote.
- **A slide has three layers.** A placeholder on a slide may state nothing but its kind, and take its box, its font size and its bullets from the layout, which may take them from the master. In the seven PowerPoint templates, 162 of 496 placeholders state no box of their own.
- **Text is DrawingML, not WordprocessingML.** Paragraphs are `a:p`, runs `a:r`, text `a:t`. None of `docxdown`'s body reader applies; its package layer, properties and image headers (all in `ooxmldown`) do.

## 3. The `pptx` crate: what I found

| | Finding |
| --- | --- |
| Record | MIT, version 0.1.0, by Hidemi Ito; modelled on python-pptx |
| Code | 39.7k lines, `#![forbid(unsafe_code)]`, no build script |
| Dependencies | `quick-xml` 0.39, `sha1` 0.10, `thiserror` 2, `zip` 7. The workspace is on `zip` 9.5.1 and reads XML with `roxmltree`, so an addon would carry two zip versions and a second XML parser |
| Scope | Reads and writes. Most of the crate builds and edits decks, which pptxdown does not do |
| Opening | 10 of 14 decks. Three written by another library were refused because their relationship ids are not of the form `rId7`, which the format does not require. One of yours was refused because its zip has a directory entry |
| Shape text | Reached, in text boxes and in groups |
| Table text | Not reachable from a slide it has parsed: a table's frame only says that it is one. On the pandoc sample it returned 165 of 218 characters, on the `ppt-rs` one 146 of 199, and across the seven templates 61,634 of 65,232 |
| Charts, SmartArt | Flagged on the frame; SmartArt has a reader of its own |
| Limits | Has them: 500 MB in all, 10,000 entries, 100 MB an entry |

**Why I recommend against it.** A deck that is refused outright is the whole document unread, and the two reasons it gave are not faults in the files. Table text is where a deck keeps its numbers, and there is no way to it through the parsed slide. Our own reader keeps to what the workspace already uses and decides on purpose what it skips (section 7.1).

It is MIT, so it stays useful as a reference for element names, and as a cross-check in a scratch harness.

## 4. Target layout

```text
crates/
├─ ooxmldown/           + package kinds for presentations: macro-enabled, template, slide show
│                       + app.xml: slides, notes, hidden slides, presentation format
│                       + chart.rs: a chart part read into data
├─ contentdown/         unchanged: page model, line builder, invoice, paper sizes
├─ pptxdown/            new. The reader and the model; no napi
│  └─ src/
│     ├─ lib.rs         Presentation::open(bytes)
│     ├─ error.rs       PresentationError
│     ├─ model.rs       Slide, Shape, Paragraph, Run, Table, Picture, Chart, Note, Comment
│     ├─ deck.rs        presentation.xml: slide order, slide size, sections
│     ├─ slide.rs       a slide's shape tree, groups and their transforms
│     ├─ inherit.rs     layouts and masters: a placeholder's box, sizes and bullets
│     ├─ body.rs        a DrawingML text body: paragraphs, runs, fields, bullets
│     ├─ table.rs       a:tbl
│     ├─ graphic.rs     pictures, media, charts, SmartArt
│     ├─ notes.rs       speaker notes and comments
│     ├─ meta.rs        metadata
│     ├─ text.rs        plain text
│     ├─ markdown.rs    Markdown
│     └─ layout.rs      slides as contentdown pages; invoice
└─ pptxdown-node/       new. @d0paminedriven/pptxdown
```

```toml
# crates/pptxdown/Cargo.toml: the same four dependencies docxdown has
[dependencies]
contentdown.workspace = true
ooxmldown.workspace   = true
roxmltree.workspace   = true
thiserror.workspace   = true

# crates/pptxdown-node/Cargo.toml
[package]
edition.workspace      = true
license.workspace      = true
keywords             = ["node-bindings", "pptx", "extract", "extraction", "slides"]
license              = { workspace = true }
name                 = "pptxdown-node"
version              = "0.1.0"
publish                = false
repository.workspace   = true
rust-version.workspace = true
[lib]
crate-type = ["cdylib"]

[dependencies]
contentdown.workspace = true
napi.workspace        = true
napi-derive.workspace = true
pptxdown.workspace    = true

[build-dependencies]
napi-build.workspace = true

# root Cargo.toml: members, and
pptxdown      = { path = "crates/pptxdown" }
pptxdown-node = { path = "crates/pptxdown-node" }
```

No third-party crate is added.

**As built.** No third-party crate was added. The files differ from the sketch:

| Planned | Built |
| --- | --- |
| `deck.rs`, `notes.rs` | In `lib.rs`: slide order, size and sections, notes and comments are read where the deck is assembled |
| `table.rs`, `graphic.rs` | In `slide.rs`, with the rest of the shape tree |
| | `source.rs`: the parts of a package behind a trait, so tests read a deck from XML held in memory |
| | `xml.rs`: helpers that go by local names, since PresentationML, DrawingML and the diagram drawing use three prefixes for the same elements |
| | `lists.rs`: `annotations()`, `placements()`, `tables()`, `charts()`, `clips()`, `notes()` |
| | `testing.rs`: a deck builder for the tests |
| `contentdown` unchanged | `flow`, `markdown` and `counting` added (see row 13 above) |

## 5. The model

```rust
// pptxdown: plain data, owned
pub struct Presentation {
  pub props: DocProps,            // from ooxmldown
  pub width: f64,                 // the slide, in points
  pub height: f64,
  pub slides: Vec<Slide>,
  pub images: Vec<Image>,         // each picture once
  pub media: Vec<Media>,          // embedded audio and video
  pub comments: Vec<Comment>,
}

pub struct Slide {
  pub number: u32,                // 1-based, hidden slides counted
  pub title: Option<String>,
  pub layout: Option<String>,     // the layout's name: "Title and Content"
  pub section: Option<String>,
  pub hidden: bool,
  pub shapes: Vec<Shape>,         // in reading order
  pub notes: Vec<Paragraph>,
}

pub struct Frame {                // where a shape is, in points, from the slide's top-left corner
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
  pub rotation: f64,
}

pub enum Shape {
  Text(TextShape),                // a text box or a placeholder with text
  Picture(Picture),
  Table(Table),
  Chart(Chart),                   // data only
  Diagram(Diagram),               // SmartArt
  Media(MediaRef),                // audio or video, with its poster picture
  Group(Group),                   // shapes of its own
}

pub struct TextShape {
  pub frame: Frame,
  pub stacking: u32,              // back to front, as the file stores it
  pub placeholder: Option<PlaceholderKind>,   // Title, Body, Footer, SlideNumber, ...
  pub paragraphs: Vec<Paragraph>,
}

pub struct Paragraph {
  pub level: u8,
  pub bullet: Option<String>,     // "•", "1.", or none
  pub align: Option<Align>,
  pub runs: Vec<Run>,             // text with bold, italic, underline, size, font, link
}
```

What JS sees:

```ts
export type Shape =
  | { type: 'text'; text: string; frame: Frame; placeholder?: PlaceholderKind; paragraphs: Array<Paragraph> }
  | { type: 'picture'; image: number; name: string; frame: Frame; altText?: string }
  | { type: 'table'; frame: Frame; columns: Array<number>; rows: Array<TableRow> }
  | { type: 'chart'; frame: Frame; kind: ChartKind; title?: string; series: Array<ChartSeries> }
  | { type: 'diagram'; frame: Frame; text: string; shapes: Array<Shape> }
  | { type: 'media'; frame: Frame; media: number; mime?: string; poster?: number }
  | { type: 'group'; frame: Frame; shapes: Array<Shape> }

type Shapes = UTR<Shape, 'type'>   // { text: ..., picture: ..., table: ... }
```

Every text shape carries its plain `text` beside its `paragraphs`, as a docx paragraph does.

**As built.**

- `frame` is optional: a placeholder that neither its slide, its layout nor its master gives a box has none.
- Every shape carries the same seven fields beside its own: `frame`, `stacking`, `shapeName`, `altText`, `placeholder`, and `uri` or `dest` for a link on the whole shape.
- A chart's `kind` is on each series, not on the chart: a chart that draws bars with a line over them has series of both kinds.
- A picture has `name` (its part) and, when the file keeps one, `vector` (the SVG's part). A clip's `poster` is the poster picture's name.
- A diagram has `nodes` (the text of its nodes) for a deck that keeps no drawing.
- A slide is `{ page, name, title, layout, section, hidden, text, shapes, notes }`. `page` is the slide's number, the word every other reader uses.

## 6. Verbs

Each has an `Async` twin and a free-function form (`pptxText(buffer)`), as in the other packages.

| Method on `PptxDown` | Returns | Nearest relative |
| --- | --- | --- |
| `metadata()` | properties, slide size, counts (section 8) | every package |
| `text()` | the deck as plain text, slide by slide | docxdown `text()` |
| `textPerPage()` | `{ page, text }` for each slide | pdfdown, docxdown `textPerPage()` |
| `structuredText()` | `{ page, header, body, footer }` for each slide (added; see section 1, row 6) | pdfdown `structuredText()` |
| `slides()` | the tree in section 5 | docxdown `blocks()` |
| `markdown()` | a heading per slide, bullets, tables, links, notes | docxdown `markdown()` |
| `tables()` | each table keyed by row and column | docxdown `tables()`, xlsxdown `spreadsheetGrid()` |
| `images()` | every picture where it is placed, with its box | pdfdown `imagesPerPage()`, docxdown `images()` |
| `annotations()` | hyperlinks and comments (section 7.11) | pdfdown `annotationsPerPage()`, docxdown `annotations()` |
| `notes()` | speaker notes, slide by slide | docxdown `notes()` |
| `charts()` | each chart as data | new |
| `media()` | embedded audio and video, with their bytes | new |
| `layout()` | slides as pages of positioned words | every package |
| `invoice()` | invoice fields | every package |
| `document()` | all of the above but `layout` and `invoice`, in one object | pdfdown, docxdown `document()` |

```ts
const deck = new PptxDown(buffer);          // or await PptxDown.openAsync(buffer)
deck.metadata().slideCount;                 // 14
deck.slides()[2]?.title;                    // "Revenue by region"
deck.markdown();                            // "## Revenue by region\n\n- North grew **12%** ..."
deck.tables()[0]?.rows["2"]?.["D"]?.text;   // the same grid shape as a docx table
deck.charts()[0]?.series[0]?.values;        // [12, 15, 9, 21]
```

## 7. How the hard parts work

### 7.1 What counts as text

| Taken | Skipped |
| --- | --- |
| `a:t` in every shape of a slide: text boxes, placeholders, table cells, shapes in groups | Prompt text a layout or master holds for an empty placeholder |
| SmartArt text, from its own parts | The second copy of a shape in `mc:Fallback` |
| Chart titles, series names and categories, as chart data | Anything on a layout or master that is not a placeholder the slide fills (decision 6; Parked) |
| The text of fields as last saved (a slide number, a date) | |
| Speaker notes, in `notes()` and on the slide, not in `text()` | |

`a:br` is a line break, `a:tab` a tab.

This is the test of the whole reader, as it is for docxdown: on every fixture, the characters the slides return must equal a raw count of the `a:t` in the slide parts, less the skipped column.

### 7.2 Three layers

A slide names its layout, and a layout its master. A placeholder on the slide is matched to one on the layout by its kind and index, and that one to the master's. For each thing the slide's shape leaves unsaid, the answer is the nearest one up that chain:

```text
box, font size, bullet, alignment:
  the shape → the layout's placeholder → the master's placeholder
            → the master's text styles (title, body, other) → the deck's defaults
```

This is what makes a body placeholder bulleted and a title large when the slide says neither. It is needed for three things: a box for a shape that states none, a font size for `layout()`, and whether a paragraph is a list item.

### 7.3 Reading order

The file stores a slide's shapes back to front, in the order they were stacked. A caption added last comes last even when it sits at the top.

The reader orders them the way a slide is read: the title placeholder first, then the rest in bands from top to bottom, left to right within a band. A group is placed by its own box and ordered inside in the same way. Each shape keeps `stacking`, so a caller can have the stored order back.

### 7.4 Text bodies

A paragraph has a level (0 to 8) and a bullet: a character, an automatic number, or none, stated on the paragraph or inherited (7.2). Automatic numbers are counted per shape and level and written out, as docxdown writes list markers. Runs carry bold, italic, underline, strike, size, font and a link. A body may say its text was shrunk to fit; the scale is applied to the sizes `layout()` uses.

**As built.** Numbers are counted the way PowerPoint shows them. Paragraphs that follow each other on a level with one scheme and one stated start (`startAt`) are one list, counted from that start. A paragraph that states another start begins a new list there, and text with no bullet ends the list. A numbered paragraph with nothing in it shows no number and is given none. `Activity-7.pptx` is the evidence: its second slide types its first step by hand and numbers five more paragraphs, two of them empty, which say they start at 2, 2, 3, 3 and 3; the steps read 2, 3, 4. A bullet set in a symbol font, or a character from the private-use range, is written as `•`.

### 7.5 Tables

A table sits in a graphic frame. Column widths and row heights are stated. A merged cell covers its neighbours with `gridSpan` and `rowSpan`, and the cells it covers are present and marked, so nothing has to be inferred. `tables()` returns the same grid `docxdown` does.

### 7.6 Pictures

A picture points at a relationship, and the relationship at a file under `ppt/media`. The bytes come back as stored, with the MIME type and pixel size from `ooxmldown`, the box it is shown in, and its alt text. An SVG is stored beside a PNG fallback; both are listed and the pair is linked. A slide's background picture is a picture with `background: true`.

### 7.7 Charts

A chart frame points at a chart part. The part holds the chart's kind (bar, line, pie, and so on), its title, and for each series a name, categories and values, with the values the application last showed cached inside it. The reader returns those. The workbook embedded behind the chart is not opened.

In the templates, each of the 16 charts has its cached values and an embedded workbook; the kinds are bar and line.

### 7.8 SmartArt

A SmartArt frame points at two parts. The data part holds the nodes and their text. The drawing part holds what PowerPoint drew from them: ordinary shapes with boxes. The reader uses the drawing part, through the same shape reader as a slide, and falls back to the data part's text in node order when a deck has no drawing part.

### 7.9 Groups and transforms

A group has a box on the slide and a coordinate space of its own for its children, so a child's position is scaled and moved by the group's, and by every group above it. Rotation is reported on the frame and not applied to the box. Nesting has a limit.

### 7.10 Notes and comments

- **Notes.** One part per slide, with a placeholder for the slide's picture, one for the notes, and one for the slide number. Only the notes body is taken. In the templates the notes parts hold nothing but the slide number.
- **Comments.** Two forms exist: the older one (a list per slide, authors in `commentAuthors.xml`) and the threaded one of recent PowerPoint. Both become comments with an author, a date, text, and the slide they are on.

### 7.11 Parity with pdfdown and docxdown: annotations and images

A link is on a run of text or on a whole shape. Its target is an address, or another slide in the deck.

| pdfdown `PageAnnotation` | pptxdown | Notes |
| --- | --- | --- |
| `page` | `page` | the slide number, exact |
| `subtype` | `subtype`: `Link` or `Comment` | |
| `rect` | `rect` | the box of the shape the link is in; a docx has none |
| `uri` | `uri` | an external link's target |
| `dest` | `dest` | a link to another slide: its number |
| `content` | `content` | a comment's text |
| | `text`, `author`, `date` | as docxdown |

**As built.** `rect` is `[x0, y0, x1, y1]` in the page frame `layout()` uses (points, y up), the form pdfdown gives, so a link's box can be laid over the words. A link written as several runs to one target is one annotation.

| pdfdown `PageImage` | pptxdown | Notes |
| --- | --- | --- |
| `page`, `imageIndex` | `page`, `imageIndex` | one entry per place a picture is used |
| `width`, `height` | `width`, `height` | pixels, from the image's own header |
| `data` | `data` | the bytes as stored |
| `filter` | `mime` | |
| `xobjectName`, `objectId` | `name` | the part: `ppt/media/image1.png` |
| | `frame`, `altText` | where it is on the slide, and its description |

### 7.12 Layout, for `layout()` and `invoice()`

Each slide is one `contentdown` page of the slide's size. Unlike a docx, the boxes are real:

```text
for each shape, in reading order:
  its box comes from the file (7.2, 7.9), less the body's insets
  its paragraphs are set top to bottom inside the box, anchored as the body says
  a line wraps at the box's width, by the estimate docxdown and xlsxdown use
table: each cell's words inside the cell's own rectangle
```

| Real | Estimated |
| --- | --- |
| Which slide a word is on | Where a line wraps inside its box |
| The box every shape, cell and picture is in | A font's real widths |
| Font size, bold, italic (through 7.2) | |

`PageLayout.images` is filled with the picture boxes, which docxdown cannot do.

**As built.** Text the estimate makes taller than the room between where it starts and the slide's bottom edge is set closer together, so every word stays on its page. A box that lies partly off the slide is cut to the part that is on it. A shape with no box anywhere is set across the slide, less a margin, and the page's `warnings` say how many were.

### 7.13 Hidden slides

A hidden slide keeps its place and its number, so slide 7 is slide 7 whether or not slide 3 is hidden. It is returned with `hidden: true`, and `metadata()` counts them. The `hiddenSlides: false` option leaves them out of every reader.

### 7.14 Slide size and paper

`presentation.xml` states the slide's size and, sometimes, what it is meant for (`screen16x9`, `letter`, `A4`). Metadata reports the size in points, the aspect ratio, that label, and `paperSize` through the same `contentdown::paper_size` as the other packages. A 960 × 540 slide is no paper size, so most decks report `UNKNOWN` with the two sizes it lies between.

## 8. What the `Document` model gets

| `Document` column | From pptxdown |
| --- | --- |
| `format` | `"pptx"` |
| `pageCount` | `metadata().slideCount` |
| `wordCount` | `metadata().wordCount`: counted from the slides; the recorded figure is beside it |
| `language` | the language the text runs state most often |
| `title`, `author`, `subject`, `keywords` | `docProps/core.xml` |
| `textPreview` | the start of `text()` |
| `pageWidth`, `pageHeight` | the slide size, in points |
| `paperSize` | `metadata().paperSize.type`: usually `UNKNOWN` |
| `pageSizeUniform` | always true: a deck has one slide size |
| `imageCount` | `metadata().imageCount` |
| `annotCount` | `metadata().annotationCount`: links and comments |
| `isEncrypted` | never true for a deck that opened; an encrypted file is refused (section 9) |

## 9. Errors and limits

```rust
pub enum PresentationError {
  /// A readable file that is not a presentation: says what it is instead.
  UnsupportedFormat(String),
  LimitExceeded(String),
  Malformed(String),
}
```

- **A workbook or a Word document** is named for what it is, as the other two name a presentation.
- **A legacy `.ppt` and an encrypted pptx** are both OLE compound files, not zips. They are recognized by their first bytes and refused with a message that says which is likely.
- **Tolerance.** A relationship id is whatever the file says it is, and a zip may hold directory entries: the two things that stopped the `pptx` crate.
- **Limits.** `ooxmldown`'s caps on the package, plus caps on slides, on shapes per slide, on total text, and on the nesting of groups.

**As built.** 10,000 slides, 50,000 shapes on a slide, groups 32 deep, and 100,000 points in a chart series. There is no separate cap on text: all of it comes from parts, which `ooxmldown` already caps. A slide, a layout, a chart or a picture that cannot be read is left out and named in `warnings`; the rest of the deck is returned.

## 10. Order of work

Each unit ends with its own tests passing.

| # | Unit | Verified by |
| --- | --- | --- |
| 1 | `ooxmldown`: package kinds for presentations, the app.xml fields | Tests on the fixtures of all three formats |
| 2 | `pptxdown`: slide order and size, shape text, `text()` | The character count of section 7.1 on every fixture |
| 3 | Layouts and masters: boxes, sizes, bullets; `slides()` | Fixtures with placeholders that state no box |
| 4 | Reading order, groups and transforms | Slides whose stored order differs from their reading order |
| 5 | Tables; `tables()` | A table with merged cells |
| 6 | Pictures and media; `images()`, `media()` | One fixture each: PNG, SVG with fallback, video |
| 7 | Notes, comments, links; `notes()`, `annotations()` | One fixture each |
| 8 | Charts in `ooxmldown`; `charts()` | The template decks' bar and line charts, against their workbooks |
| 9 | SmartArt | A fixture from PowerPoint |
| 10 | `metadata()`, `textPerPage()`, `markdown()` | The table in section 8 |
| 11 | `layout()`, `invoice()` | Boxes against the file; a one-slide invoice |
| 12 | `pptxdown-node`: binding, tests on your `WorkupService`, wasm | Native and wasm suites |
| 13 | `ci/` folder and workflow; release `pptxdown@0.1.0` | The first run |

**As built.** Units 1 to 12 are done, and unit 13's `ci/` folder and workflow are written. The release is yours.

| Suite | Tests |
| --- | --- |
| `pptxdown` (Rust) | 50 |
| `pptxdown-node` (Rust, what JavaScript is given for decks put together as data) | 7 |
| `pptxdown-node` on the native addon | 51 |
| `pptxdown-node` on the WebAssembly build | 8 |
| `pptxdown-node`, the spec kept out of the repository with the private decks | 9 |

## 11. Fixtures

You have some. None of the 14 decks sampled here has SmartArt, a comment, a group, a video or a hidden slide, so this is what would cover the plan:

- Written by PowerPoint, by Keynote or Google Slides (exported), and by LibreOffice.
- Slides with a title and bullets on several levels; a numbered list.
- A table with merged cells; a chart; SmartArt.
- Pictures, one of them an SVG; a group; a video or audio clip.
- Speaker notes; comments, old and threaded; a link to a site and a link to another slide.
- A hidden slide; sections; a 4:3 deck beside a 16:9 one.
- For the refusals: a `.ppt`, an encrypted `.pptx`, a `.pptm`, a `.ppsx`.

Public ones go in `crates/pptxdown-node/__test__/fixtures`, private ones under `fixtures/private`, as with the other packages.

**As built.** The tests in the repository read the two public decks, both written by PowerPoint. They cover titles (and a slide without one), a title that takes its box from its layout, numbered steps that state where they start, pictures in three formats, a 4:3 deck beside a 16:9 one, and notes pages with no notes on them. Neither holds a table, a link, a comment, a chart, SmartArt, a group or a clip. Those are tested on decks built in memory from XML in `pptxdown`, and what JavaScript is given for them on decks put together as data in the binding's own Rust tests: merged cells, groups, SmartArt, charts, clips, an SVG beside its raster copy, comments in both forms, links to an address and to another slide, sections, hidden slides, and placeholders with no box.

Three more of your decks are kept out of the repository under `fixtures/private`. A spec in that folder, which git ignores with them, reads them when the tests run on this machine. Nothing about them is in this file, the README or the tracked tests. No fixture was added and no script is kept in the repo.

Small synthetic ones can be made here from Markdown, with no script kept in the repo, by three writers that produce different files:

| Writer | What it gave for the same page of Markdown |
| --- | --- |
| pandoc | 5 slides, placeholders that mostly take their box from the layout, a table, a link, notes, bullets on three levels |
| LibreOffice, saving pandoc's deck again | The same content with every box stated, and shapes wrapped in `mc:AlternateContent` |
| `ppt-rs` (`pptcli md2ppt`) | 4 slides, every box stated, a table, notes from a `>` quote. The link came out as plain text and the nested bullet on the top level |

`ppt-rs` is a tool for the scratch folder only, never a dependency: it brings `axum`, `reqwest`, `tokio`, `syntect` and a `zip` of its own. Its Markdown starts a slide at each `#` heading, and it can also write pictures, code blocks and Mermaid diagrams, which would give picture fixtures.

## Parked

- Content a layout or master adds to every slide and that no placeholder holds: a logo, a strap line.
- Rendering a slide to an image.
- Animations and transitions.
- The workbook behind a chart, and other embedded files: a job for the caller to route.
- Applying rotation and flips to boxes.
- Writing or editing decks.

## Not verified

- The threaded comment format: read from the specification, not from a file.
- SmartArt's drawing part against its data part: none of the 14 decks has SmartArt.
- That every placeholder without a box finds one on its layout or master. (As built: every one does in the decks tested; one that finds none is reported without a box and set across the slide in `layout()`.)
- As built, the same holds for everything section 11 lists as tested only on decks built in memory: a chart in a deck, a clip, a hidden slide, a group and a comment have not been read from a file PowerPoint wrote.
- Decks written by `ppt-rs` beyond the one sample: its pictures and diagrams have not been looked at.
- Keynote and Google Slides exports: none in the sample.
- How well reading by position matches what a person would call the order of a busy slide.
- `invoice()` on a deck: no invoice written as slides has been tried.
