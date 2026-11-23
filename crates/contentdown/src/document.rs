//! A document as a flow of text: paragraphs with how each run is set, the rows of tables,
//! the paper, and what runs along the head and the foot of the pages. A format that records
//! a flow and not where anything lands (RTF, and a Word file of before 2007) is read into
//! this, and `pages` sets it on pages by estimate.

mod pages;

pub use pages::pages;

/// How a paragraph is lined up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
  #[default]
  Left,
  Center,
  Right,
  Justified,
}

/// A stretch of text set one way.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
  pub text: String,
  pub size: f64,
  pub bold: bool,
  pub italic: bool,
  pub font: Option<String>,
}

/// One piece of a paragraph.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
  Text(Run),
  Tab,
  LineBreak,
}

/// Where a paragraph is put on the page, when the file gives it a place of its own. Lengths
/// are in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placing {
  /// How far in from the page's left side, or from the left margin.
  pub x: f64,
  /// How far down from what `down_from` names.
  pub y: f64,
  /// How wide the paragraph is, where the file says.
  pub width: Option<f64>,
  /// `x` is measured from the page's side and not from the margin.
  pub from_page_side: bool,
  /// What `y` is measured down from.
  pub down_from: DownFrom,
}

/// What a placed paragraph's distance down the page is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownFrom {
  /// The page's top edge.
  PageTop,
  /// The top margin.
  TopMargin,
  /// Where the text has reached: the top of the block that comes next in the flow, which is
  /// the paragraph a placed one is tied to.
  Text,
}

/// A paragraph. Lengths are in points.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paragraph {
  pub inlines: Vec<Inline>,
  pub align: Align,
  pub indent_left: f64,
  pub indent_right: f64,
  /// The paragraph's own place on the page. None for a paragraph in the flow of the text.
  pub placing: Option<Placing>,
  /// The paragraph goes on in the frame the one before it is in: the file placed that one,
  /// and has not started over since.
  pub continues_frame: bool,
  /// The space the file states above the paragraph and below it, in points. Nothing where
  /// it states none.
  pub space_before: f64,
  pub space_after: f64,
}

impl Paragraph {
  /// The paragraph's text, with a tab for a tab and a line end for a line break.
  pub fn text(&self) -> String {
    let mut text = String::new();
    for inline in &self.inlines {
      match inline {
        Inline::Text(run) => text.push_str(&run.text),
        Inline::Tab => text.push('\t'),
        Inline::LineBreak => text.push('\n'),
      }
    }
    text
  }
}

/// One row of a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
  pub left: f64,
  pub edges: Vec<f64>,
  pub cells: Vec<Vec<Paragraph>>,
}

/// One block of the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
  Paragraph(Paragraph),
  Row(Row),
  PageBreak,
}

/// The paper and its margins, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
  pub width: f64,
  pub height: f64,
  pub left: f64,
  pub right: f64,
  pub top: f64,
  pub bottom: f64,
}

/// What runs along the head and the foot of the pages.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Running {
  /// The header and the footer of every page. Empty where the file gives none.
  pub header: Vec<Block>,
  pub footer: Vec<Block>,
  /// Those of the first page, where the file says its first page has its own: empty where it
  /// then gives none, so that the first page has none.
  pub first_header: Option<Vec<Block>>,
  pub first_footer: Option<Vec<Block>>,
}

/// The document's content.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
  pub blocks: Vec<Block>,
  pub paper: Paper,
  pub running: Running,
}

impl Body {
  /// The document's text: a line to a paragraph, and a line to a table's row with a tab
  /// between its cells.
  pub fn text(&self) -> String {
    let lines: Vec<String> = self
      .blocks
      .iter()
      .filter_map(|block| match block {
        Block::Paragraph(paragraph) => Some(paragraph.text()),
        Block::Row(row) => {
          let cell = |cell: &Vec<Paragraph>| {
            let paragraphs: Vec<String> = cell.iter().map(Paragraph::text).collect();
            paragraphs.join(" ")
          };
          let cells: Vec<String> = row.cells.iter().map(cell).collect();
          Some(cells.join("\t"))
        }
        Block::PageBreak => None,
      })
      .collect();
    lines.join("\n")
  }
}
