# docxdown: plan

Status: **built.** The library (`crates/docxdown`), the additions to `ooxmldown`, and the binding's source, tests and CI (`crates/docxdown-node`) are written, with every decision in section 1 taken as recommended. Two things remain yours: the binding's `Cargo.toml` still lists pdfdown's dependencies and needs the ones in section 4, and it has no `.cargo/config.toml` yet. Where the build differs from this plan, the section says so under **As built**.

The research was done in a scratch folder: a trial of `docx-rust` on 14 documents, and raw counts of what those documents hold.

Names of functions and types in the snippets are proposals. The numbers in section 3 come from two documents I generated (pandoc and LibreOffice), the eleven in your metadata package's test folder, and one fixture already in this repo. No figure from the private corpus is in this file.

## 1. Decisions

All open. Each row has what I recommend and will use unless you say otherwise.

| # | Question | Recommended | Why |
| --- | --- | --- | --- |
| 1 | What reads the document? | Our own reader on `ooxmldown` + `roxmltree`, the way `xlsxdown` is built. `docx-rust` is not a dependency | It parsed every file I tried, but it silently drops text (section 3) |
| 2 | Shape of the content | A tree of blocks, as unions keyed on `type` | Works with `UTR`; a paragraph and a table carry different fields |
| 3 | Verbs in the first release | Everything in section 6 | They fall out of one model |
| 4 | Markdown output | In the first release | Small once blocks exist, and it is the form a model reads best |
| 5 | Pages | Split where Word last broke the page, plus explicit breaks; the recorded page count reported beside it | A docx has no pages of its own (section 7.7) |
| 6 | `invoice()` | A flow layout handed to `contentdown`, as `xlsxdown` does with a sheet | Reuses the extractor unchanged (section 7.8) |
| 7 | Tracked changes | Final view by default: insertions kept, deletions dropped. An option gives the original | It is what the reader of the document sees |
| 8 | Where `docProps` reading lives | `ooxmldown` | xlsx and pptx have the same two parts |
| 9 | Async | `AsyncTask`, as `xlsxdown-node` and `pdfdown-node` | Same family, same pattern |
| 10 | `.docm` and `.dotx` | Read their content; macros are never run and `hasMacros` says they exist | The text is ordinary WordprocessingML |
| 11 | Names and version | crates `docxdown`, `docxdown-node`; npm `@d0paminedriven/docxdown`; start at `0.1.0` | As the others |

## 2. What a docx is

```text
report.docx  (a zip)
├─ [Content_Types].xml
├─ _rels/.rels                   → the main part, and docProps
├─ docProps/core.xml             title, author, subject, keywords, dates
├─ docProps/app.xml              application, pages, words (as last saved)
└─ word/
   ├─ document.xml               the body: paragraphs, tables, section properties
   ├─ styles.xml                 styles, inheritance, defaults
   ├─ numbering.xml              list definitions
   ├─ header1.xml, footer1.xml   one part per header and footer
   ├─ footnotes.xml, endnotes.xml, comments.xml
   ├─ settings.xml
   ├─ _rels/document.xml.rels    → images, hyperlinks, headers, footers
   └─ media/image1.png           images in the format they were inserted in
```

Two facts shape the design:

- **A docx flows.** It records paragraphs and tables in order, not where anything lands on a page. Positions and pages exist only after a layout engine runs.
- **The main part is found through relationships**, not by its usual name. `ooxmldown` already does that, within size limits.

## 3. `docx-rust`: what I found

| | Finding |
| --- | --- |
| Record | MIT, first released 2022, 12 releases, 2.4M downloads, last release January 2026 |
| Code | 13.8k lines, no `unsafe`, no build script |
| Dependencies | `hard-xml` 1.27, `derive_more` 0.99, `thiserror` 1, `log`, `zip` 4. The workspace is on `zip` 9.5.1, so an addon would carry two zip versions |
| Parsing | 14 of 14 files parsed without an error or a panic |
| Its `text()` | Paragraphs only. On a sample with a table it returned 102 of 155 characters |
| Its model | Reaches the table (155 of 155). Does not reach text boxes: 57 of 1,063 and 56 of 9,710 characters unreachable in two of your test documents |
| Reader | Opens parts by fixed names and reads each to the end with no size limit |
| Shape | `Docx<'a>` borrows from `DocxFile`, so holding one in a napi class needs a self-referential type. `xlsxdown`'s `Workbook` owns its data |
| Scope | About half the crate writes documents, which docxdown does not do |

From its source, these have no type in the model, so their content is dropped without an error:

| Element | What it holds |
| --- | --- |
| `w:tbl` inside `w:tc` | Nested tables (the variant is commented out) |
| `w:sdt` inside `w:tc` | Content controls in table cells, common in forms |
| `mc:AlternateContent`, `w:txbxContent`, `w:pict` | Text boxes and legacy drawings |
| `w:ins` | Tracked insertions |
| `w:fldSimple` | Simple fields and their results |
| `w:smartTag`, `w:customXml` | Runs wrapped by older versions of Word |

**Why I recommend against it.** For an invoice pipeline, text that goes missing without an error is the worst kind of failure: the output looks complete. A reader that walks the XML itself can take every `w:t` it finds and decide what to skip on purpose (section 7.1). It also keeps to what the workspace already uses: `ooxmldown` for the package and its limits, `roxmltree` for XML, no new dependency.

`docx-rust` is MIT, so it stays useful as a reference for element names, and as a cross-check in a scratch harness.

## 4. Target layout

```text
crates/
├─ ooxmldown/           + docprops.rs      core.xml and app.xml, for every Office format
│                       + PackageKind for macro-enabled and template documents
├─ contentdown/         unchanged: page model, line builder, invoice, paper sizes
├─ docxdown/            new. The reader and the model; no napi
│  └─ src/
│     ├─ lib.rs         Document::open(bytes)
│     ├─ error.rs       DocumentError
│     ├─ parts.rs       which parts the document has, through relationships
│     ├─ model.rs       Block, Paragraph, Inline, Table, Section, Image, Note, Comment, Field
│     ├─ body.rs        document.xml → blocks
│     ├─ styles.rs      inheritance, headings, run defaults
│     ├─ numbering.rs   list kind, level and marker
│     ├─ notes.rs       footnotes, endnotes, comments, headers, footers
│     ├─ media.rs       images through relationships
│     ├─ meta.rs        metadata and sections
│     ├─ text.rs        plain text
│     ├─ markdown.rs    Markdown
│     ├─ pages.rs       where pages split
│     └─ layout.rs      flow layout → contentdown pages; invoice
└─ docxdown-node/       new. @d0paminedriven/docxdown
```

```toml
# crates/docxdown/Cargo.toml: the same four dependencies xlsxdown has
[dependencies]
contentdown.workspace = true
ooxmldown.workspace   = true
roxmltree.workspace   = true
thiserror.workspace   = true

# crates/docxdown-node/Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
contentdown.workspace = true
docxdown.workspace    = true
napi.workspace        = true
napi-derive.workspace = true

[build-dependencies]
napi-build.workspace = true

# root Cargo.toml: members, and
docxdown      = { path = "crates/docxdown" }
docxdown-node = { path = "crates/docxdown-node" }
```

No third-party crate is added.

## 5. The model

```rust
// docxdown: plain data, owned
pub struct Document {
  pub props: DocProps,            // from ooxmldown
  pub sections: Vec<Section>,
  pub blocks: Vec<Block>,
  pub headers: Vec<Part>,         // each a list of blocks
  pub footers: Vec<Part>,
  pub notes: Vec<Note>,           // footnotes and endnotes
  pub comments: Vec<Comment>,
  pub images: Vec<Image>,
}

pub enum Block {
  Paragraph(Paragraph),
  Table(Table),
}

pub struct Paragraph {
  pub style: Option<String>,      // the style's name: "Heading 1", "List Paragraph"
  pub heading: Option<u8>,        // 1 to 9, from the style's outline level
  pub list: Option<ListItem>,
  pub inlines: Vec<Inline>,
  pub section: u32,               // index into Document::sections
  pub page: u32,                  // 1-based; section 7.7
}

pub struct ListItem {
  pub level: u8,
  pub ordered: bool,
  pub marker: String,             // "1.", "a)", "•"
}

pub enum Inline {
  Text(TextRun),                  // text with bold, italic, underline, strike, size, font
  Link { href: String, inlines: Vec<Inline> },
  Image { image: u32 },           // index into Document::images
  NoteRef { note: u32 },
  CommentRef { comment: u32 },
  Field(Field),                   // content control or form field, with its value
  Break(BreakKind),               // line, page, column
  Tab,
}

pub struct Table {
  pub columns: Vec<f64>,          // widths in points, from the table grid
  pub rows: Vec<Row>,
}

pub struct Cell {
  pub blocks: Vec<Block>,         // a cell holds paragraphs and tables
  pub col_span: u32,
  pub row_span: u32,              // from vertical merges
}
```

What JS sees:

```ts
export type Block =
  | { type: 'paragraph'; text: string; style?: string; heading?: number; list?: ListItem; inlines: Array<Inline>; page: number }
  | { type: 'table'; columns: Array<number>; rows: Array<TableRow>; page: number }

export type Inline =
  | { type: 'text'; text: string; bold?: boolean; italic?: boolean; underline?: boolean; strike?: boolean }
  | { type: 'link'; href: string; text: string; inlines: Array<Inline> }
  | { type: 'image'; image: number }
  | { type: 'noteRef'; note: number }
  | { type: 'commentRef'; comment: number }
  | { type: 'field'; name?: string; kind: FieldKind; value: string }
  | { type: 'break'; kind: 'line' | 'page' | 'column' }
  | { type: 'tab' }

type Blocks = UTR<Block, 'type'>   // { paragraph: ..., table: ... }
```

Every paragraph carries its plain `text` beside its `inlines`, so a caller who only wants text never walks the runs.

## 6. Verbs

Each has an `Async` twin and a free-function form, as in the other packages.

| Method on `DocxDown` | Returns | Nearest relative |
| --- | --- | --- |
| `metadata()` | properties, sections, counts (section 8) | pdfdown `metadata()` |
| `text()` | the whole document as plain text | |
| `textPerPage()` | text split at page breaks | pdfdown `textPerPage()` |
| `blocks()` | the tree in section 5 | |
| `markdown()` | headings, lists, tables, emphasis, links, footnotes | |
| `tables()` | each table as rows of cell text, keyed like the workbook grid | xlsxdown `spreadsheetGrid()` |
| `images()` | every picture where it is placed: original bytes, MIME type, pixel and display size, alt text (section 7.10) | pdfdown `imagesPerPage()` |
| `annotations()` | hyperlinks and comments in one flat list (section 7.10) | pdfdown `annotationsPerPage()` |
| `formFields()` | content controls and legacy form fields with their values | pdfdown `formFieldsPerPage()` |
| `notes()` | footnotes and endnotes, each with its blocks | |
| `layout()` | pages of positioned words (section 7.8) | pdfdown and xlsxdown `layout()` |
| `invoice()` | invoice fields | every package |
| `document()` | all of the above in one object | pdfdown `document()` |

```ts
const doc = new DocxDown(buffer);          // or await DocxDown.openAsync(buffer)
doc.metadata().sections[0]?.paperSize;     // { type: 'LETTER', width: 612, height: 792 }
doc.markdown();                            // "# Invoice 4711\n\nBill to **Acme Corp** ..."
doc.tables()[0]?.rows["2"]?.["B"]?.text;   // the workbook-grid shape, for line items
doc.invoice().fields.invoiceId?.value;
```

## 7. How the hard parts work

### 7.1 What counts as text

The reader takes text wherever it is and skips only what a reader of the document would not see.

| Taken | Skipped |
| --- | --- |
| `w:t` in any run: body, tables, nested tables, content controls, hyperlinks, smart tags | `w:delText`: deleted text (in the final view) |
| Text boxes, once: the `mc:Choice` branch | `mc:Fallback`: the second copy of the same text box |
| Field results | `w:instrText`: field codes such as `PAGE` or `MERGEFIELD` |
| Tracked insertions (`w:ins`) | |

`w:tab` becomes a tab, `w:br` and `w:cr` a line break, `w:noBreakHyphen` a non-breaking hyphen, and `w:softHyphen` nothing.

This is the test of the whole reader: on every fixture, the characters `text()` returns must equal a raw count of the `w:t` in the file, less the skipped column.

### 7.2 Styles and headings

A paragraph names a style; a style names the one it is based on. Formatting is the nearest answer along that chain, then the document defaults:

```text
run's own properties → character style → paragraph style → its basedOn chain → docDefaults
```

A paragraph is a heading when its style, or one it is based on, has an outline level, or is named `heading N`. Bold and italic are toggles between styles: set by the paragraph style and again by a character style, they cancel. A run's own setting always wins.

### 7.3 Lists

`numbering.xml` maps a paragraph's list id and level to a format (bullet, decimal, letter, roman) and a pattern such as `%1.%2.`. The reader keeps a counter per list and level, restarting where the definition says, and writes the marker out: `2.`, `b)`, `iv.`, `•`.

### 7.4 Tables

Column widths come from the table grid, in twentieths of a point. A cell's `gridSpan` is its column span. A vertical merge is a `restart` cell followed by `continue` cells, which become one cell with a row span. A cell holds blocks, so a table inside a cell is a table.

### 7.5 Images

A drawing points at a relationship; the relationship points at a file under `word/media`. The bytes come back as stored (PNG, JPEG, GIF, EMF, SVG), with the MIME type, the size the picture is shown at (EMU to points) and its alt text. A linked picture, which lives outside the file, is reported without bytes.

### 7.6 Sections and paper size

Each section states its page: `w:pgSz` in twentieths of a point, an orientation, and margins.

```rust
pub struct Section {
  pub width: f64,                  // points: 12240 twips is 612
  pub height: f64,
  pub landscape: bool,
  pub margins: Margins,
  pub paper_size: PaperSize,       // contentdown::paper_size(width, height)
}
```

`paper_size` is the function pdfdown uses, so a docx reports the same union and the same `DocumentPaperSize` values. Every sample that states a page size states 12240 by 15840 twips: Letter. The pandoc sample states none, which the reader reports as `UNKNOWN`.

### 7.7 Pages

A docx does not say where its pages break. Three things do, partly:

| Source | What it gives | In the samples |
| --- | --- | --- |
| `w:lastRenderedPageBreak` | Where Word broke the page the last time it saved | Word files only: 1 marker in the two-page documents, 4 in the six-page one |
| `w:br w:type="page"`, section breaks | Breaks the author asked for | none |
| `Pages` in `docProps/app.xml` | The count as the application last recorded it | present in all of them |

So `page` on each block counts those markers and breaks, and metadata reports both numbers: the recorded count and the counted one. The six-page sample shows why both are kept: its markers give five. Files written by LibreOffice or pandoc carry no markers, so there everything is page 1 and the recorded count is the only evidence.

**As built.** On the thirteen public fixtures the count equals the recorded one for eleven; the other two lack Word's marker for one page. Two rules came out of those files:

- **A table row carried over a page is marked twice.** Word leaves a marker where the row broke, and another on whatever starts next (the next row, or the paragraph after the table). The reader counts that as one page. A row that starts a page is marked in one cell and moves as a whole.
- **A paragraph is on the page its first content is on.** A paragraph that holds only a picture after a marker is on the new page, and so is its picture.

### 7.8 A flow layout, for `invoice()`

`contentdown`'s invoice reader takes pages of positioned words. `xlsxdown` gives it that by laying a sheet out as a page. docxdown does the same for a document:

```text
frame = the section's page, less its margins
for each block, top to bottom:
  paragraph → words placed left to right, wrapped at the frame's width
  table     → each cell's words placed inside its column of the table grid
a page split (7.7) starts a new PageLayout
```

Word widths are estimated from the font size, with the constants `xlsxdown` already uses, since a docx carries no font metrics.

| Kept | Not kept |
| --- | --- |
| Reading order | True positions |
| Cells of one row on one line, columns aligned | Where Word actually wrapped a line |
| A label next to its value | Fonts' real widths |
| Bold, and the font size | |

Those are the relationships the invoice reader works from. The same pages are what `layout()` returns.

**As built.** Each page also gets the header and footer its section names (the first-page ones on a title page), since a letterhead and a footer with bank details are where a Word invoice keeps its seller. A table whose grid is a placeholder (Word writes one for a table it sizes to its content) is sized by what its cells hold. A page whose content is estimated to run past the paper is given the height it needs, so a layout page is always the page a block names.

### 7.9 Tracked changes, comments, fields

- **Tracked changes.** The final view by default. `metadata()` says whether the document has any, and an option returns the original view (insertions dropped, deletions kept).
- **Comments.** Each with its author, date, text, and the text it is attached to.
- **Fields.** Content controls (`w:sdt`) with their tag, title, kind and value; legacy form fields; merge fields with their result. These are a document's form fields.

### 7.10 Parity with pdfdown: images and annotations

A link made with Ctrl+K is a `w:hyperlink` element around the runs it covers; older files write the same thing as a `HYPERLINK` field. Both become one annotation. Comments are the other kind.

| pdfdown `PageAnnotation` | docxdown | Notes |
| --- | --- | --- |
| `page` | `page` | by the page splits of section 7.7 |
| `subtype` | `subtype`: `Link` or `Comment` | |
| `uri` | `uri` | an external link's target |
| `dest` | `dest` | an internal link: the bookmark it jumps to |
| `content` | `content` | a comment's text |
| `rect` | | a docx has no positions |
| | `text` | the text the link or comment sits on, which a PDF does not record |
| | `author`, `date` | comments |

| pdfdown `PageImage` | docxdown | Notes |
| --- | --- | --- |
| `page`, `imageIndex` | `page`, `imageIndex` | one entry per place a picture is used |
| `width`, `height` | `width`, `height` | pixels, read from the image's own header; absent for vector formats |
| `data` | `data` | the bytes as stored, not re-encoded to PNG |
| `filter` | `mime` | the format: a docx keeps the file that was inserted |
| `xobjectName`, `objectId` | `name` | the part in the package, such as `word/media/image1.jpg` |
| `colorSpace`, `bitsPerComponent` | | PDF concepts |
| | `displayWidth`, `displayHeight`, `altText` | the size it is shown at, in points, and its description |

The same links stay in the block tree as `link` inlines, and pictures as `image` inlines, so the flat lists and the tree agree.

**As built.** Both lists also cover notes, comments, headers and footers, so each entry carries `part` (`body`, `header`, `footer`, `footnote`, `endnote`, `comment`). `page` is absent in a header or a footer, which belong to no one page; a note's entries are on the page that refers to the note.

## 8. What the `Document` model gets

| `Document` column | From docxdown |
| --- | --- |
| `format` | `"docx"` |
| `pageCount` | `metadata().pageCount`: the count the application recorded |
| `wordCount` | `metadata().wordCount`: counted from the text; the recorded figure is beside it |
| `language` | the document's default language, from `styles.xml` |
| `title`, `author`, `subject`, `keywords` | `docProps/core.xml` |
| `textPreview` | the start of `text()` |
| `pageWidth`, `pageHeight` | the first section, in points |
| `paperSize` | `sections[0].paperSize.type` |
| `pageSizeUniform` | whether every section has the same size |
| `imageCount` | `metadata().imageCount` |
| `annotCount` | `metadata().annotationCount`: links and comments |
| `isEncrypted` | never true for a document that opened; an encrypted file is refused (section 9) |

## 9. Errors and limits

```rust
pub enum DocumentError {
  /// A readable file that is not a Word document: says what it is instead.
  UnsupportedFormat(String),
  LimitExceeded(String),
  Malformed(String),
}
```

- **A workbook or a presentation** is named for what it is, as `xlsxdown` names a docx.
- **A legacy `.doc` and an encrypted docx** are both OLE compound files, not zips. They are recognized by their first bytes and refused with a message that says which is likely.
- **Limits.** `ooxmldown`'s caps on the package, plus caps on blocks, on total text, and on nesting depth (tables in cells in tables).

## 10. Order of work

Each unit ends with its own tests passing.

| # | Unit | Verified by |
| --- | --- | --- |
| 1 | `ooxmldown`: `docProps`, and package kinds for Word's variants | Tests on xlsx and docx fixtures |
| 2 | `docxdown`: parts, body text, `text()` | The character count of section 7.1 on every fixture |
| 3 | Styles and numbering: headings and lists; `blocks()` | Fixtures with known headings and list markers |
| 4 | Tables: spans, merges, nesting; `tables()` | Fixtures with merged and nested cells |
| 5 | Sections, paper size, `metadata()` | The table in section 8 |
| 6 | Images, notes, comments, headers, footers, fields | One fixture each |
| 7 | Pages; `textPerPage()` | Word fixtures of known length |
| 8 | `markdown()` | Compared with pandoc's Markdown of the same file, by eye and by headings and table counts |
| 9 | Flow layout; `layout()`, `invoice()` | An invoice written in Word |
| 10 | `docxdown-node`: binding, tests on your `WorkupService`, wasm | Native and wasm suites |
| 11 | `ci/` folder and workflow; release `docxdown@0.1.0` | The first run |

## 11. Fixtures

You said you have plenty. What would cover the plan:

- Written by Word, by LibreOffice and by Google Docs.
- Tables with merged cells; a table inside a cell; lists with several levels; headings.
- Images; a text box; footnotes; comments; tracked changes.
- A form with content controls.
- More than one section, one of them landscape.
- An invoice or two.
- For the refusals: a `.doc`, an encrypted `.docx`, a `.docm`, a `.dotx`.

Public ones go in `crates/docxdown-node/__test__/fixtures`, private ones under `fixtures/private`, as with the other packages. Small synthetic ones can be made here with pandoc and LibreOffice, with no script kept in the repo.

## Parked

- A real layout engine: true pagination, positions, rendering a page to an image or to PDF.
- Equations, charts and SmartArt.
- Embedded workbooks: a job for `xlsxdown` once the caller can route an embedded part.
- Writing or editing documents.
- `pptxdown`, which will want the drawing and image code from 7.5.

## Not verified

- ~~That napi-derive accepts a structured enum that contains itself.~~ It does: `Block` holds rows of cells of `Block`. The object types inside the union must be readable from JavaScript as well as writable.
- ~~The spelling of `type` values.~~ `discriminant_case = "camelCase"` gives `paragraph`, `table`, `noteRef`, `commentRef`.
- Tracked changes, comments, content controls, form fields, endnotes and landscape sections are covered by tests written from XML, not by a file from Word: none of the public fixtures has them.
- `docx-rust` on documents with tracked changes, nested tables or smart tags: the gaps for those come from reading its source, not from a file that has them.
- Google Docs exports: none in the sample.
- The flow layout's estimates against real invoices. `invoice()` runs end to end and reads the header fields of the sample; how well it reads line items and dates from a Word invoice has not been measured.
- Paragraph spacing and line spacing are constants in the layout, not read from the styles.
