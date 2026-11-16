//! A document as Markdown: headings, lists, emphasis, links, pipe tables and footnotes.
//!
//! Markdown has no place for some of what a document holds. Underline, fonts, sizes and
//! alignment are left out; a merged cell is one filled cell with empty ones beside it; a table
//! inside a cell is flattened into that cell's text.

use crate::model::{Block, BreakKind, Cell, Document, Inline, Paragraph, Table, TextRun};

/// The emphasis Markdown can show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Emphasis {
  bold: bool,
  italic: bool,
  strike: bool,
}

impl Emphasis {
  fn of(run: &TextRun) -> Self {
    Emphasis {
      bold: run.bold,
      italic: run.italic,
      strike: run.strike,
    }
  }
}

/// Where text is being written, which decides how a line break and a `|` are spelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
  Flow,
  /// A heading, which is one line and already stands out: bold adds nothing to it.
  Heading,
  /// A pipe table's cell, which must stay on one line.
  Cell,
}

/// A hard line break inside a paragraph.
const HARD_BREAK: &str = "\\\n";

/// Writes `text` with whatever Markdown would read as markup escaped.
fn escape(text: &str, place: Place, out: &mut String) {
  let mut previous: Option<char> = None;
  let mut chars = text.chars().peekable();
  while let Some(c) = chars.next() {
    let next = chars.peek().copied();
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let special = match c {
      '\\' | '`' | '*' | '[' | ']' | '<' => true,
      '|' => place == Place::Cell,
      // An underscore inside a word is only an underscore.
      '_' => !(word(previous) && word(next)),
      '~' => previous == Some('~') || next == Some('~'),
      _ => false,
    };
    if special {
      out.push('\\');
    }
    // A line break in the text itself is white space, not a break someone asked for.
    out.push(if matches!(c, '\n' | '\r') { ' ' } else { c });
    previous = Some(c);
  }
}

/// Keeps the start of a line from reading as a heading, a quote, a list item or a rule.
fn guard_line(line: &str) -> String {
  let bytes = line.as_bytes();
  let ends = |at: usize| matches!(bytes.get(at), Some(b' ') | None);
  let escape_at = match bytes.first() {
    Some(b'#' | b'>') => Some(0),
    Some(b'-' | b'+') if ends(1) || (bytes.len() >= 3 && bytes.iter().all(|b| *b == b'-')) => {
      Some(0)
    }
    Some(b'0'..=b'9') => {
      let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
      (matches!(bytes.get(digits), Some(b'.' | b')')) && ends(digits + 1)).then_some(digits)
    }
    _ => None,
  };
  match escape_at {
    Some(at) => format!("{}\\{}", &line[..at], &line[at..]),
    None => line.to_owned(),
  }
}

struct Writer<'a> {
  document: &'a Document,
}

impl Writer<'_> {
  /// Writes a stretch of text with one emphasis. White space at its ends goes outside the
  /// markers, where Markdown needs it.
  fn span(emphasis: Emphasis, text: &str, place: Place, out: &mut String) {
    let core = text.trim();
    if core.is_empty() {
      out.push_str(text);
      return;
    }
    let lead = &text[..text.len() - text.trim_start().len()];
    let trail = &text[text.trim_end().len()..];
    let marks = match (emphasis.bold, emphasis.italic) {
      (true, true) => "***",
      (true, false) => "**",
      (false, true) => "*",
      (false, false) => "",
    };
    let strike = if emphasis.strike { "~~" } else { "" };
    out.push_str(lead);
    out.push_str(strike);
    out.push_str(marks);
    escape(core, place, out);
    out.push_str(marks);
    out.push_str(strike);
    out.push_str(trail);
  }

  fn inlines(&self, inlines: &[Inline], place: Place, out: &mut String) {
    // Runs that differ only in what Markdown cannot show are written as one.
    let mut pending: Option<(Emphasis, String)> = None;
    let flush = |pending: &mut Option<(Emphasis, String)>, out: &mut String| {
      if let Some((emphasis, text)) = pending.take() {
        Self::span(emphasis, &text, place, out);
      }
    };
    for inline in inlines {
      if let Inline::Text(run) = inline {
        let mut emphasis = Emphasis::of(run);
        emphasis.bold &= place != Place::Heading;
        match &mut pending {
          Some((current, text)) if *current == emphasis => text.push_str(&run.text),
          _ => {
            flush(&mut pending, out);
            pending = Some((emphasis, run.text.clone()));
          }
        }
        continue;
      }
      flush(&mut pending, out);
      match inline {
        Inline::Link(link) => {
          let mut label = String::new();
          self.inlines(&link.inlines, place, &mut label);
          let target = match (&link.uri, &link.dest) {
            (Some(uri), Some(dest)) => Some(format!("{uri}#{dest}")),
            (Some(uri), None) => Some(uri.clone()),
            (None, Some(dest)) => Some(format!("#{dest}")),
            (None, None) => None,
          };
          match target {
            // White space at the ends of what a link covers goes outside the link.
            Some(target) if !label.trim().is_empty() => {
              out.push_str(&label[..label.len() - label.trim_start().len()]);
              out.push('[');
              out.push_str(label.trim());
              out.push_str("](<");
              out.push_str(&target.replace('<', "%3C").replace('>', "%3E"));
              out.push_str(">)");
              out.push_str(&label[label.trim_end().len()..]);
            }
            _ => out.push_str(&label),
          }
        }
        Inline::Image(used) => {
          if let Some(image) = self.document.images.get(used.image as usize) {
            out.push_str("![");
            escape(used.alt_text.as_deref().unwrap_or(""), place, out);
            out.push_str("](<");
            out.push_str(&image.name.replace('<', "%3C").replace('>', "%3E"));
            out.push_str(">)");
          }
        }
        Inline::NoteRef(note) => {
          out.push_str(&format!("[^{}]", u64::from(*note) + 1));
        }
        Inline::Break(BreakKind::Line) => out.push_str(match place {
          Place::Flow => HARD_BREAK,
          Place::Heading => " ",
          Place::Cell => "<br>",
        }),
        Inline::Tab => out.push(' '),
        Inline::Text(_)
        | Inline::CommentRef(_)
        | Inline::Break(BreakKind::Page | BreakKind::Column) => {}
      }
    }
    flush(&mut pending, out);
  }

  /// A paragraph's text with its emphasis, and nothing in front of it.
  fn paragraph_text(&self, paragraph: &Paragraph, place: Place) -> String {
    let mut text = String::new();
    self.inlines(&paragraph.inlines, place, &mut text);
    text.trim().to_owned()
  }

  /// What stands in front of a list item, and how far it is indented.
  fn list_prefix(paragraph: &Paragraph) -> Option<String> {
    let item = paragraph.list.as_ref()?;
    let indent = "    ".repeat(usize::from(item.level));
    let marker = item.marker.as_str();
    let number = marker
      .strip_suffix(['.', ')'])
      .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
    Some(match (item.ordered, number) {
      (true, Some(number)) => format!("{indent}{number}. "),
      // A letter, a roman numeral or `1.2.` has no Markdown form: it is kept as text.
      (true, None) => {
        let mut kept = format!("{indent}- ");
        escape(marker, Place::Flow, &mut kept);
        kept.push(' ');
        kept
      }
      (false, _) => format!("{indent}- "),
    })
  }

  /// A paragraph as it stands in the flow; `None` when it holds nothing to show.
  fn paragraph(&self, paragraph: &Paragraph) -> Option<String> {
    let place = match paragraph.heading {
      Some(_) => Place::Heading,
      None => Place::Flow,
    };
    let text = self.paragraph_text(paragraph, place);
    if text.is_empty() {
      return None;
    }
    if let Some(level) = paragraph.heading {
      let marks = "#".repeat(usize::from(level.clamp(1, 6)));
      return Some(format!("{marks} {text}"));
    }
    if let Some(prefix) = Self::list_prefix(paragraph) {
      // Lines after a break stay inside the item.
      let hang = " ".repeat(prefix.len());
      return Some(format!(
        "{prefix}{}",
        text.replace(HARD_BREAK, &format!("{HARD_BREAK}{hang}"))
      ));
    }
    Some(
      text
        .split(HARD_BREAK)
        .map(|line| guard_line(line.trim_start()))
        .collect::<Vec<_>>()
        .join(HARD_BREAK),
    )
  }

  /// A cell on one line: its paragraphs separated by breaks, a table inside it flattened.
  fn cell(&self, cell: &Cell) -> String {
    let mut parts = Vec::new();
    for block in &cell.blocks {
      match block {
        Block::Paragraph(paragraph) => {
          let text = self.paragraph_text(paragraph, Place::Cell);
          if text.is_empty() {
            continue;
          }
          match paragraph
            .list
            .as_ref()
            .filter(|item| !item.marker.is_empty())
          {
            Some(item) => {
              let mut line = String::new();
              escape(&item.marker, Place::Cell, &mut line);
              parts.push(format!("{line} {text}"));
            }
            None => parts.push(text),
          }
        }
        Block::Table(inner) => {
          for row in &inner.rows {
            let cells: Vec<String> = row.cells.iter().map(|cell| self.cell(cell)).collect();
            parts.push(cells.join(" \\| "));
          }
        }
      }
    }
    parts.join("<br>")
  }

  /// A pipe table. Its first row is the header, which Markdown requires a table to have.
  fn table(&self, table: &Table) -> Option<String> {
    let columns = table
      .rows
      .iter()
      .flat_map(|row| &row.cells)
      .map(|cell| cell.column.saturating_add(cell.col_span.max(1)) as usize)
      .max()
      .unwrap_or(0)
      .max(table.columns.len());
    if table.rows.is_empty() || columns == 0 {
      return None;
    }
    let mut lines = Vec::with_capacity(table.rows.len() + 1);
    for (at, row) in table.rows.iter().enumerate() {
      let mut cells = vec![String::new(); columns];
      for cell in &row.cells {
        if let Some(slot) = cells.get_mut(cell.column as usize) {
          *slot = self.cell(cell);
        }
      }
      lines.push(format!("| {} |", cells.join(" | ")));
      if at == 0 {
        lines.push(format!("|{}", " --- |".repeat(columns)));
      }
    }
    Some(lines.join("\n"))
  }

  /// Blocks as Markdown: a blank line between them, list items on consecutive lines.
  fn blocks(&self, blocks: &[Block]) -> String {
    let mut out = String::new();
    let mut after_item = false;
    for block in blocks {
      let (text, is_item) = match block {
        Block::Paragraph(paragraph) => (
          self.paragraph(paragraph),
          paragraph.list.is_some() && paragraph.heading.is_none(),
        ),
        Block::Table(table) => (self.table(table), false),
      };
      let Some(text) = text else {
        continue;
      };
      if !out.is_empty() {
        out.push_str(if after_item && is_item { "\n" } else { "\n\n" });
      }
      out.push_str(&text);
      after_item = is_item;
    }
    out
  }
}

impl Document {
  /// The body as Markdown, with its footnotes and endnotes after it.
  pub fn markdown(&self) -> String {
    let writer = Writer { document: self };
    let mut out = writer.blocks(&self.blocks);
    for (at, note) in self.notes.iter().enumerate() {
      let text = writer.blocks(&note.blocks);
      if text.is_empty() {
        continue;
      }
      if !out.is_empty() {
        out.push_str("\n\n");
      }
      // What follows a note's first line is indented to stay part of the note.
      let lines: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(line, text)| match (line, text.is_empty()) {
          (0, _) | (_, true) => text.to_owned(),
          _ => format!("    {text}"),
        })
        .collect();
      out.push_str(&format!("[^{}]: {}", at + 1, lines.join("\n")));
    }
    out
  }
}

#[cfg(test)]
mod tests;
