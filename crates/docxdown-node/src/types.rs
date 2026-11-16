//! The shapes JavaScript sees. Plain data: each value is built from `docxdown`'s model and copied
//! out. Only the options are ever read back from JavaScript.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use docxdown::{Document, Options, View, column_letters};
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

// ── Options ───────────────────────────────────────────────────────

/// Which side of a document's tracked changes to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum TrackedView {
  /// As the document reads with every change accepted: insertions kept, deletions dropped.
  Final,
  /// As it read before the changes: insertions dropped, deletions kept.
  Original,
}

/// How a document is read.
#[derive(Debug, Clone, Default)]
#[napi(object)]
pub struct DocxOptions {
  /// Which side of the tracked changes to read (default `final`).
  pub view: Option<TrackedView>,
}

/// The reader's options for what JavaScript passed, which may be nothing.
pub(crate) fn options_of(opts: Option<DocxOptions>) -> Options {
  let view = match opts.and_then(|opts| opts.view) {
    Some(TrackedView::Original) => View::Original,
    Some(TrackedView::Final) | None => View::Final,
  };
  Options { view }
}

// ── Paper ─────────────────────────────────────────────────────────

/// Declares the two paper-size unions from one list of sizes, so they cannot drift apart: the
/// standard sizes on their own, and the same sizes with `UNKNOWN` beside them.
macro_rules! paper_sizes {
  ($($size:ident),+ $(,)?) => {
    /// A standard paper size, as a union keyed on `type`, with its own dimensions in points,
    /// portrait: `width` is the short side.
    #[napi(discriminant_case = "UPPERCASE")]
    pub enum StandardPaperSize {
      $($size { width: f64, height: f64 },)+
    }

    /// The standard paper size nearest to a section's page, as a union keyed on `type`. The
    /// values of `type` are spelled the way a database enum spells them (`A4`, `LETTER`,
    /// `UNKNOWN`). A standard size carries its own dimensions in points, portrait: `width` is
    /// the short side. They are the standard's, not the page's; the page's are on the section.
    ///
    /// `UNKNOWN` carries `closestTo`, the standard sizes the page lies between by area: first the
    /// largest one no larger than the page, then the smallest one larger. A side is `null` when
    /// the page is beyond that end of the list (smaller than A10, or larger than B0), and both
    /// are when the section states no page size.
    #[napi(discriminant_case = "UPPERCASE")]
    pub enum PaperSize {
      $($size { width: f64, height: f64 },)+
      Unknown {
        closest_to: (Option<StandardPaperSize>, Option<StandardPaperSize>),
      },
    }

    impl StandardPaperSize {
      /// `None` for the unknown size, which has no dimensions of its own.
      fn new(size: contentdown::PaperSize) -> Option<Self> {
        let (width, height) = size.dimensions()?;
        match size {
          $(contentdown::PaperSize::$size => Some(StandardPaperSize::$size { width, height }),)+
          contentdown::PaperSize::Unknown => None,
        }
      }
    }

    impl From<StandardPaperSize> for PaperSize {
      fn from(value: StandardPaperSize) -> Self {
        match value {
          $(StandardPaperSize::$size { width, height } => PaperSize::$size { width, height },)+
        }
      }
    }
  };
}

paper_sizes!(
  A0, A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, B0, B1, B2, B3, B4, B5, B6, B7, B8, B9, B10, Letter,
  Legal, Tabloid, Executive, Statement
);

impl PaperSize {
  /// The union for the size of a section whose page is `width` by `height`, when it states one.
  fn new(size: contentdown::PaperSize, width: Option<f64>, height: Option<f64>) -> Self {
    match StandardPaperSize::new(size) {
      Some(standard) => standard.into(),
      None => {
        let (smaller, larger) = match (width, height) {
          (Some(width), Some(height)) => contentdown::paper_neighbors(width, height),
          _ => (None, None),
        };
        PaperSize::Unknown {
          closest_to: (
            smaller.and_then(StandardPaperSize::new),
            larger.and_then(StandardPaperSize::new),
          ),
        }
      }
    }
  }
}

/// A section's margins, in points.
#[napi(object, object_from_js = false)]
pub struct Margins {
  pub top: f64,
  pub right: f64,
  pub bottom: f64,
  pub left: f64,
}

/// A stretch of the document with one page setup. Lengths are in points.
#[napi(object, object_from_js = false)]
pub struct DocxSection {
  /// Absent when the section states no page size.
  pub width: Option<f64>,
  pub height: Option<f64>,
  pub landscape: bool,
  pub margins: Margins,
  /// Text columns.
  pub columns: u32,
  /// The standard paper size nearest to `width` by `height`, in either orientation; its `type`
  /// is `UNKNOWN` when none is within 3% on both sides, or the section states no size.
  pub paper_size: PaperSize,
  /// The page the section starts on, from 1.
  pub first_page: u32,
}

// ── Metadata ──────────────────────────────────────────────────────

/// What a document says about itself, and counts of what it holds.
#[napi(object, object_from_js = false)]
pub struct DocxMeta {
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  pub keywords: Option<String>,
  pub description: Option<String>,
  pub category: Option<String>,
  /// The default language of the text, as the styles state it: `en-US`.
  pub language: Option<String>,
  pub last_modified_by: Option<String>,
  pub revision: Option<String>,
  /// When the document was created and last saved, as the file writes them.
  pub created: Option<String>,
  pub modified: Option<String>,
  /// The application that last saved the file, and its version.
  pub application: Option<String>,
  pub app_version: Option<String>,
  pub company: Option<String>,
  pub template: Option<String>,
  /// The page count the application recorded when it saved the file; the reader's own count
  /// when it recorded none.
  pub page_count: u32,
  pub pages_recorded: Option<u32>,
  /// The pages the reader counted from page breaks: what `page` on a block, a picture or an
  /// annotation counts in. It can fall short of the recorded count, since Word does not mark
  /// every page it laid out, and it is 1 for a file no layout engine has saved.
  pub pages_counted: u32,
  /// Words of the body, counted from its text.
  pub word_count: u32,
  pub words_recorded: Option<u32>,
  /// Characters of the body that are not white space.
  pub character_count: u32,
  pub characters_recorded: Option<u32>,
  /// Lines as the application counted them when it saved the file. The reader has no count of
  /// its own to set beside it: a docx records no lines.
  pub lines_recorded: Option<u32>,
  /// Paragraphs that hold something, the ones in tables included.
  pub paragraph_count: u32,
  /// Paragraphs as the application counted them, by rules of its own.
  pub paragraphs_recorded: Option<u32>,
  pub heading_count: u32,
  pub list_item_count: u32,
  /// Tables, the ones inside cells included: the length of `tables()`.
  pub table_count: u32,
  /// Places a picture is used: the length of `images()`.
  pub image_count: u32,
  /// Links and comments: the length of `annotations()`.
  pub annotation_count: u32,
  pub link_count: u32,
  pub comment_count: u32,
  pub footnote_count: u32,
  pub endnote_count: u32,
  /// Content controls, form fields and merge fields: the length of `formFields()`.
  pub field_count: u32,
  pub header_count: u32,
  pub footer_count: u32,
  /// The package is macro-enabled. Macros are never read or run.
  pub has_macros: bool,
  /// The package is a template.
  pub is_template: bool,
  /// The document carries tracked insertions or deletions.
  pub has_tracked_changes: bool,
  /// Whether every section has the same page size.
  pub page_size_uniform: bool,
  pub sections: Vec<DocxSection>,
  /// What could not be read and was left out, in words.
  pub warnings: Vec<String>,
}

pub(crate) fn metadata_of(document: &Document) -> DocxMeta {
  let metadata = document.metadata();
  let props = &document.props;
  let sections = metadata
    .sections
    .into_iter()
    .map(|section| DocxSection {
      width: section.width,
      height: section.height,
      landscape: section.landscape,
      margins: Margins {
        top: section.margins.top,
        right: section.margins.right,
        bottom: section.margins.bottom,
        left: section.margins.left,
      },
      columns: section.columns,
      paper_size: PaperSize::new(section.paper_size, section.width, section.height),
      first_page: section.first_page,
    })
    .collect();
  DocxMeta {
    title: props.title.clone(),
    author: props.creator.clone(),
    subject: props.subject.clone(),
    keywords: props.keywords.clone(),
    description: props.description.clone(),
    category: props.category.clone(),
    language: document.language.clone(),
    last_modified_by: props.last_modified_by.clone(),
    revision: props.revision.clone(),
    created: props.created.clone(),
    modified: props.modified.clone(),
    application: props.application.clone(),
    app_version: props.app_version.clone(),
    company: props.company.clone(),
    template: props.template.clone(),
    page_count: metadata.page_count,
    pages_recorded: metadata.pages_recorded,
    pages_counted: metadata.pages_counted,
    word_count: metadata.word_count,
    words_recorded: metadata.words_recorded,
    character_count: metadata.character_count,
    characters_recorded: metadata.characters_recorded,
    lines_recorded: metadata.lines_recorded,
    paragraph_count: metadata.paragraph_count,
    paragraphs_recorded: metadata.paragraphs_recorded,
    heading_count: metadata.heading_count,
    list_item_count: metadata.list_item_count,
    table_count: metadata.table_count,
    image_count: metadata.image_count,
    annotation_count: metadata.annotation_count,
    link_count: metadata.link_count,
    comment_count: metadata.comment_count,
    footnote_count: metadata.footnote_count,
    endnote_count: metadata.endnote_count,
    field_count: metadata.field_count,
    header_count: metadata.header_count,
    footer_count: metadata.footer_count,
    has_macros: document.has_macros,
    is_template: document.is_template,
    has_tracked_changes: document.has_tracked_changes,
    page_size_uniform: metadata.page_size_uniform,
    sections,
    warnings: document.warnings.clone(),
  }
}

// ── Text ──────────────────────────────────────────────────────────

/// The text of one page, by the reader's page count.
#[napi(object, object_from_js = false)]
pub struct PageText {
  /// From 1.
  pub page: u32,
  pub text: String,
}

pub(crate) fn text_per_page_of(document: &Document) -> Vec<PageText> {
  document
    .text_per_page()
    .into_iter()
    .zip(1u32..)
    .map(|(text, page)| PageText { page, text })
    .collect()
}

// ── Blocks ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum Align {
  Left,
  Center,
  Right,
  Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum BreakKind {
  Line,
  Page,
  Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum VerticalAlign {
  Superscript,
  Subscript,
}

/// A paragraph's place in a list.
#[napi(object)]
pub struct ListItem {
  /// Nesting, from 0.
  pub level: u32,
  /// Numbered or lettered, as opposed to bulleted.
  pub ordered: bool,
  /// What stands in front of the paragraph: `2.`, `b)`, `iv.`, `•`.
  pub marker: String,
}

/// One thing inside a paragraph, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase")]
pub enum Inline {
  /// A stretch of text with one formatting, resolved through the styles. A formatting flag is
  /// present only when it is set.
  Text {
    text: String,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    vertical: Option<VerticalAlign>,
    /// In points.
    size: Option<f64>,
    font: Option<String>,
  },
  /// A hyperlink over a stretch of the paragraph: `uri` for an address outside the document,
  /// `dest` for a bookmark inside it.
  Link {
    uri: Option<String>,
    dest: Option<String>,
    /// The text the link covers.
    text: String,
    inlines: Vec<Inline>,
    page: u32,
  },
  /// A picture where it is used. `image` is its place in the document's pictures, which
  /// `images()` lists by placement with the same `name`.
  Image {
    image: u32,
    /// The part's path in the package, or the address of a linked picture.
    name: String,
    /// The size it is shown at, in points.
    display_width: Option<f64>,
    display_height: Option<f64>,
    alt_text: Option<String>,
    page: u32,
  },
  /// A reference to a footnote or an endnote: its place in `notes()`.
  NoteRef {
    note: u32,
  },
  /// The point a comment is attached at: the comment's `id`.
  CommentRef {
    comment: i64,
  },
  Break {
    kind: BreakKind,
  },
  Tab,
}

/// One row of a table in the block tree.
#[napi(object)]
pub struct TableRow {
  pub cells: Vec<TableCell>,
  /// The row repeats at the top of each page: a header row.
  pub header: bool,
}

/// A cell, which holds blocks of its own. A cell merged into the one above it is not listed: the
/// cell above carries the span.
#[napi(object)]
pub struct TableCell {
  /// The cell's text, one line per paragraph.
  pub text: String,
  pub blocks: Vec<Block>,
  /// The grid column the cell starts in, from 0.
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
}

/// One thing in the flow of a document, as a union keyed on `type`.
#[napi(discriminant_case = "camelCase")]
pub enum Block {
  Paragraph {
    /// The paragraph's own text: tabs and line breaks kept, a list marker not included.
    text: String,
    /// The name of the paragraph's style: `heading 1`, `List Paragraph`.
    style: Option<String>,
    /// 1 to 9 when the paragraph is a heading.
    heading: Option<u32>,
    list: Option<ListItem>,
    align: Option<Align>,
    inlines: Vec<Inline>,
    /// The page the paragraph starts on, from 1, by the reader's page count.
    page: u32,
    /// Index into `metadata().sections`.
    section: u32,
    /// Present, and true, when the paragraph sits in a text box, which floats beside the flow.
    text_box: Option<bool>,
  },
  Table {
    /// Column widths in points, from the table grid.
    columns: Vec<f64>,
    rows: Vec<TableRow>,
    page: u32,
    section: u32,
  },
}

fn inlines_of(document: &Document, inlines: &[docxdown::Inline]) -> Vec<Inline> {
  let set = |flag: bool| flag.then_some(true);
  inlines
    .iter()
    .map(|inline| match inline {
      docxdown::Inline::Text(run) => Inline::Text {
        text: run.text.clone(),
        bold: set(run.bold),
        italic: set(run.italic),
        underline: set(run.underline),
        strike: set(run.strike),
        vertical: match run.vertical {
          docxdown::VerticalAlign::Baseline => None,
          docxdown::VerticalAlign::Superscript => Some(VerticalAlign::Superscript),
          docxdown::VerticalAlign::Subscript => Some(VerticalAlign::Subscript),
        },
        size: run.size,
        font: run.font.clone(),
      },
      docxdown::Inline::Link(link) => Inline::Link {
        uri: link.uri.clone(),
        dest: link.dest.clone(),
        text: link.text(),
        inlines: inlines_of(document, &link.inlines),
        page: link.page,
      },
      docxdown::Inline::Image(used) => Inline::Image {
        image: used.image,
        name: document
          .images
          .get(used.image as usize)
          .map(|image| image.name.clone())
          .unwrap_or_default(),
        display_width: used.display_width,
        display_height: used.display_height,
        alt_text: used.alt_text.clone(),
        page: used.page,
      },
      docxdown::Inline::NoteRef(note) => Inline::NoteRef { note: *note },
      docxdown::Inline::CommentRef(comment) => Inline::CommentRef {
        comment: document
          .comments
          .get(*comment as usize)
          .map_or(-1, |comment| comment.id),
      },
      docxdown::Inline::Break(kind) => Inline::Break {
        kind: match kind {
          docxdown::BreakKind::Line => BreakKind::Line,
          docxdown::BreakKind::Page => BreakKind::Page,
          docxdown::BreakKind::Column => BreakKind::Column,
        },
      },
      docxdown::Inline::Tab => Inline::Tab,
    })
    .collect()
}

/// Blocks of the document as JavaScript sees them: the body's, a note's, a header's.
pub(crate) fn blocks_from(document: &Document, blocks: &[docxdown::Block]) -> Vec<Block> {
  blocks
    .iter()
    .map(|block| match block {
      docxdown::Block::Paragraph(paragraph) => Block::Paragraph {
        text: paragraph.text(),
        style: paragraph.style.clone(),
        heading: paragraph.heading.map(u32::from),
        list: paragraph.list.as_ref().map(|item| ListItem {
          level: u32::from(item.level),
          ordered: item.ordered,
          marker: item.marker.clone(),
        }),
        align: paragraph.align.map(|align| match align {
          docxdown::Align::Left => Align::Left,
          docxdown::Align::Center => Align::Center,
          docxdown::Align::Right => Align::Right,
          docxdown::Align::Justify => Align::Justify,
        }),
        inlines: inlines_of(document, &paragraph.inlines),
        page: paragraph.page,
        section: paragraph.section,
        text_box: paragraph.text_box.then_some(true),
      },
      docxdown::Block::Table(table) => Block::Table {
        columns: table.columns.clone(),
        rows: table
          .rows
          .iter()
          .map(|row| TableRow {
            cells: row
              .cells
              .iter()
              .map(|cell| TableCell {
                text: cell.lines(),
                blocks: blocks_from(document, &cell.blocks),
                column: cell.column,
                col_span: cell.col_span,
                row_span: cell.row_span,
              })
              .collect(),
            header: row.header,
          })
          .collect(),
        page: table.page,
        section: table.section,
      },
    })
    .collect()
}

pub(crate) fn blocks_of(document: &Document) -> Vec<Block> {
  blocks_from(document, &document.blocks)
}

// ── Tables ────────────────────────────────────────────────────────

/// A column's letters, ordered the way columns are: `Z` before `AA`. Plain text order would put
/// `AA` before `B`, and JavaScript keeps an object's letter keys in the order they were added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnKey(pub String);

impl AsRef<str> for ColumnKey {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl Ord for ColumnKey {
  fn cmp(&self, other: &Self) -> Ordering {
    self
      .0
      .len()
      .cmp(&other.0.len())
      .then_with(|| self.0.cmp(&other.0))
  }
}

impl PartialOrd for ColumnKey {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

/// One cell of a table, at the top-left corner of the range it covers.
#[napi(object, object_from_js = false)]
pub struct DocxTableCell {
  /// The cell as a sheet would refer to it: `B2`.
  #[napi(js_name = "ref")]
  pub reference: String,
  /// From 0.
  pub row: u32,
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
  /// The cell's text, one line per paragraph.
  pub text: String,
}

/// A table with its cells keyed for lookup: `rows["2"]["B"]` is the cell in the second row and
/// the second column, and `rows["2"]` is the whole of the second row. A cell merged into another
/// has no key; the cell it is merged into carries the span, and `merges` names the range.
#[napi(object, object_from_js = false)]
pub struct DocxTable {
  /// The table's place among the document's tables, in reading order, from 0. A table inside a
  /// cell follows the table that holds it.
  pub index: u32,
  /// The table whose cell holds this one, when it sits inside another.
  pub parent: Option<u32>,
  /// The page the table starts on, from 1.
  pub page: u32,
  /// Column widths in points, from the table grid.
  pub columns: Vec<f64>,
  pub row_count: u32,
  pub column_count: u32,
  /// How many rows at the top repeat on each page: the header rows.
  pub header_rows: u32,
  /// The ranges merged cells cover, in A1 form: `A1:B2`.
  pub merges: Vec<String>,
  /// Keyed by row number from 1 (`"2"`), then by column letters (`"B"`), each in table order.
  #[napi(ts_type = "Record<string, Record<string, DocxTableCell>>")]
  pub rows: BTreeMap<String, BTreeMap<ColumnKey, DocxTableCell>>,
}

pub(crate) fn tables_of(document: &Document) -> Vec<DocxTable> {
  document
    .tables()
    .into_iter()
    .map(|table| {
      let mut rows: BTreeMap<String, BTreeMap<ColumnKey, DocxTableCell>> = BTreeMap::new();
      for cell in table.cells {
        rows
          .entry((u64::from(cell.row) + 1).to_string())
          .or_default()
          .insert(
            ColumnKey(column_letters(cell.column)),
            DocxTableCell {
              reference: docxdown::cell_ref(cell.row, cell.column),
              row: cell.row,
              column: cell.column,
              col_span: cell.col_span,
              row_span: cell.row_span,
              text: cell.text,
            },
          );
      }
      DocxTable {
        index: table.index,
        parent: table.parent,
        page: table.page,
        columns: table.columns,
        row_count: table.row_count,
        column_count: table.column_count,
        header_rows: table.header_rows,
        merges: table.merges,
        rows,
      }
    })
    .collect()
}

// ── Pictures and annotations ──────────────────────────────────────

/// Where in a document something sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum PartKind {
  Body,
  Header,
  Footer,
  Footnote,
  Endnote,
  /// Inside the text of a comment.
  Comment,
}

impl From<docxdown::PartKind> for PartKind {
  fn from(part: docxdown::PartKind) -> Self {
    match part {
      docxdown::PartKind::Body => PartKind::Body,
      docxdown::PartKind::Header => PartKind::Header,
      docxdown::PartKind::Footer => PartKind::Footer,
      docxdown::PartKind::Footnote => PartKind::Footnote,
      docxdown::PartKind::Endnote => PartKind::Endnote,
      docxdown::PartKind::Comment => PartKind::Comment,
    }
  }
}

/// A picture where it is placed: one entry for each place a picture is used.
#[napi(object, object_from_js = false)]
pub struct DocxImage {
  /// From 1, by the reader's page count. Absent in a header or a footer, which belong to no one
  /// page, and in a note or a comment that nothing refers to.
  pub page: Option<u32>,
  pub part: PartKind,
  /// Its place among the pictures of the same page, from 0.
  pub image_index: u32,
  /// The part's path in the package (`word/media/image1.png`), or the address of a linked
  /// picture.
  pub name: String,
  /// The format, read from the bytes themselves; absent for a linked picture or a format that is
  /// not recognized.
  pub mime: Option<String>,
  /// Pixels, read from the picture's own header; absent for vector formats.
  pub width: Option<u32>,
  pub height: Option<u32>,
  /// The bytes as the document stores them, not re-encoded. Empty for a linked picture, which
  /// lives outside the file.
  pub data: Buffer,
  /// The picture is linked to, not embedded.
  pub linked: bool,
  /// The size it is shown at, in points.
  pub display_width: Option<f64>,
  pub display_height: Option<f64>,
  pub alt_text: Option<String>,
}

pub(crate) fn images_of(document: &Document) -> Vec<DocxImage> {
  document
    .placements()
    .into_iter()
    .filter_map(|placement| {
      let image = document.images.get(placement.image as usize)?;
      Some(DocxImage {
        page: placement.page,
        part: placement.part.into(),
        image_index: placement.image_index,
        name: image.name.clone(),
        mime: image.mime.map(str::to_owned),
        width: image.width,
        height: image.height,
        data: image.data.clone().into(),
        linked: image.linked,
        display_width: placement.display_width,
        display_height: placement.display_height,
        alt_text: placement.alt_text,
      })
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum AnnotationSubtype {
  Link,
  Comment,
}

/// A hyperlink or a comment.
#[napi(object, object_from_js = false)]
pub struct DocxAnnotation {
  /// From 1, by the reader's page count. Absent in a header or a footer, which belong to no one
  /// page, and for a note or a comment that nothing refers to.
  pub page: Option<u32>,
  pub part: PartKind,
  pub subtype: AnnotationSubtype,
  /// Where an external link leads.
  pub uri: Option<String>,
  /// The bookmark an internal link jumps to.
  pub dest: Option<String>,
  /// What a comment says.
  pub content: Option<String>,
  /// The text the link covers or the comment is attached to.
  pub text: String,
  /// Who wrote a comment, and when, as the file writes the date.
  pub author: Option<String>,
  pub date: Option<String>,
}

pub(crate) fn annotations_of(document: &Document) -> Vec<DocxAnnotation> {
  document
    .annotations()
    .into_iter()
    .map(|annotation| DocxAnnotation {
      page: annotation.page,
      part: annotation.part.into(),
      subtype: match annotation.subtype {
        docxdown::AnnotationKind::Link => AnnotationSubtype::Link,
        docxdown::AnnotationKind::Comment => AnnotationSubtype::Comment,
      },
      uri: annotation.uri,
      dest: annotation.dest,
      content: annotation.content,
      text: annotation.text,
      author: annotation.author,
      date: annotation.date,
    })
    .collect()
}

// ── Fields and notes ──────────────────────────────────────────────

/// What a field is: a content control by what it holds (`text` to `picture`), a legacy form
/// field (`formText`, `formCheckbox`, `formDropDown`), or a mail-merge field (`merge`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "camelCase")]
pub enum FieldKind {
  Text,
  RichText,
  Date,
  DropDown,
  ComboBox,
  Checkbox,
  Picture,
  FormText,
  FormCheckbox,
  FormDropDown,
  Merge,
}

/// A place in the document made to hold a value.
#[napi(object, object_from_js = false)]
pub struct DocxFormField {
  pub kind: FieldKind,
  /// The machine name: a content control's tag, a form field's name, a merge field's name.
  pub name: Option<String>,
  /// The label shown to the person filling it in.
  pub title: Option<String>,
  /// What it holds now: `true` or `false` for a checkbox, and empty while a content control
  /// still shows its placeholder.
  pub value: String,
  pub page: u32,
}

pub(crate) fn form_fields_of(document: &Document) -> Vec<DocxFormField> {
  document
    .fields
    .iter()
    .map(|field| DocxFormField {
      kind: match field.kind {
        docxdown::FieldKind::Text => FieldKind::Text,
        docxdown::FieldKind::RichText => FieldKind::RichText,
        docxdown::FieldKind::Date => FieldKind::Date,
        docxdown::FieldKind::DropDown => FieldKind::DropDown,
        docxdown::FieldKind::ComboBox => FieldKind::ComboBox,
        docxdown::FieldKind::Checkbox => FieldKind::Checkbox,
        docxdown::FieldKind::Picture => FieldKind::Picture,
        docxdown::FieldKind::FormText => FieldKind::FormText,
        docxdown::FieldKind::FormCheckbox => FieldKind::FormCheckbox,
        docxdown::FieldKind::FormDropDown => FieldKind::FormDropDown,
        docxdown::FieldKind::Merge => FieldKind::Merge,
      },
      name: field.name.clone(),
      title: field.title.clone(),
      value: field.value.clone(),
      page: field.page,
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum NoteKind {
  Footnote,
  Endnote,
}

/// A footnote or an endnote.
#[napi(object, object_from_js = false)]
pub struct DocxNote {
  pub kind: NoteKind,
  /// The id the document gives it, unique within its kind.
  pub id: i64,
  /// The note's text, one line per paragraph.
  pub text: String,
  pub blocks: Vec<Block>,
  /// The page of the paragraph that refers to it; absent when nothing does.
  pub page: Option<u32>,
}

/// A paragraph's text as a line: a list item with its marker in front.
fn lines_of(blocks: &[docxdown::Block]) -> String {
  blocks
    .iter()
    .map(|block| match block {
      docxdown::Block::Paragraph(paragraph) => paragraph.line(),
      docxdown::Block::Table(table) => table.text(),
    })
    .collect::<Vec<_>>()
    .join("\n")
}

pub(crate) fn notes_of(document: &Document) -> Vec<DocxNote> {
  document
    .notes
    .iter()
    .map(|note| DocxNote {
      kind: match note.kind {
        docxdown::NoteKind::Footnote => NoteKind::Footnote,
        docxdown::NoteKind::Endnote => NoteKind::Endnote,
      },
      id: note.id,
      text: lines_of(&note.blocks),
      blocks: blocks_from(document, &note.blocks),
      page: (note.page > 0).then_some(note.page),
    })
    .collect()
}

// ── The whole document ────────────────────────────────────────────

/// A header or a footer.
#[napi(object, object_from_js = false)]
pub struct DocxPart {
  /// The part's path in the package: `word/header1.xml`.
  pub name: String,
  /// Its text, one line per paragraph.
  pub text: String,
  pub blocks: Vec<Block>,
}

fn parts_of(document: &Document, parts: &[docxdown::Part]) -> Vec<DocxPart> {
  parts
    .iter()
    .map(|part| DocxPart {
      name: part.name.clone(),
      text: lines_of(&part.blocks),
      blocks: blocks_from(document, &part.blocks),
    })
    .collect()
}

/// Everything read from a document, in one object: what each reader returns under the reader's
/// own name, and the headers and footers. The page layout and the invoice fields, which are
/// estimates, are left to `layout()` and `invoice()`.
#[napi(object, object_from_js = false)]
pub struct DocxDocument {
  pub metadata: DocxMeta,
  pub text: String,
  pub text_per_page: Vec<PageText>,
  pub markdown: String,
  pub blocks: Vec<Block>,
  pub tables: Vec<DocxTable>,
  pub images: Vec<DocxImage>,
  pub annotations: Vec<DocxAnnotation>,
  pub form_fields: Vec<DocxFormField>,
  pub notes: Vec<DocxNote>,
  pub headers: Vec<DocxPart>,
  pub footers: Vec<DocxPart>,
}

pub(crate) fn document_of(document: &Document) -> DocxDocument {
  DocxDocument {
    metadata: metadata_of(document),
    text: document.text(),
    text_per_page: text_per_page_of(document),
    markdown: document.markdown(),
    blocks: blocks_of(document),
    tables: tables_of(document),
    images: images_of(document),
    annotations: annotations_of(document),
    form_fields: form_fields_of(document),
    notes: notes_of(document),
    headers: parts_of(document, &document.headers),
    footers: parts_of(document, &document.footers),
  }
}
