//! A DrawingML text body: paragraphs, their runs and fields, and what stands in front of them.
//!
//! The same body is in a shape on a slide, in a table cell, in a diagram and on a notes page.

use std::collections::HashMap;

use contentdown::counting::{letters, roman};
use ooxmldown::Relationship;
use roxmltree::Node;

use crate::inherit::{Bullet, LEVELS, LevelStyle, ListStyle};
use crate::model::{LinkTarget, Paragraph, Run};
use crate::xml::{attr, child, chosen, elements, is, is_alternate, name, number, rel_attr};

/// Alternates and math nest this deep at most inside a paragraph.
const MAX_DEPTH: u32 = 16;

/// What a link's target is found through.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Links<'a> {
  /// The relationships of the part being read, by id.
  pub rels: &'a HashMap<String, Relationship>,
  /// The number of each slide, by its part.
  pub slides: &'a HashMap<String, u32>,
}

impl Links<'_> {
  /// Where `a:hlinkClick` leads: an address, another slide, or a jump with no fixed target.
  pub(crate) fn target(&self, link: Node<'_, '_>) -> Option<LinkTarget> {
    let relationship = rel_attr(link, "id")
      .filter(|id| !id.is_empty())
      .and_then(|id| self.rels.get(id));
    if let Some(relationship) = relationship {
      if relationship.external {
        return Some(LinkTarget::External(relationship.target.clone()));
      }
      return self
        .slides
        .get(&relationship.target)
        .map(|number| LinkTarget::Slide(*number));
    }
    // `ppaction://hlinkshowjump?jump=nextslide`
    let action = attr(link, "action")?.strip_prefix("ppaction://")?;
    let jump = action.split_once("jump=").map_or(action, |(_, jump)| jump);
    (!jump.is_empty()).then(|| LinkTarget::Action(jump.to_owned()))
  }
}

/// A count written the way an automatic number's scheme says: `arabicPeriod` is `3.`,
/// `alphaLcParenR` is `c)`, `romanUcParenBoth` is `(III)`.
fn marker(scheme: &str, n: u32) -> String {
  let (count, rest) = if let Some(rest) = scheme.strip_prefix("alphaLc") {
    (letters(n), rest)
  } else if let Some(rest) = scheme.strip_prefix("alphaUc") {
    (letters(n).to_uppercase(), rest)
  } else if let Some(rest) = scheme.strip_prefix("romanLc") {
    (roman(n), rest)
  } else if let Some(rest) = scheme.strip_prefix("romanUc") {
    (roman(n).to_uppercase(), rest)
  } else {
    (
      n.to_string(),
      scheme.strip_prefix("arabic").unwrap_or("Period"),
    )
  };
  match rest {
    "ParenBoth" => format!("({count})"),
    "ParenR" => format!("{count})"),
    "Plain" => count,
    _ => format!("{count}."),
  }
}

/// A bullet character as text. One from a symbol font, or from a font's private range, means
/// nothing outside that font.
fn bullet_text(text: &str, symbol: bool) -> String {
  match text.chars().next() {
    Some(c) if !symbol && !('\u{E000}'..='\u{F8FF}').contains(&c) && !c.is_control() => {
      text.to_owned()
    }
    _ => "•".to_owned(),
  }
}

fn formatted(text: String, style: &LevelStyle) -> Run {
  Run {
    text,
    bold: style.bold.unwrap_or(false),
    italic: style.italic.unwrap_or(false),
    underline: style.underline.unwrap_or(false),
    strike: style.strike.unwrap_or(false),
    size: style.size,
    font: style.font.clone(),
    link: None,
    field: None,
  }
}

/// Adds a run, joining it to the one before it when the two differ in nothing but their text.
fn push(runs: &mut Vec<Run>, run: Run) {
  if run.text.is_empty() {
    return;
  }
  if let Some(last) = runs.last_mut()
    && last.field.is_none()
    && run.field.is_none()
    && last.bold == run.bold
    && last.italic == run.italic
    && last.underline == run.underline
    && last.strike == run.strike
    && last.size == run.size
    && last.font == run.font
    && last.link == run.link
  {
    last.text.push_str(&run.text);
    return;
  }
  runs.push(run);
}

fn read_runs(
  parent: Node<'_, '_>,
  paragraph: &LevelStyle,
  links: &Links<'_>,
  runs: &mut Vec<Run>,
  depth: u32,
) {
  if depth > MAX_DEPTH {
    return;
  }
  for item in elements(parent) {
    if is_alternate(item) {
      if let Some(branch) = chosen(item) {
        read_runs(branch, paragraph, links, runs, depth + 1);
      }
      continue;
    }
    let properties = child(item, "rPr");
    let style = LevelStyle::of_run(properties).over(paragraph);
    match name(item) {
      Some("r") => {
        let text = child(item, "t").and_then(|t| t.text()).unwrap_or("");
        let mut run = formatted(text.to_owned(), &style);
        run.link = properties
          .and_then(|p| child(p, "hlinkClick"))
          .and_then(|link| links.target(link));
        push(runs, run);
      }
      Some("br") => push(runs, formatted("\n".to_owned(), &style)),
      Some("fld") => {
        let text = child(item, "t").and_then(|t| t.text()).unwrap_or("");
        let mut run = formatted(text.to_owned(), &style);
        run.field = attr(item, "type").map(str::to_owned);
        push(runs, run);
      }
      // An equation, as the characters it is written with.
      Some("m" | "oMath" | "oMathPara") => {
        let text: String = item
          .descendants()
          .filter(|n| is(*n, "t"))
          .filter_map(|n| n.text())
          .collect();
        push(runs, formatted(text, &style));
      }
      _ => {}
    }
  }
}

/// A numbered list being counted on one level: the scheme and the start its paragraphs state,
/// and the number the next of them takes.
struct Counted {
  scheme: String,
  start: u32,
  next: u32,
}

/// The paragraphs of a text body. `list` is what each level takes from above the body: the
/// shape's own list style over the layers'.
///
/// Numbers are counted the way PowerPoint shows them. Paragraphs that follow each other on a
/// level with one scheme and one stated start are one list, which counts from that start. A
/// paragraph that states another scheme or another start begins a new list there; one with a
/// character for a bullet, or with text and no bullet, ends the list. A paragraph with nothing
/// in it shows no number, takes none, and ends nothing.
pub(crate) fn read_paragraphs(
  body: Node<'_, '_>,
  list: &ListStyle,
  links: &Links<'_>,
) -> Vec<Paragraph> {
  let mut counts: [Option<Counted>; LEVELS] = Default::default();
  let mut paragraphs = Vec::new();
  for node in elements(body).filter(|n| is(*n, "p")) {
    let properties = child(node, "pPr");
    let level = number::<usize>(properties.and_then(|p| attr(p, "lvl")))
      .unwrap_or(0)
      .min(LEVELS - 1);
    let style = LevelStyle::of_paragraph(properties).over(&list.levels[level]);
    let mut runs = Vec::new();
    read_runs(node, &style, links, &mut runs, 0);
    let has_text = runs.iter().any(|run| !run.text.trim().is_empty());

    // A deeper level starts over each time a shallower one moves on.
    for deeper in counts.iter_mut().skip(level + 1) {
      *deeper = None;
    }
    let (bullet, ordered) = if !has_text {
      (None, false)
    } else {
      match style.bullet.as_ref() {
        Some(Bullet::Char { text, symbol }) => {
          counts[level] = None;
          (Some(bullet_text(text, *symbol)), false)
        }
        Some(Bullet::Auto { scheme, start }) => {
          let n = match &counts[level] {
            Some(list) if list.scheme == *scheme && list.start == *start => list.next,
            _ => *start,
          };
          counts[level] = Some(Counted {
            scheme: scheme.clone(),
            start: *start,
            next: n.saturating_add(1),
          });
          (Some(marker(scheme, n)), true)
        }
        Some(Bullet::None) | None => {
          counts[level] = None;
          (None, false)
        }
      }
    };
    paragraphs.push(Paragraph {
      level: u8::try_from(level).unwrap_or(u8::MAX),
      bullet,
      ordered,
      align: style.align,
      runs,
    });
  }
  paragraphs
}

#[cfg(test)]
mod tests;
