//! A document as plain data: blocks in reading order, and what hangs off them.

use ooxmldown::DocProps;

/// One thing in the flow of a document.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
  Paragraph(Paragraph),
  Table(Table),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
  Left,
  Center,
  Right,
  Justify,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
  /// The name of the paragraph's style: `heading 1`, `List Paragraph`.
  pub style: Option<String>,
  /// 1 to 9 when the paragraph is a heading.
  pub heading: Option<u8>,
  pub list: Option<ListItem>,
  pub align: Option<Align>,
  pub inlines: Vec<Inline>,
  /// Index into [`Document::sections`].
  pub section: u32,
  /// The page the paragraph starts on, from 1; see [`crate::pages`].
  pub page: u32,
  /// The paragraph sits in a text box, which floats beside the flow.
  pub text_box: bool,
}

/// A paragraph's place in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
  /// Nesting, from 0.
  pub level: u8,
  /// Numbered or lettered, as opposed to bulleted.
  pub ordered: bool,
  /// What stands in front of the paragraph: `2.`, `b)`, `iv.`, `•`.
  pub marker: String,
}

/// One thing inside a paragraph.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
  Text(TextRun),
  Link(Link),
  Image(ImageRef),
  /// Index into [`Document::notes`].
  NoteRef(u32),
  /// Index into [`Document::comments`].
  CommentRef(u32),
  Break(BreakKind),
  Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakKind {
  Line,
  Page,
  Column,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerticalAlign {
  #[default]
  Baseline,
  Superscript,
  Subscript,
}

/// A stretch of text with one formatting, resolved through the styles.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextRun {
  pub text: String,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strike: bool,
  pub vertical: VerticalAlign,
  /// In points.
  pub size: Option<f64>,
  pub font: Option<String>,
}

/// A hyperlink over a stretch of the paragraph.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
  /// Where an external link leads.
  pub uri: Option<String>,
  /// The bookmark an internal link jumps to.
  pub dest: Option<String>,
  pub inlines: Vec<Inline>,
  /// The page the link starts on.
  pub page: u32,
}

/// A picture where it is used.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
  /// Index into [`Document::images`].
  pub image: u32,
  /// The size it is shown at, in points.
  pub display_width: Option<f64>,
  pub display_height: Option<f64>,
  pub alt_text: Option<String>,
  /// The page it is placed on.
  pub page: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
  /// Column widths in points, from the table grid.
  pub columns: Vec<f64>,
  pub rows: Vec<Row>,
  pub section: u32,
  pub page: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
  pub cells: Vec<Cell>,
  /// The row repeats at the top of each page: a header row.
  pub header: bool,
}

/// A cell, which holds blocks of its own. A cell merged into the one above it is not listed: the
/// cell above carries the span.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
  pub blocks: Vec<Block>,
  /// The grid column the cell starts in, from 0.
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Margins {
  pub top: f64,
  pub right: f64,
  pub bottom: f64,
  pub left: f64,
}

/// A stretch of the document with one page setup. Lengths are in points.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
  /// Absent when the section states no page size.
  pub width: Option<f64>,
  pub height: Option<f64>,
  pub landscape: bool,
  pub margins: Margins,
  /// Text columns.
  pub columns: u32,
  /// The section starts on a new page, as opposed to continuing on the same one.
  pub starts_page: bool,
  /// Its first page has a header and footer of its own.
  pub title_page: bool,
  /// The headers it uses, as indices into [`Document::headers`].
  pub headers: PartRefs,
  /// The footers it uses, as indices into [`Document::footers`].
  pub footers: PartRefs,
}

/// The header or footer a section uses on each kind of page. A section that names none uses
/// the ones of the section before it, which is already applied here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartRefs {
  pub default: Option<u32>,
  /// On the first page, when the section has a title page.
  pub first: Option<u32>,
  /// On even pages, when the document tells odd and even pages apart.
  pub even: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteKind {
  Footnote,
  Endnote,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
  pub kind: NoteKind,
  /// The id the reference uses, unique within its kind.
  pub id: i64,
  pub blocks: Vec<Block>,
  /// The page of the paragraph that refers to it; 0 when nothing does.
  pub page: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
  pub id: i64,
  pub author: Option<String>,
  pub initials: Option<String>,
  /// As written: `2026-11-15T09:30:00Z`.
  pub date: Option<String>,
  pub blocks: Vec<Block>,
  /// The text the comment is attached to, from the range it marks.
  pub anchor_text: String,
  /// The page of the paragraph that refers to it; 0 when nothing does.
  pub page: u32,
}

/// A header or a footer.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
  /// The part's path in the package: `word/header1.xml`.
  pub name: String,
  pub blocks: Vec<Block>,
}

/// A picture the document holds, once however often it is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
  /// The part's path in the package, or the address of a linked picture.
  pub name: String,
  /// From the bytes themselves; absent for a linked picture or a format that is not recognized.
  pub mime: Option<&'static str>,
  /// Pixels, for raster formats whose header states them.
  pub width: Option<u32>,
  pub height: Option<u32>,
  /// The bytes as stored. Empty for a linked picture, which lives outside the file.
  pub data: Vec<u8>,
  pub linked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
  /// Content controls, by what they hold.
  Text,
  RichText,
  Date,
  DropDown,
  ComboBox,
  Checkbox,
  Picture,
  /// Legacy form fields.
  FormText,
  FormCheckbox,
  FormDropDown,
  /// A mail-merge field.
  Merge,
}

/// A place in the document made to hold a value: a content control, a form field, a merge field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
  pub kind: FieldKind,
  /// The machine name: a content control's tag, a form field's name, a merge field's name.
  pub name: Option<String>,
  /// The label shown to the person filling it in.
  pub title: Option<String>,
  /// What it holds now. Empty while a content control still shows its placeholder.
  pub value: String,
  pub page: u32,
}

/// A whole document.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
  pub props: DocProps,
  /// The default language of the text, from the styles: `en-US`.
  pub language: Option<String>,
  /// The package is macro-enabled. Macros are never read or run.
  pub has_macros: bool,
  /// The package is a template.
  pub is_template: bool,
  /// The document carries tracked insertions or deletions.
  pub has_tracked_changes: bool,
  pub sections: Vec<Section>,
  pub blocks: Vec<Block>,
  pub headers: Vec<Part>,
  pub footers: Vec<Part>,
  pub notes: Vec<Note>,
  pub comments: Vec<Comment>,
  pub images: Vec<Image>,
  pub fields: Vec<Field>,
  /// How many pages the reader counted; see [`crate::pages`].
  pub page_count: u32,
  /// What could not be read and was left out, in words.
  pub warnings: Vec<String>,
}
