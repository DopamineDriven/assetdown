//! What a deck is, in numbers: its slide size and the paper it is nearest to, and counts of what
//! it holds.

use contentdown::{PaperSize, paper_size};

use crate::lists::{AnnotationKind, each_shape};
use crate::model::{Paragraph, Presentation, Shape};

/// Counts and page facts. What the deck says about itself (title, author, dates) is in
/// [`Presentation::props`].
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
  /// The slides the deck lists, hidden ones counted.
  pub slide_count: u32,
  /// The count the application recorded when it saved the file.
  pub slides_recorded: Option<u32>,
  pub hidden_slide_count: u32,
  /// Slides that have speaker notes.
  pub notes_count: u32,
  /// Words on the slides, counted from the text [`Presentation::text`] returns, bullets aside.
  /// Speaker notes are not counted, and neither are the dates, footers and slide numbers that
  /// repeat on every slide.
  pub word_count: u32,
  pub words_recorded: Option<u32>,
  /// Characters on the slides that are not white space.
  pub character_count: u32,
  /// Paragraphs on the slides that hold text.
  pub paragraph_count: u32,
  pub paragraphs_recorded: Option<u32>,
  pub table_count: u32,
  pub chart_count: u32,
  pub diagram_count: u32,
  /// Places a picture is used: the length of [`Presentation::placements`].
  pub image_count: u32,
  /// Places a clip is used: the length of [`Presentation::clips`].
  pub media_count: u32,
  /// Links and comments: the length of [`Presentation::annotations`].
  pub annotation_count: u32,
  pub link_count: u32,
  pub comment_count: u32,
  /// The size of a slide, in points.
  pub width: f64,
  pub height: f64,
  /// The slide's shape in the usual words: `16:9`, `4:3`.
  pub aspect_ratio: String,
  /// The standard paper size nearest to the slide; unknown for a slide sized for a screen.
  pub paper_size: PaperSize,
  /// The names of the layouts the slides are built on, each once, in the order they are met.
  pub layouts: Vec<String>,
  /// The names of the deck's sections, in order.
  pub sections: Vec<String>,
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

/// The shape of a slide as a ratio people use, when it is near one; else as its own two sides.
fn aspect_ratio(width: f64, height: f64) -> String {
  const USUAL: [(f64, f64); 6] = [
    (16.0, 9.0),
    (4.0, 3.0),
    (16.0, 10.0),
    (3.0, 2.0),
    (1.0, 1.0),
    (3.0, 4.0),
  ];
  if !(width > 0.0 && height > 0.0) {
    return String::new();
  }
  let ratio = width / height;
  USUAL
    .iter()
    .find(|(w, h)| (ratio - w / h).abs() / (w / h) < 0.01)
    .map_or_else(
      || format!("{}:{}", width.round(), height.round()),
      |(w, h)| format!("{w}:{h}"),
    )
}

/// A count of text as it is met.
#[derive(Default)]
struct Tally {
  words: usize,
  characters: usize,
  paragraphs: usize,
}

impl Tally {
  /// One paragraph's text, or one line of a diagram's node. One with nothing on it is not
  /// counted.
  fn line(&mut self, text: &str) {
    if text.trim().is_empty() {
      return;
    }
    self.paragraphs += 1;
    self.words += text.split_whitespace().count();
    self.characters += text.chars().filter(|c| !c.is_whitespace()).count();
  }

  fn paragraphs(&mut self, listed: &[Paragraph]) {
    for paragraph in listed {
      self.line(&paragraph.text());
    }
  }
}

impl Presentation {
  pub fn metadata(&self) -> Metadata {
    let mut tally = Tally::default();
    let (mut tables, mut charts, mut diagrams) = (0, 0, 0);
    let mut layouts = Vec::new();
    let mut sections = Vec::new();
    for slide in &self.slides {
      // What repeats on every slide is a placeholder of the slide itself, not of a group.
      for shown in slide.shapes.iter().filter(|shape| !shape.is_furniture()) {
        each_shape(std::slice::from_ref(shown), &mut |shape| match shape {
          Shape::Text(shape) => tally.paragraphs(&shape.paragraphs),
          Shape::Table(table) => {
            tables += 1;
            for cell in table.rows.iter().flat_map(|row| &row.cells) {
              tally.paragraphs(&cell.paragraphs);
            }
          }
          Shape::Chart(_) => charts += 1,
          Shape::Diagram(diagram) => {
            diagrams += 1;
            // A diagram with no drawing has only the text of its nodes.
            if diagram.shapes.is_empty() {
              for line in diagram.nodes.iter().flat_map(|node| node.lines()) {
                tally.line(line);
              }
            }
          }
          Shape::Picture(_) | Shape::Media(_) | Shape::Group(_) => {}
        });
      }
      for (name, seen) in [
        (&slide.layout, &mut layouts),
        (&slide.section, &mut sections),
      ] {
        if let Some(name) = name
          && !seen.contains(name)
        {
          seen.push(name.clone());
        }
      }
    }
    let annotations = self.annotations();
    let links = annotations
      .iter()
      .filter(|annotation| annotation.subtype == AnnotationKind::Link)
      .count();
    Metadata {
      slide_count: self.slide_count,
      slides_recorded: self.props.slides,
      hidden_slide_count: count(self.slides.iter().filter(|slide| slide.hidden).count()),
      notes_count: count(
        self
          .slides
          .iter()
          .filter(|slide| !slide.notes.is_empty())
          .count(),
      ),
      word_count: count(tally.words),
      words_recorded: self.props.words,
      character_count: count(tally.characters),
      paragraph_count: count(tally.paragraphs),
      paragraphs_recorded: self.props.paragraphs,
      table_count: count(tables),
      chart_count: count(charts),
      diagram_count: count(diagrams),
      image_count: count(self.placements().len()),
      media_count: count(self.clips().len()),
      annotation_count: count(annotations.len()),
      link_count: count(links),
      comment_count: count(annotations.len() - links),
      width: self.width,
      height: self.height,
      aspect_ratio: aspect_ratio(self.width, self.height),
      paper_size: paper_size(self.width, self.height),
      layouts,
      sections,
    }
  }
}
