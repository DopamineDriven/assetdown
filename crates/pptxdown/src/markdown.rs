//! A deck as Markdown: a heading for each slide, then what the slide holds in reading order.
//!
//! A slide's title is its heading. Body text keeps its bullets and levels as lists, a table is
//! a pipe table, a chart is a table of its data, and the speaker notes follow the slide as a
//! quote. What repeats on every slide (the date, the footer, the slide number) is left out, as
//! is a background picture.

use contentdown::markdown::{Emphasis, HARD_BREAK, Place, escape, guard_line, image, link, span};

use crate::model::{
  ChartShape, LinkTarget, MediaKind, Paragraph, Presentation, Run, Shape, Slide, Table, TableCell,
};

/// Blocks of Markdown, one under the other: a blank line between them, list items on
/// consecutive lines.
#[derive(Default)]
struct Blocks {
  out: String,
  after_item: bool,
}

impl Blocks {
  fn push(&mut self, text: &str, is_item: bool) {
    if text.is_empty() {
      return;
    }
    if !self.out.is_empty() {
      self.out.push_str(if self.after_item && is_item {
        "\n"
      } else {
        "\n\n"
      });
    }
    self.out.push_str(text);
    self.after_item = is_item;
  }
}

/// The address a link is written with; a jump with no fixed target has none.
fn address(target: &LinkTarget) -> Option<String> {
  match target {
    LinkTarget::External(uri) => Some(uri.clone()),
    LinkTarget::Slide(number) => Some(format!("#slide-{number}")),
    LinkTarget::Action(_) => None,
  }
}

/// How a line break inside a paragraph is spelled.
fn line_break(place: Place) -> &'static str {
  match place {
    Place::Flow => HARD_BREAK,
    Place::Heading => " ",
    Place::Cell => "<br>",
  }
}

/// Runs with no link among them. Runs that differ only in what Markdown cannot show are
/// written as one.
fn plain(runs: &[Run], place: Place, out: &mut String) {
  let emphasis = |run: &Run| Emphasis {
    bold: run.bold && place != Place::Heading,
    italic: run.italic,
    strike: run.strike,
  };
  let mut rest = runs;
  while let Some(first) = rest.first() {
    let same = rest
      .iter()
      .take_while(|run| emphasis(run) == emphasis(first))
      .count();
    let text: String = rest[..same].iter().map(|run| run.text.as_str()).collect();
    for (at, piece) in text.split('\n').enumerate() {
      if at > 0 {
        out.push_str(line_break(place));
      }
      span(emphasis(first), &piece.replace('\t', " "), place, out);
    }
    rest = &rest[same..];
  }
}

/// A paragraph's runs, a link around each stretch that leads somewhere.
fn inline(runs: &[Run], place: Place) -> String {
  let mut out = String::new();
  let mut rest = runs;
  while let Some(first) = rest.first() {
    let same = rest.iter().take_while(|run| run.link == first.link).count();
    let mut label = String::new();
    plain(&rest[..same], place, &mut label);
    match first.link.as_ref().and_then(address) {
      Some(address) => link(&label, &address, &mut out),
      None => out.push_str(&label),
    }
    rest = &rest[same..];
  }
  out.trim().to_owned()
}

/// What stands in front of a list item, indented by its level.
fn list_prefix(paragraph: &Paragraph) -> String {
  let indent = "    ".repeat(usize::from(paragraph.level));
  let bullet = paragraph.bullet.as_deref().unwrap_or("");
  let number = bullet
    .strip_suffix(['.', ')'])
    .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
  match (paragraph.ordered, number) {
    (true, Some(number)) => format!("{indent}{number}. "),
    // A letter or a roman numeral has no Markdown form: it is kept as text.
    (true, None) => {
      let mut kept = format!("{indent}- ");
      escape(bullet, Place::Flow, &mut kept);
      kept.push(' ');
      kept
    }
    (false, _) => format!("{indent}- "),
  }
}

/// The paragraphs of a text box as blocks: a bulleted or indented one as a list item.
fn paragraphs(listed: &[Paragraph], blocks: &mut Blocks) {
  for paragraph in listed {
    let text = inline(&paragraph.runs, Place::Flow);
    if text.is_empty() {
      continue;
    }
    if paragraph.bullet.is_some() || paragraph.level > 0 {
      let prefix = list_prefix(paragraph);
      // Lines after a break stay inside the item.
      let hang = format!("{HARD_BREAK}{}", " ".repeat(prefix.len()));
      blocks.push(
        &format!("{prefix}{}", text.replace(HARD_BREAK, &hang)),
        true,
      );
    } else {
      let guarded: Vec<String> = text
        .split(HARD_BREAK)
        .map(|line| guard_line(line.trim_start()))
        .collect();
      blocks.push(&guarded.join(HARD_BREAK), false);
    }
  }
}

/// A cell on one line: its paragraphs separated by breaks.
fn cell(cell: &TableCell) -> String {
  cell
    .paragraphs
    .iter()
    .filter_map(|paragraph| {
      let text = inline(&paragraph.runs, Place::Cell);
      if text.is_empty() {
        return None;
      }
      Some(
        match paragraph.bullet.as_deref().filter(|b| !b.is_empty()) {
          Some(bullet) => {
            let mut line = String::new();
            escape(bullet, Place::Cell, &mut line);
            format!("{line} {text}")
          }
          None => text,
        },
      )
    })
    .collect::<Vec<_>>()
    .join("<br>")
}

fn row(cells: &[String]) -> String {
  format!("| {} |", cells.join(" | "))
}

fn rule(columns: usize) -> String {
  format!("|{}", " --- |".repeat(columns))
}

/// A pipe table. Its first row is the header, which Markdown requires a table to have; a
/// merged cell is one filled cell with empty ones beside it.
fn table(table: &Table) -> String {
  let columns = table
    .rows
    .iter()
    .flat_map(|row| &row.cells)
    .map(|cell| usize::try_from(cell.column.saturating_add(cell.col_span.max(1))).unwrap_or(0))
    .max()
    .unwrap_or(0)
    .max(table.columns.len());
  if table.rows.is_empty() || columns == 0 {
    return String::new();
  }
  let mut lines = Vec::with_capacity(table.rows.len() + 1);
  for (at, listed) in table.rows.iter().enumerate() {
    let mut cells = vec![String::new(); columns];
    for found in &listed.cells {
      if let Some(slot) = usize::try_from(found.column)
        .ok()
        .and_then(|column| cells.get_mut(column))
      {
        *slot = cell(found);
      }
    }
    lines.push(row(&cells));
    if at == 0 {
      lines.push(rule(columns));
    }
  }
  lines.join("\n")
}

/// A chart as a table of its data: a row for each category, a column for each series.
fn chart(chart: &ChartShape, blocks: &mut Blocks) {
  let chart = &chart.chart;
  let mut title = String::new();
  escape(
    chart.title.as_deref().unwrap_or("Chart"),
    Place::Flow,
    &mut title,
  );
  blocks.push(&format!("**{title}**"), false);
  let points = chart
    .series
    .iter()
    .map(|series| series.values.len().max(series.categories.len()))
    .max()
    .unwrap_or(0);
  if points == 0 {
    return;
  }
  let escaped = |text: &str| {
    let mut out = String::new();
    escape(text, Place::Cell, &mut out);
    out
  };
  let mut header = vec![String::new()];
  header.extend(chart.series.iter().zip(1usize..).map(|(series, number)| {
    series
      .name
      .as_deref()
      .map_or_else(|| format!("Series {number}"), escaped)
  }));
  let mut lines = vec![row(&header), rule(header.len())];
  for point in 0..points {
    // The label of a point is the first one any series gives it.
    let label = chart.series.iter().find_map(|series| {
      series
        .categories
        .get(point)
        .filter(|label| !label.is_empty())
    });
    let mut cells = vec![label.map_or_else(String::new, |label| escaped(label))];
    cells.extend(chart.series.iter().map(|series| {
      series
        .values
        .get(point)
        .copied()
        .flatten()
        .map_or_else(String::new, |value| value.to_string())
    }));
    lines.push(row(&cells));
  }
  blocks.push(&lines.join("\n"), false);
}

struct Writer<'a> {
  deck: &'a Presentation,
}

impl Writer<'_> {
  fn shape(&self, shape: &Shape, blocks: &mut Blocks) {
    match shape {
      Shape::Text(shape) => paragraphs(&shape.paragraphs, blocks),
      Shape::Table(found) => blocks.push(&table(found), false),
      Shape::Chart(found) => chart(found, blocks),
      Shape::Picture(picture) => {
        if let Some(stored) = self.deck.images.get(picture.image as usize)
          && !picture.background
        {
          let mut out = String::new();
          image(
            picture.info.alt_text.as_deref().unwrap_or(""),
            &stored.name,
            Place::Flow,
            &mut out,
          );
          blocks.push(&out, false);
        }
      }
      Shape::Media(clip) => {
        if let Some(stored) = self.deck.media.get(clip.media as usize) {
          let label = match stored.kind {
            MediaKind::Audio => "audio",
            MediaKind::Video => "video",
          };
          let mut out = String::new();
          link(label, &stored.name, &mut out);
          blocks.push(&out, false);
        }
      }
      Shape::Group(group) => {
        for inner in &group.shapes {
          self.shape(inner, blocks);
        }
      }
      Shape::Diagram(diagram) => {
        for inner in &diagram.shapes {
          self.shape(inner, blocks);
        }
        for node in &diagram.nodes {
          let mut item = "- ".to_owned();
          escape(&node.replace('\n', " "), Place::Flow, &mut item);
          blocks.push(&item, true);
        }
      }
    }
  }

  fn slide(&self, slide: &Slide) -> String {
    let mut blocks = Blocks::default();
    let mut heading = "## ".to_owned();
    match &slide.title {
      Some(title) => escape(title, Place::Heading, &mut heading),
      None => heading.push_str(&format!("Slide {}", slide.number)),
    }
    blocks.push(&heading, false);
    let is_title = |shape: &Shape| {
      shape
        .info()
        .placeholder
        .is_some_and(|placeholder| placeholder.kind.is_title())
    };
    for shape in &slide.shapes {
      if !is_title(shape) && !shape.is_furniture() {
        self.shape(shape, &mut blocks);
      }
    }
    // The notes follow the slide as a quote, a paragraph to a line.
    let notes: Vec<String> = slide
      .notes
      .iter()
      .map(|paragraph| inline(&paragraph.runs, Place::Flow).replace(HARD_BREAK, " "))
      .filter(|line| !line.is_empty())
      .collect();
    if !notes.is_empty() {
      blocks.push(&format!("> **Notes:** {}", notes.join("\n>\n> ")), false);
    }
    blocks.out
  }
}

impl Presentation {
  /// The deck as Markdown, slide by slide.
  pub fn markdown(&self) -> String {
    let writer = Writer { deck: self };
    self
      .slides
      .iter()
      .map(|slide| writer.slide(slide))
      .collect::<Vec<_>>()
      .join("\n\n")
  }
}

#[cfg(test)]
mod tests;
