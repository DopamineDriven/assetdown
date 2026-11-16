//! A presentation as plain data: slides in order, the shapes on each, and what hangs off them.

use ooxmldown::{Chart, DocProps};

/// Where a shape is: its box in points, measured from the slide's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
  /// Clockwise, in degrees. It is reported and not applied: the box is the shape's before it
  /// is turned.
  pub rotation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
  Left,
  Center,
  Right,
  Justify,
}

/// What a placeholder is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderKind {
  Title,
  CenteredTitle,
  Subtitle,
  Body,
  Date,
  Footer,
  SlideNumber,
  Header,
  Picture,
  Chart,
  Table,
  Diagram,
  Media,
  ClipArt,
  /// Content of any kind, which is what a placeholder that names no kind holds.
  Object,
  /// The picture of the slide on a notes page.
  SlideImage,
}

impl PlaceholderKind {
  /// The kind `p:ph` names in its `type`; one that names none holds content of any kind.
  pub(crate) fn parse(kind: Option<&str>) -> PlaceholderKind {
    match kind {
      Some("title") => PlaceholderKind::Title,
      Some("ctrTitle") => PlaceholderKind::CenteredTitle,
      Some("subTitle") => PlaceholderKind::Subtitle,
      Some("body") => PlaceholderKind::Body,
      Some("dt") => PlaceholderKind::Date,
      Some("ftr") => PlaceholderKind::Footer,
      Some("sldNum") => PlaceholderKind::SlideNumber,
      Some("hdr") => PlaceholderKind::Header,
      Some("pic") => PlaceholderKind::Picture,
      Some("chart") => PlaceholderKind::Chart,
      Some("tbl") => PlaceholderKind::Table,
      Some("dgm") => PlaceholderKind::Diagram,
      Some("media") => PlaceholderKind::Media,
      Some("clipArt") => PlaceholderKind::ClipArt,
      Some("sldImg") => PlaceholderKind::SlideImage,
      _ => PlaceholderKind::Object,
    }
  }

  pub fn is_title(self) -> bool {
    matches!(
      self,
      PlaceholderKind::Title | PlaceholderKind::CenteredTitle
    )
  }

  /// A date, a footer, a slide number or a header: what a deck repeats on every slide.
  pub fn is_furniture(self) -> bool {
    matches!(
      self,
      PlaceholderKind::Date
        | PlaceholderKind::Footer
        | PlaceholderKind::SlideNumber
        | PlaceholderKind::Header
    )
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placeholder {
  pub kind: PlaceholderKind,
  /// The number that ties it to the placeholder of its layout.
  pub index: Option<u32>,
}

/// Where a link leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
  /// An address outside the deck.
  External(String),
  /// Another slide of the deck, by its number.
  Slide(u32),
  /// A jump with no fixed target, in the file's own word: `nextslide`, `endshow`.
  Action(String),
}

/// A stretch of text with one formatting, resolved through the layout and the master.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
  /// A line break inside a paragraph is a run whose text is a line break.
  pub text: String,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strike: bool,
  /// In points.
  pub size: Option<f64>,
  pub font: Option<String>,
  pub link: Option<LinkTarget>,
  /// The kind of field the text is the last saved value of: `slidenum`, `datetime1`.
  pub field: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paragraph {
  /// Nesting, from 0.
  pub level: u8,
  /// What stands in front of the paragraph: `•`, `2.`, `b)`. Absent when it has no bullet.
  pub bullet: Option<String>,
  /// The bullet is a number or a letter that counts.
  pub ordered: bool,
  pub align: Option<Align>,
  pub runs: Vec<Run>,
}

/// What every shape has.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShapeInfo {
  /// Absent when neither the shape, its layout nor its master states a box.
  pub frame: Option<Frame>,
  /// The shape's place in the order the file stacks them, back to front, from 0.
  pub stacking: u32,
  pub name: Option<String>,
  pub alt_text: Option<String>,
  pub placeholder: Option<Placeholder>,
  /// A link on the whole shape.
  pub link: Option<LinkTarget>,
}

/// The space between a text box's edges and its text, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Insets {
  pub left: f64,
  pub top: f64,
  pub right: f64,
  pub bottom: f64,
}

/// Where text stands in a box that is taller than it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Anchor {
  #[default]
  Top,
  Middle,
  Bottom,
}

/// A text box, or a placeholder that holds text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextShape {
  pub info: ShapeInfo,
  pub paragraphs: Vec<Paragraph>,
  pub insets: Insets,
  pub anchor: Anchor,
  /// Lines wrap at the box's width, as opposed to running on.
  pub wrap: bool,
  /// The fraction of its size the text was shrunk to so that it fits its box; 1 when it was not.
  pub font_scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
  pub info: ShapeInfo,
  /// Index into [`Presentation::images`].
  pub image: u32,
  /// The SVG the picture is drawn from, when the file keeps one beside the raster copy that
  /// `image` is: index into [`Presentation::images`].
  pub vector: Option<u32>,
  /// The picture is the slide's background, behind every shape.
  pub background: bool,
}

/// A cell of a table. A cell another one is merged over is not listed.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
  /// The grid column the cell starts in, from 0.
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
  pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
  /// In points.
  pub height: f64,
  pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
  pub info: ShapeInfo,
  /// Column widths in points.
  pub columns: Vec<f64>,
  pub rows: Vec<TableRow>,
}

/// A chart, as its data.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartShape {
  pub info: ShapeInfo,
  pub chart: Chart,
}

/// SmartArt.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
  pub info: ShapeInfo,
  /// What the application drew the diagram as, when the deck keeps that: shapes with boxes.
  pub shapes: Vec<Shape>,
  /// The text of the diagram's nodes, in the order the deck lists them, when it keeps no
  /// drawing.
  pub nodes: Vec<String>,
}

/// An audio or a video clip on a slide.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaShape {
  pub info: ShapeInfo,
  /// Index into [`Presentation::media`].
  pub media: u32,
  /// The picture shown for the clip: index into [`Presentation::images`].
  pub poster: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
  pub info: ShapeInfo,
  pub shapes: Vec<Shape>,
}

/// One thing on a slide.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
  Text(TextShape),
  Picture(Picture),
  Table(Table),
  Chart(ChartShape),
  Diagram(Diagram),
  Media(MediaShape),
  Group(Group),
}

impl Shape {
  pub fn info(&self) -> &ShapeInfo {
    match self {
      Shape::Text(shape) => &shape.info,
      Shape::Picture(shape) => &shape.info,
      Shape::Table(shape) => &shape.info,
      Shape::Chart(shape) => &shape.info,
      Shape::Diagram(shape) => &shape.info,
      Shape::Media(shape) => &shape.info,
      Shape::Group(shape) => &shape.info,
    }
  }

  /// Whether the shape is a date, a footer, a slide number or a header placeholder.
  pub fn is_furniture(&self) -> bool {
    self
      .info()
      .placeholder
      .is_some_and(|placeholder| placeholder.kind.is_furniture())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
  pub author: Option<String>,
  pub initials: Option<String>,
  /// As the file writes it.
  pub date: Option<String>,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
  /// From 1, in the order of the deck, hidden slides counted.
  pub number: u32,
  /// The slide's part in the package: `ppt/slides/slide3.xml`.
  pub name: String,
  /// The text of the title placeholder.
  pub title: Option<String>,
  /// The name of the layout the slide is built on: `Title and Content`.
  pub layout: Option<String>,
  pub section: Option<String>,
  /// The slide is left out when the deck is shown.
  pub hidden: bool,
  /// In reading order: the title, then top to bottom and left to right.
  pub shapes: Vec<Shape>,
  /// The speaker notes.
  pub notes: Vec<Paragraph>,
  pub comments: Vec<Comment>,
}

/// A picture the deck holds, once however often it is used.
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
pub enum MediaKind {
  Audio,
  Video,
}

/// An audio or a video clip the deck holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
  /// The part's path in the package, or the address of a linked clip.
  pub name: String,
  pub kind: MediaKind,
  /// As the package types the part: `video/mp4`.
  pub mime: Option<String>,
  /// The bytes as stored. Empty for a linked clip.
  pub data: Vec<u8>,
  pub linked: bool,
}

/// A whole presentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Presentation {
  pub props: DocProps,
  /// The package is macro-enabled. Macros are never read or run.
  pub has_macros: bool,
  /// The package is a template.
  pub is_template: bool,
  /// The package is saved to open straight into the show.
  pub is_show: bool,
  /// The size of a slide, in points.
  pub width: f64,
  pub height: f64,
  /// What the slides are sized for, in the file's word: `screen16x9`, `letter`.
  pub size_kind: Option<String>,
  /// The language most of the text says it is in: `en-US`.
  pub language: Option<String>,
  /// How many slides the deck lists, hidden ones counted, whether or not they were read.
  pub slide_count: u32,
  pub slides: Vec<Slide>,
  pub images: Vec<Image>,
  pub media: Vec<Media>,
  /// What could not be read and was left out, in words.
  pub warnings: Vec<String>,
}
