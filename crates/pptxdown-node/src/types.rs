//! The shapes JavaScript sees. Plain data: each value is built from `pptxdown`'s model and copied
//! out. Only the options are ever read back from JavaScript.
//!
//! A reader whose result carries bytes (a picture, a clip) has two forms here: what is read,
//! which holds the bytes as a `Vec<u8>` and can be built on a worker thread, and what JavaScript
//! is given, whose `Buffer` is made from it back on the JavaScript thread.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use pptxdown::{LinkTarget, Options, Presentation, ShapeInfo, column_letters};

// ── Options ───────────────────────────────────────────────────────

/// How a deck is read.
#[derive(Debug, Clone, Default)]
#[napi(object)]
pub struct PptxOptions {
  /// Whether hidden slides are read (default `true`). Left out, the slides after a hidden one
  /// keep their numbers, and `metadata().slideCount` still counts it.
  pub hidden_slides: Option<bool>,
}

/// The reader's options for what JavaScript passed, which may be nothing.
pub(crate) fn options_of(opts: Option<PptxOptions>) -> Options {
  let defaults = Options::default();
  Options {
    hidden_slides: opts
      .and_then(|opts| opts.hidden_slides)
      .unwrap_or(defaults.hidden_slides),
  }
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

    /// The standard paper size nearest to a slide, as a union keyed on `type`. The values of
    /// `type` are spelled the way a database enum spells them (`A4`, `LETTER`, `UNKNOWN`). A
    /// standard size carries its own dimensions in points, portrait: `width` is the short side.
    /// They are the standard's, not the slide's; the slide's are on the metadata.
    ///
    /// A slide sized for a screen is no paper size, so most decks are `UNKNOWN`. It carries
    /// `closestTo`, the standard sizes the slide lies between by area: first the largest one no
    /// larger than the slide, then the smallest one larger. A side is `null` when the slide is
    /// beyond that end of the list (smaller than A10, or larger than B0).
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
  /// The union for the size of a slide that is `width` by `height`.
  fn new(size: contentdown::PaperSize, width: f64, height: f64) -> Self {
    match StandardPaperSize::new(size) {
      Some(standard) => standard.into(),
      None => {
        let (smaller, larger) = contentdown::paper_neighbors(width, height);
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

// ── Metadata ──────────────────────────────────────────────────────

/// What a deck says about itself, and counts of what it holds.
#[napi(object, object_from_js = false)]
pub struct PptxMeta {
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  pub keywords: Option<String>,
  pub description: Option<String>,
  pub category: Option<String>,
  /// The language most of the text says it is in: `en-US`.
  pub language: Option<String>,
  pub last_modified_by: Option<String>,
  pub revision: Option<String>,
  /// When the deck was created and last saved, as the file writes them.
  pub created: Option<String>,
  pub modified: Option<String>,
  /// The application that last saved the file, and its version.
  pub application: Option<String>,
  pub app_version: Option<String>,
  pub company: Option<String>,
  pub template: Option<String>,
  /// What the deck is sized for, in the application's words: `Widescreen`.
  pub presentation_format: Option<String>,
  /// The slides the deck lists, hidden ones counted, whether or not they were read.
  pub slide_count: u32,
  /// The count the application recorded when it saved the file.
  pub slides_recorded: Option<u32>,
  pub hidden_slide_count: u32,
  pub hidden_slides_recorded: Option<u32>,
  /// Slides that have speaker notes: the length of `notes()`.
  pub notes_count: u32,
  /// Notes pages as the application counted them. PowerPoint keeps one for every slide, with or
  /// without notes on it, so this is usually the slide count.
  pub notes_recorded: Option<u32>,
  /// Words on the slides, counted from their text. Notes are not counted.
  pub word_count: u32,
  pub words_recorded: Option<u32>,
  /// Characters on the slides that are not white space.
  pub character_count: u32,
  /// Paragraphs on the slides that hold text.
  pub paragraph_count: u32,
  /// Paragraphs as the application counted them, by rules of its own.
  pub paragraphs_recorded: Option<u32>,
  /// The length of `tables()`.
  pub table_count: u32,
  /// The length of `charts()`.
  pub chart_count: u32,
  /// SmartArt diagrams.
  pub diagram_count: u32,
  /// Places a picture is used: the length of `images()`.
  pub image_count: u32,
  /// Places an audio or a video clip is used: the length of `media()`.
  pub media_count: u32,
  /// Links and comments: the length of `annotations()`.
  pub annotation_count: u32,
  pub link_count: u32,
  pub comment_count: u32,
  /// The size of a slide, in points. A deck has one size for all of its slides.
  pub width: f64,
  pub height: f64,
  /// The slide's shape in the usual words: `16:9`, `4:3`.
  pub aspect_ratio: String,
  /// What the slides are sized for, in the file's own word: `screen16x9`, `letter`. Absent when
  /// the size is a custom one.
  pub size_kind: Option<String>,
  /// The standard paper size nearest to `width` by `height`, in either orientation; its `type`
  /// is `UNKNOWN` when none is within 3% on both sides, as for a slide sized for a screen.
  pub paper_size: PaperSize,
  /// The names of the layouts the slides are built on, each once, in the order they are met.
  pub layouts: Vec<String>,
  /// The names of the deck's sections, in order.
  pub sections: Vec<String>,
  /// The package is macro-enabled. Macros are never read or run.
  pub has_macros: bool,
  /// The package is a template.
  pub is_template: bool,
  /// The package is saved to open straight into the show (`.ppsx`).
  pub is_show: bool,
  /// What could not be read and was left out, in words.
  pub warnings: Vec<String>,
}

pub(crate) fn metadata_of(deck: &Presentation) -> PptxMeta {
  let metadata = deck.metadata();
  let props = &deck.props;
  PptxMeta {
    title: props.title.clone(),
    author: props.creator.clone(),
    subject: props.subject.clone(),
    keywords: props.keywords.clone(),
    description: props.description.clone(),
    category: props.category.clone(),
    language: deck.language.clone(),
    last_modified_by: props.last_modified_by.clone(),
    revision: props.revision.clone(),
    created: props.created.clone(),
    modified: props.modified.clone(),
    application: props.application.clone(),
    app_version: props.app_version.clone(),
    company: props.company.clone(),
    template: props.template.clone(),
    presentation_format: props.presentation_format.clone(),
    slide_count: metadata.slide_count,
    slides_recorded: metadata.slides_recorded,
    hidden_slide_count: metadata.hidden_slide_count,
    hidden_slides_recorded: props.hidden_slides,
    notes_count: metadata.notes_count,
    notes_recorded: props.notes,
    word_count: metadata.word_count,
    words_recorded: metadata.words_recorded,
    character_count: metadata.character_count,
    paragraph_count: metadata.paragraph_count,
    paragraphs_recorded: metadata.paragraphs_recorded,
    table_count: metadata.table_count,
    chart_count: metadata.chart_count,
    diagram_count: metadata.diagram_count,
    image_count: metadata.image_count,
    media_count: metadata.media_count,
    annotation_count: metadata.annotation_count,
    link_count: metadata.link_count,
    comment_count: metadata.comment_count,
    width: metadata.width,
    height: metadata.height,
    aspect_ratio: metadata.aspect_ratio,
    size_kind: deck.size_kind.clone(),
    paper_size: PaperSize::new(metadata.paper_size, metadata.width, metadata.height),
    layouts: metadata.layouts,
    sections: metadata.sections,
    has_macros: deck.has_macros,
    is_template: deck.is_template,
    is_show: deck.is_show,
    warnings: deck.warnings.clone(),
  }
}

// ── Text ──────────────────────────────────────────────────────────

/// The text of one slide.
#[napi(object, object_from_js = false)]
pub struct PageText {
  /// The slide's number, from 1.
  pub page: u32,
  pub text: String,
}

pub(crate) fn text_per_page_of(deck: &Presentation) -> Vec<PageText> {
  deck
    .text_per_page()
    .into_iter()
    .map(|(page, text)| PageText { page, text })
    .collect()
}

/// The text of one slide with what repeats on every slide set apart. `header` is the text of a
/// date, footer, slide number or header placeholder in the top half of the slide, `footer` of
/// one in the bottom half, and `body` everything else. In a deck of one slide nothing repeats,
/// so all of it is `body`.
#[napi(object, object_from_js = false)]
pub struct StructuredPageText {
  /// The slide's number, from 1.
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
}

pub(crate) fn structured_text_of(deck: &Presentation) -> Vec<StructuredPageText> {
  deck
    .structured_text()
    .into_iter()
    .map(|slide| StructuredPageText {
      page: slide.page,
      header: slide.header,
      body: slide.body,
      footer: slide.footer,
    })
    .collect()
}

// ── Slides ────────────────────────────────────────────────────────

/// Where a shape is: its box in points, measured from the slide's top-left corner, as
/// PowerPoint measures it.
#[derive(Debug, Clone, Copy)]
#[napi(object)]
pub struct Frame {
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
  /// Clockwise, in degrees. It is reported and not applied: the box is the shape's before it is
  /// turned.
  pub rotation: f64,
}

fn frame_of(frame: pptxdown::Frame) -> Frame {
  Frame {
    x: frame.x,
    y: frame.y,
    width: frame.width,
    height: frame.height,
    rotation: frame.rotation,
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum Align {
  Left,
  Center,
  Right,
  Justify,
}

/// What a placeholder is for. `object` holds content of any kind, which is what a placeholder
/// that names no kind does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "camelCase")]
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
  Object,
  SlideImage,
}

fn placeholder_of(kind: pptxdown::PlaceholderKind) -> PlaceholderKind {
  match kind {
    pptxdown::PlaceholderKind::Title => PlaceholderKind::Title,
    pptxdown::PlaceholderKind::CenteredTitle => PlaceholderKind::CenteredTitle,
    pptxdown::PlaceholderKind::Subtitle => PlaceholderKind::Subtitle,
    pptxdown::PlaceholderKind::Body => PlaceholderKind::Body,
    pptxdown::PlaceholderKind::Date => PlaceholderKind::Date,
    pptxdown::PlaceholderKind::Footer => PlaceholderKind::Footer,
    pptxdown::PlaceholderKind::SlideNumber => PlaceholderKind::SlideNumber,
    pptxdown::PlaceholderKind::Header => PlaceholderKind::Header,
    pptxdown::PlaceholderKind::Picture => PlaceholderKind::Picture,
    pptxdown::PlaceholderKind::Chart => PlaceholderKind::Chart,
    pptxdown::PlaceholderKind::Table => PlaceholderKind::Table,
    pptxdown::PlaceholderKind::Diagram => PlaceholderKind::Diagram,
    pptxdown::PlaceholderKind::Media => PlaceholderKind::Media,
    pptxdown::PlaceholderKind::ClipArt => PlaceholderKind::ClipArt,
    pptxdown::PlaceholderKind::Object => PlaceholderKind::Object,
    pptxdown::PlaceholderKind::SlideImage => PlaceholderKind::SlideImage,
  }
}

/// Where a link leads, as the two fields JavaScript sees: `uri` for an address outside the deck,
/// `dest` for a place inside it (the number of a slide, or a jump such as `nextslide`).
fn target_of(link: Option<&LinkTarget>) -> (Option<String>, Option<String>) {
  match link {
    Some(LinkTarget::External(uri)) => (Some(uri.clone()), None),
    Some(LinkTarget::Slide(number)) => (None, Some(number.to_string())),
    Some(LinkTarget::Action(action)) => (None, Some(action.clone())),
    None => (None, None),
  }
}

/// A stretch of text with one formatting, resolved through the slide's layout and master. A
/// formatting flag is present only when it is set. A line break inside a paragraph is a run
/// whose text is a line break.
#[napi(object)]
pub struct Run {
  pub text: String,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  pub underline: Option<bool>,
  pub strike: Option<bool>,
  /// In points.
  pub size: Option<f64>,
  /// The font the run names; absent when it names none of its own and takes the theme's.
  pub font: Option<String>,
  /// Where a link on the run leads: an address outside the deck.
  pub uri: Option<String>,
  /// Where a link on the run leads inside the deck: the number of a slide, or a jump such as
  /// `nextslide`.
  pub dest: Option<String>,
  /// The kind of field the text is the last saved value of: `slidenum`, `datetime1`.
  pub field: Option<String>,
}

/// A paragraph of a text body.
#[napi(object)]
pub struct Paragraph {
  /// The paragraph's own text: tabs and line breaks kept, its bullet not included.
  pub text: String,
  /// Nesting, from 0.
  pub level: u32,
  /// What stands in front of the paragraph: `•`, `2.`, `b)`. Absent when it has no bullet.
  pub bullet: Option<String>,
  /// The bullet is a number or a letter that counts.
  pub ordered: bool,
  pub align: Option<Align>,
  pub runs: Vec<Run>,
}

fn paragraphs_of(paragraphs: &[pptxdown::Paragraph]) -> Vec<Paragraph> {
  let set = |flag: bool| flag.then_some(true);
  paragraphs
    .iter()
    .map(|paragraph| Paragraph {
      text: paragraph.text(),
      level: u32::from(paragraph.level),
      bullet: paragraph.bullet.clone(),
      ordered: paragraph.ordered,
      align: paragraph.align.map(|align| match align {
        pptxdown::Align::Left => Align::Left,
        pptxdown::Align::Center => Align::Center,
        pptxdown::Align::Right => Align::Right,
        pptxdown::Align::Justify => Align::Justify,
      }),
      runs: paragraph
        .runs
        .iter()
        .map(|run| {
          let (uri, dest) = target_of(run.link.as_ref());
          Run {
            text: run.text.clone(),
            bold: set(run.bold),
            italic: set(run.italic),
            underline: set(run.underline),
            strike: set(run.strike),
            size: run.size,
            font: run.font.clone(),
            uri,
            dest,
            field: run.field.clone(),
          }
        })
        .collect(),
    })
    .collect()
}

/// A cell of a table on a slide. A cell another one is merged over is not listed: the cell that
/// covers it carries the span.
#[napi(object)]
pub struct TableCell {
  /// The cell's text, one line per paragraph.
  pub text: String,
  pub paragraphs: Vec<Paragraph>,
  /// The grid column the cell starts in, from 0.
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
}

/// One row of a table on a slide.
#[napi(object)]
pub struct TableRow {
  /// In points.
  pub height: f64,
  pub cells: Vec<TableCell>,
}

/// What sort of chart a series is drawn as. The 3-D forms are the same kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum ChartKind {
  Area,
  Bar,
  Bubble,
  Doughnut,
  Line,
  Pie,
  Radar,
  Scatter,
  Stock,
  Surface,
  Other,
}

/// One series of a chart, as the values the application last showed. A chart that mixes kinds
/// (bars with a line over them) has series of each.
#[napi(object)]
pub struct ChartSeries {
  pub kind: ChartKind,
  pub name: Option<String>,
  /// The label of each point: a category, or the x value of a scatter or bubble point as
  /// written.
  pub categories: Vec<String>,
  /// The value of each point; `null` where the file holds none.
  pub values: Vec<Option<f64>>,
}

fn series_of(chart: &pptxdown::Chart) -> Vec<ChartSeries> {
  chart
    .series
    .iter()
    .map(|series| ChartSeries {
      kind: match series.kind {
        pptxdown::ChartKind::Area => ChartKind::Area,
        pptxdown::ChartKind::Bar => ChartKind::Bar,
        pptxdown::ChartKind::Bubble => ChartKind::Bubble,
        pptxdown::ChartKind::Doughnut => ChartKind::Doughnut,
        pptxdown::ChartKind::Line => ChartKind::Line,
        pptxdown::ChartKind::Pie => ChartKind::Pie,
        pptxdown::ChartKind::Radar => ChartKind::Radar,
        pptxdown::ChartKind::Scatter => ChartKind::Scatter,
        pptxdown::ChartKind::Stock => ChartKind::Stock,
        pptxdown::ChartKind::Surface => ChartKind::Surface,
        pptxdown::ChartKind::Other => ChartKind::Other,
      },
      name: series.name.clone(),
      categories: series.categories.clone(),
      values: series.values.clone(),
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum MediaKind {
  Audio,
  Video,
}

fn media_kind_of(kind: pptxdown::MediaKind) -> MediaKind {
  match kind {
    pptxdown::MediaKind::Audio => MediaKind::Audio,
    pptxdown::MediaKind::Video => MediaKind::Video,
  }
}

/// One thing on a slide, as a union keyed on `type`.
///
/// Every shape carries the same seven fields beside its own. `frame` is its box, absent when
/// neither the shape, its layout nor its master states one. `stacking` is its place in the
/// order the file stacks shapes, back to front, from 0, which is the order a slide is stored
/// in; the shapes themselves are listed in reading order. `shapeName` is the name the file
/// gives the shape (`Title 1`), `altText` its description, and `placeholder` what it is a
/// placeholder for, when it is one. `uri` and `dest` say where a link on the whole shape leads,
/// as they do on a run.
///
/// - `text`: a text box, or a placeholder that holds text. `text` is its paragraphs as lines,
///   each with its bullet in front.
/// - `picture`: `image` is the picture's place in the deck's pictures and `name` its part
///   (`ppt/media/image1.png`), which `images()` lists by placement under the same `name`.
///   `vector` names the SVG the picture is drawn from, when the file keeps one beside the
///   raster copy. `background` is present, and true, for the slide's background picture.
/// - `table`: `columns` are the column widths in points.
/// - `chart`: the chart as data. Nothing is drawn.
/// - `diagram`: SmartArt. `shapes` is what the application drew it as, when the deck keeps that;
///   `nodes` is the text of its nodes when it does not.
/// - `media`: an audio or a video clip. `media` is the clip's place in the deck's clips and
///   `name` its part, which `media()` lists under the same `name`; `poster` names the picture
///   shown for it.
/// - `group`: shapes of its own, placed on the slide, not within the group.
#[napi(discriminant_case = "camelCase")]
pub enum Shape {
  Text {
    text: String,
    paragraphs: Vec<Paragraph>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
  Picture {
    image: u32,
    name: String,
    vector: Option<String>,
    background: Option<bool>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
  Table {
    columns: Vec<f64>,
    rows: Vec<TableRow>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
  Chart {
    title: Option<String>,
    series: Vec<ChartSeries>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
  Diagram {
    text: String,
    shapes: Vec<Shape>,
    nodes: Vec<String>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
  Media {
    media: u32,
    name: String,
    kind: MediaKind,
    mime: Option<String>,
    poster: Option<String>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
  Group {
    shapes: Vec<Shape>,
    frame: Option<Frame>,
    stacking: u32,
    shape_name: Option<String>,
    alt_text: Option<String>,
    placeholder: Option<PlaceholderKind>,
    uri: Option<String>,
    dest: Option<String>,
  },
}

/// What every shape has, as JavaScript sees it.
struct Common {
  frame: Option<Frame>,
  stacking: u32,
  shape_name: Option<String>,
  alt_text: Option<String>,
  placeholder: Option<PlaceholderKind>,
  uri: Option<String>,
  dest: Option<String>,
}

fn common_of(info: &ShapeInfo) -> Common {
  let (uri, dest) = target_of(info.link.as_ref());
  Common {
    frame: info.frame.map(frame_of),
    stacking: info.stacking,
    shape_name: info.name.clone(),
    alt_text: info.alt_text.clone(),
    placeholder: info
      .placeholder
      .map(|placeholder| placeholder_of(placeholder.kind)),
    uri,
    dest,
  }
}

/// The name of one of the deck's pictures: its part, or the address of a linked one.
fn image_name(deck: &Presentation, image: u32) -> Option<String> {
  let at = usize::try_from(image).ok()?;
  deck.images.get(at).map(|image| image.name.clone())
}

fn shapes_of(deck: &Presentation, shapes: &[pptxdown::Shape]) -> Vec<Shape> {
  shapes
    .iter()
    .map(|shape| {
      let Common {
        frame,
        stacking,
        shape_name,
        alt_text,
        placeholder,
        uri,
        dest,
      } = common_of(shape.info());
      match shape {
        pptxdown::Shape::Text(text) => Shape::Text {
          text: shape.text(),
          paragraphs: paragraphs_of(&text.paragraphs),
          frame,
          stacking,
          shape_name,
          alt_text,
          placeholder,
          uri,
          dest,
        },
        pptxdown::Shape::Picture(picture) => Shape::Picture {
          image: picture.image,
          name: image_name(deck, picture.image).unwrap_or_default(),
          vector: picture.vector.and_then(|vector| image_name(deck, vector)),
          background: picture.background.then_some(true),
          frame,
          stacking,
          shape_name,
          alt_text,
          placeholder,
          uri,
          dest,
        },
        pptxdown::Shape::Table(table) => Shape::Table {
          columns: table.columns.clone(),
          rows: table
            .rows
            .iter()
            .map(|row| TableRow {
              height: row.height,
              cells: row
                .cells
                .iter()
                .map(|cell| TableCell {
                  text: cell.lines(),
                  paragraphs: paragraphs_of(&cell.paragraphs),
                  column: cell.column,
                  col_span: cell.col_span,
                  row_span: cell.row_span,
                })
                .collect(),
            })
            .collect(),
          frame,
          stacking,
          shape_name,
          alt_text,
          placeholder,
          uri,
          dest,
        },
        pptxdown::Shape::Chart(chart) => Shape::Chart {
          title: chart.chart.title.clone(),
          series: series_of(&chart.chart),
          frame,
          stacking,
          shape_name,
          alt_text,
          placeholder,
          uri,
          dest,
        },
        pptxdown::Shape::Diagram(diagram) => Shape::Diagram {
          text: shape.text(),
          shapes: shapes_of(deck, &diagram.shapes),
          nodes: diagram.nodes.clone(),
          frame,
          stacking,
          shape_name,
          alt_text,
          placeholder,
          uri,
          dest,
        },
        pptxdown::Shape::Media(clip) => {
          let media = usize::try_from(clip.media)
            .ok()
            .and_then(|at| deck.media.get(at));
          Shape::Media {
            media: clip.media,
            name: media.map(|media| media.name.clone()).unwrap_or_default(),
            kind: media.map_or(MediaKind::Video, |media| media_kind_of(media.kind)),
            mime: media.and_then(|media| media.mime.clone()),
            poster: clip.poster.and_then(|poster| image_name(deck, poster)),
            frame,
            stacking,
            shape_name,
            alt_text,
            placeholder,
            uri,
            dest,
          }
        }
        pptxdown::Shape::Group(group) => Shape::Group {
          shapes: shapes_of(deck, &group.shapes),
          frame,
          stacking,
          shape_name,
          alt_text,
          placeholder,
          uri,
          dest,
        },
      }
    })
    .collect()
}

/// One slide of a deck.
#[napi(object, object_from_js = false)]
pub struct PptxSlide {
  /// The slide's number, from 1, in the order of the deck, hidden slides counted: the `page`
  /// every other reader names.
  pub page: u32,
  /// The slide's part in the package: `ppt/slides/slide3.xml`.
  pub name: String,
  /// The text of the title placeholder.
  pub title: Option<String>,
  /// The name of the layout the slide is built on: `Title and Content`.
  pub layout: Option<String>,
  /// The section of the deck the slide is in.
  pub section: Option<String>,
  /// The slide is left out when the deck is shown.
  pub hidden: bool,
  /// The slide's text, in reading order, without what repeats on every slide.
  pub text: String,
  /// In reading order: the title, then top to bottom and left to right.
  pub shapes: Vec<Shape>,
  /// The speaker notes, one line per paragraph; absent when the slide has none.
  pub notes: Option<String>,
}

pub(crate) fn slides_of(deck: &Presentation) -> Vec<PptxSlide> {
  deck
    .slides
    .iter()
    .map(|slide| PptxSlide {
      page: slide.number,
      name: slide.name.clone(),
      title: slide.title.clone(),
      layout: slide.layout.clone(),
      section: slide.section.clone(),
      hidden: slide.hidden,
      text: slide.text(),
      shapes: shapes_of(deck, &slide.shapes),
      notes: (!slide.notes.is_empty()).then(|| slide.notes_text()),
    })
    .collect()
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
pub struct PptxTableCell {
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
pub struct PptxTable {
  /// The table's place among the deck's tables, in reading order, from 0.
  pub index: u32,
  /// The number of the slide the table is on, from 1.
  pub page: u32,
  /// Where the table is on the slide.
  pub frame: Option<Frame>,
  /// Column widths in points.
  pub columns: Vec<f64>,
  pub row_count: u32,
  pub column_count: u32,
  /// The ranges merged cells cover, in A1 form: `A1:B2`.
  pub merges: Vec<String>,
  /// Keyed by row number from 1 (`"2"`), then by column letters (`"B"`), each in table order.
  #[napi(ts_type = "Record<string, Record<string, PptxTableCell>>")]
  pub rows: BTreeMap<String, BTreeMap<ColumnKey, PptxTableCell>>,
}

pub(crate) fn tables_of(deck: &Presentation) -> Vec<PptxTable> {
  deck
    .tables()
    .into_iter()
    .map(|table| {
      let mut rows: BTreeMap<String, BTreeMap<ColumnKey, PptxTableCell>> = BTreeMap::new();
      for cell in table.cells {
        rows
          .entry((u64::from(cell.row) + 1).to_string())
          .or_default()
          .insert(
            ColumnKey(column_letters(cell.column)),
            PptxTableCell {
              reference: pptxdown::cell_ref(cell.row, cell.column),
              row: cell.row,
              column: cell.column,
              col_span: cell.col_span,
              row_span: cell.row_span,
              text: cell.text,
            },
          );
      }
      PptxTable {
        index: table.index,
        page: table.page,
        frame: table.frame.map(frame_of),
        columns: table.columns,
        row_count: table.row_count,
        column_count: table.column_count,
        merges: table.merges,
        rows,
      }
    })
    .collect()
}

// ── Pictures ──────────────────────────────────────────────────────

/// The SVG a picture is drawn from.
#[napi(object, object_from_js = false)]
pub struct PptxVector {
  /// The part's path in the package: `ppt/media/image2.svg`.
  pub name: String,
  pub mime: Option<String>,
  /// The bytes as the deck stores them.
  pub data: Buffer,
}

/// A picture where it is placed: one entry for each place a picture is used.
#[napi(object, object_from_js = false)]
pub struct PptxImage {
  /// The number of the slide the picture is on, from 1.
  pub page: u32,
  /// Its place among the pictures of the same slide, in reading order, from 0.
  pub image_index: u32,
  /// The part's path in the package (`ppt/media/image1.png`), or the address of a linked
  /// picture.
  pub name: String,
  /// The format, read from the bytes themselves; absent for a linked picture or a format that is
  /// not recognized.
  pub mime: Option<String>,
  /// Pixels, read from the picture's own header; absent for vector formats.
  pub width: Option<u32>,
  pub height: Option<u32>,
  /// The bytes as the deck stores them, not re-encoded. Empty for a linked picture, which lives
  /// outside the file.
  pub data: Buffer,
  /// The picture is linked to, not embedded.
  pub linked: bool,
  /// Where the picture is on the slide, and so the size it is shown at.
  pub frame: Option<Frame>,
  pub alt_text: Option<String>,
  /// The picture is the slide's background, behind every shape.
  pub background: bool,
  /// The SVG the picture is drawn from, when the file keeps one beside the raster copy that
  /// `data` is.
  pub vector: Option<PptxVector>,
}

/// A [`PptxVector`] as it is read.
pub struct VectorRead {
  name: String,
  mime: Option<String>,
  data: Vec<u8>,
}

/// A [`PptxImage`] as it is read.
pub struct ImageRead {
  page: u32,
  image_index: u32,
  name: String,
  mime: Option<String>,
  width: Option<u32>,
  height: Option<u32>,
  data: Vec<u8>,
  linked: bool,
  frame: Option<Frame>,
  alt_text: Option<String>,
  background: bool,
  vector: Option<VectorRead>,
}

impl From<ImageRead> for PptxImage {
  fn from(read: ImageRead) -> Self {
    PptxImage {
      page: read.page,
      image_index: read.image_index,
      name: read.name,
      mime: read.mime,
      width: read.width,
      height: read.height,
      data: read.data.into(),
      linked: read.linked,
      frame: read.frame,
      alt_text: read.alt_text,
      background: read.background,
      vector: read.vector.map(|vector| PptxVector {
        name: vector.name,
        mime: vector.mime,
        data: vector.data.into(),
      }),
    }
  }
}

pub(crate) fn images_read(deck: &Presentation) -> Vec<ImageRead> {
  let image = |at: u32| deck.images.get(usize::try_from(at).ok()?);
  deck
    .placements()
    .into_iter()
    .filter_map(|placement| {
      let stored = image(placement.image)?;
      Some(ImageRead {
        page: placement.page,
        image_index: placement.image_index,
        name: stored.name.clone(),
        mime: stored.mime.map(str::to_owned),
        width: stored.width,
        height: stored.height,
        data: stored.data.clone(),
        linked: stored.linked,
        frame: placement.frame.map(frame_of),
        alt_text: placement.alt_text,
        background: placement.background,
        vector: placement.vector.and_then(image).map(|vector| VectorRead {
          name: vector.name.clone(),
          mime: vector.mime.map(str::to_owned),
          data: vector.data.clone(),
        }),
      })
    })
    .collect()
}

pub(crate) fn images_js(read: Vec<ImageRead>) -> Vec<PptxImage> {
  read.into_iter().map(Into::into).collect()
}

// ── Annotations ───────────────────────────────────────────────────

/// Where on a slide's page something sits: on the slide, or in its speaker notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum = "lowercase")]
pub enum PartKind {
  Slide,
  Notes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum AnnotationSubtype {
  Link,
  Comment,
}

/// A hyperlink or a comment.
#[napi(object, object_from_js = false)]
pub struct PptxAnnotation {
  /// The number of the slide it is on, from 1.
  pub page: u32,
  pub part: PartKind,
  pub subtype: AnnotationSubtype,
  /// The box of the shape a link is in, as `[x0, y0, x1, y1]` in the page frame `layout()` uses:
  /// points, y up, the slide's bottom-left corner at the origin. Absent for a comment, for a
  /// link in the notes, and for a shape that states no box.
  pub rect: Option<Vec<f64>>,
  /// Where an external link leads.
  pub uri: Option<String>,
  /// Where a link inside the deck leads: the number of a slide, or a jump such as `nextslide`.
  pub dest: Option<String>,
  /// What a comment says.
  pub content: Option<String>,
  /// The text the link covers. Empty for a comment, which is attached to a slide.
  pub text: String,
  /// Who wrote a comment, and when, as the file writes the date.
  pub author: Option<String>,
  pub date: Option<String>,
}

pub(crate) fn annotations_of(deck: &Presentation) -> Vec<PptxAnnotation> {
  deck
    .annotations()
    .into_iter()
    .map(|annotation| PptxAnnotation {
      page: annotation.page,
      part: match annotation.part {
        pptxdown::PartKind::Slide => PartKind::Slide,
        pptxdown::PartKind::Notes => PartKind::Notes,
      },
      subtype: match annotation.subtype {
        pptxdown::AnnotationKind::Link => AnnotationSubtype::Link,
        pptxdown::AnnotationKind::Comment => AnnotationSubtype::Comment,
      },
      rect: annotation.rect.map(|frame| {
        vec![
          frame.x,
          deck.height - (frame.y + frame.height),
          frame.x + frame.width,
          deck.height - frame.y,
        ]
      }),
      uri: annotation.uri,
      dest: annotation.dest,
      content: annotation.content,
      text: annotation.text,
      author: annotation.author,
      date: annotation.date,
    })
    .collect()
}

// ── Notes, charts and clips ───────────────────────────────────────

/// The speaker notes of one slide.
#[napi(object, object_from_js = false)]
pub struct PptxNote {
  /// The number of the slide the notes belong to, from 1.
  pub page: u32,
  /// The notes' text, one line per paragraph.
  pub text: String,
  pub paragraphs: Vec<Paragraph>,
}

pub(crate) fn notes_of(deck: &Presentation) -> Vec<PptxNote> {
  deck
    .slides
    .iter()
    .filter(|slide| !slide.notes.is_empty())
    .map(|slide| PptxNote {
      page: slide.number,
      text: slide.notes_text(),
      paragraphs: paragraphs_of(&slide.notes),
    })
    .collect()
}

/// A chart on a slide, as data: the values the application last showed. Nothing is drawn, and
/// the workbook kept behind the chart is not opened.
#[napi(object, object_from_js = false)]
pub struct PptxChart {
  /// The chart's place among the deck's charts, in reading order, from 0.
  pub index: u32,
  /// The number of the slide the chart is on, from 1.
  pub page: u32,
  /// Where the chart is on the slide.
  pub frame: Option<Frame>,
  pub title: Option<String>,
  pub series: Vec<ChartSeries>,
}

pub(crate) fn charts_of(deck: &Presentation) -> Vec<PptxChart> {
  deck
    .charts()
    .into_iter()
    .zip(0u32..)
    .map(|(used, index)| PptxChart {
      index,
      page: used.page,
      frame: used.info.frame.map(frame_of),
      title: used.chart.title.clone(),
      series: series_of(used.chart),
    })
    .collect()
}

/// An audio or a video clip where it is placed: one entry for each place a clip is used.
#[napi(object, object_from_js = false)]
pub struct PptxMedia {
  /// The number of the slide the clip is on, from 1.
  pub page: u32,
  /// The part's path in the package (`ppt/media/media1.mp4`), or the address of a linked clip.
  pub name: String,
  pub kind: MediaKind,
  /// As the package types the part: `video/mp4`.
  pub mime: Option<String>,
  /// The bytes as the deck stores them. Empty for a linked clip, which lives outside the file.
  pub data: Buffer,
  /// The clip is linked to, not embedded.
  pub linked: bool,
  /// Where the clip is on the slide.
  pub frame: Option<Frame>,
  /// The picture shown for the clip, by the `name` the deck's pictures go by.
  pub poster: Option<String>,
}

/// A [`PptxMedia`] as it is read.
pub struct MediaRead {
  page: u32,
  name: String,
  kind: MediaKind,
  mime: Option<String>,
  data: Vec<u8>,
  linked: bool,
  frame: Option<Frame>,
  poster: Option<String>,
}

impl From<MediaRead> for PptxMedia {
  fn from(read: MediaRead) -> Self {
    PptxMedia {
      page: read.page,
      name: read.name,
      kind: read.kind,
      mime: read.mime,
      data: read.data.into(),
      linked: read.linked,
      frame: read.frame,
      poster: read.poster,
    }
  }
}

pub(crate) fn media_read(deck: &Presentation) -> Vec<MediaRead> {
  deck
    .clips()
    .into_iter()
    .filter_map(|clip| {
      let stored = deck.media.get(usize::try_from(clip.media).ok()?)?;
      Some(MediaRead {
        page: clip.page,
        name: stored.name.clone(),
        kind: media_kind_of(stored.kind),
        mime: stored.mime.clone(),
        data: stored.data.clone(),
        linked: stored.linked,
        frame: clip.frame.map(frame_of),
        poster: clip.poster.and_then(|poster| image_name(deck, poster)),
      })
    })
    .collect()
}

pub(crate) fn media_js(read: Vec<MediaRead>) -> Vec<PptxMedia> {
  read.into_iter().map(Into::into).collect()
}

// ── The whole deck ────────────────────────────────────────────────

/// Everything read from a deck, in one object: what each reader returns under the reader's own
/// name. The page layout and the invoice fields, which are estimates, are left to `layout()` and
/// `invoice()`.
#[napi(object, object_from_js = false)]
pub struct PptxDocument {
  pub metadata: PptxMeta,
  pub text: String,
  pub text_per_page: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub markdown: String,
  pub slides: Vec<PptxSlide>,
  pub tables: Vec<PptxTable>,
  pub images: Vec<PptxImage>,
  pub annotations: Vec<PptxAnnotation>,
  pub notes: Vec<PptxNote>,
  pub charts: Vec<PptxChart>,
  pub media: Vec<PptxMedia>,
}

/// A [`PptxDocument`] as it is read.
pub struct DocumentRead {
  metadata: PptxMeta,
  text: String,
  text_per_page: Vec<PageText>,
  structured_text: Vec<StructuredPageText>,
  markdown: String,
  slides: Vec<PptxSlide>,
  tables: Vec<PptxTable>,
  images: Vec<ImageRead>,
  annotations: Vec<PptxAnnotation>,
  notes: Vec<PptxNote>,
  charts: Vec<PptxChart>,
  media: Vec<MediaRead>,
}

pub(crate) fn document_read(deck: &Presentation) -> DocumentRead {
  DocumentRead {
    metadata: metadata_of(deck),
    text: deck.text(),
    text_per_page: text_per_page_of(deck),
    structured_text: structured_text_of(deck),
    markdown: deck.markdown(),
    slides: slides_of(deck),
    tables: tables_of(deck),
    images: images_read(deck),
    annotations: annotations_of(deck),
    notes: notes_of(deck),
    charts: charts_of(deck),
    media: media_read(deck),
  }
}

pub(crate) fn document_js(read: DocumentRead) -> PptxDocument {
  PptxDocument {
    metadata: read.metadata,
    text: read.text,
    text_per_page: read.text_per_page,
    structured_text: read.structured_text,
    markdown: read.markdown,
    slides: read.slides,
    tables: read.tables,
    images: images_js(read.images),
    annotations: read.annotations,
    notes: read.notes,
    charts: read.charts,
    media: media_js(read.media),
  }
}

#[cfg(test)]
mod tests;
