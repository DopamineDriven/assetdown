//! `styles.xml`: what a paragraph or a run takes from the style it names, the styles that style
//! is based on, and the document's defaults.

use std::collections::HashMap;

use roxmltree::{Document as Xml, Node};

use crate::model::{Align, VerticalAlign};
use crate::xml::{attr, child, number, on_off, val, wml};

/// Run formatting as one place states it: `None` where that place says nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunProps {
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  pub underline: Option<bool>,
  pub strike: Option<bool>,
  pub vertical: Option<VerticalAlign>,
  /// In points.
  pub size: Option<f64>,
  pub font: Option<String>,
  /// The character style the run names.
  pub style: Option<String>,
}

impl RunProps {
  pub(crate) fn read(properties: Option<Node<'_, '_>>) -> RunProps {
    let Some(rpr) = properties else {
      return RunProps::default();
    };
    RunProps {
      bold: on_off(child(rpr, "b")),
      italic: on_off(child(rpr, "i")),
      underline: child(rpr, "u").map(|u| val(u) != Some("none")),
      strike: on_off(child(rpr, "strike")).or(on_off(child(rpr, "dstrike"))),
      vertical: child(rpr, "vertAlign").and_then(|v| match val(v) {
        Some("superscript") => Some(VerticalAlign::Superscript),
        Some("subscript") => Some(VerticalAlign::Subscript),
        Some("baseline") => Some(VerticalAlign::Baseline),
        _ => None,
      }),
      // Sizes are in half-points.
      size: number::<f64>(child(rpr, "sz").and_then(val)).map(|half| half / 2.0),
      font: child(rpr, "rFonts")
        .and_then(|fonts| {
          attr(fonts, "ascii")
            .or(attr(fonts, "hAnsi"))
            .or(attr(fonts, "cs"))
        })
        .map(str::to_owned),
      style: child(rpr, "rStyle").and_then(val).map(str::to_owned),
    }
  }

  /// `self` where it speaks, `under` where it does not.
  fn over(self, under: &RunProps) -> RunProps {
    RunProps {
      bold: self.bold.or(under.bold),
      italic: self.italic.or(under.italic),
      underline: self.underline.or(under.underline),
      strike: self.strike.or(under.strike),
      vertical: self.vertical.or(under.vertical),
      size: self.size.or(under.size),
      font: self.font.or_else(|| under.font.clone()),
      style: self.style.or_else(|| under.style.clone()),
    }
  }
}

pub(crate) fn read_align(properties: Option<Node<'_, '_>>) -> Option<Align> {
  match properties.and_then(|p| child(p, "jc")).and_then(val)? {
    "left" | "start" => Some(Align::Left),
    "center" => Some(Align::Center),
    "right" | "end" => Some(Align::Right),
    "both" | "distribute" | "justify" => Some(Align::Justify),
    _ => None,
  }
}

/// A list a paragraph belongs to: the numbering instance and the level in it.
pub(crate) fn read_list(properties: Option<Node<'_, '_>>) -> Option<(u32, u8)> {
  let list = properties.and_then(|p| child(p, "numPr"))?;
  let id = number::<u32>(child(list, "numId").and_then(val))?;
  let level = number::<u8>(child(list, "ilvl").and_then(val)).unwrap_or(0);
  Some((id, level))
}

/// An outline level as a heading number: levels 0 to 8 are headings 1 to 9, and 9 is body text.
pub(crate) fn read_heading(properties: Option<Node<'_, '_>>) -> Option<u8> {
  number::<u8>(
    properties
      .and_then(|p| child(p, "outlineLvl"))
      .and_then(val),
  )
  .filter(|level| *level < 9)
  .map(|level| level + 1)
}

#[derive(Debug, Clone, Default)]
struct Style {
  name: String,
  based_on: Option<String>,
  heading: Option<u8>,
  list: Option<(u32, u8)>,
  align: Option<Align>,
  page_break_before: Option<bool>,
  run: RunProps,
}

/// What a paragraph takes from its style and the styles under it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ParagraphStyle {
  pub name: Option<String>,
  pub heading: Option<u8>,
  pub list: Option<(u32, u8)>,
  pub align: Option<Align>,
  pub page_break_before: bool,
  pub run: RunProps,
}

#[derive(Debug, Default)]
pub(crate) struct Styles {
  styles: HashMap<String, Style>,
  /// The paragraph style a paragraph has when it names none.
  default_paragraph: Option<String>,
  /// Run formatting from `docDefaults`.
  pub defaults: RunProps,
  /// The default language of the text: `en-US`.
  pub language: Option<String>,
}

/// A style chain longer than this is a cycle or a mistake.
const MAX_CHAIN: usize = 32;

/// `heading 3` and `Heading3`, as style names and ids write it.
fn heading_from_name(name: &str) -> Option<u8> {
  let rest = name.trim().to_ascii_lowercase();
  let digits = rest.strip_prefix("heading")?.trim();
  digits.parse::<u8>().ok().filter(|n| (1..=9).contains(n))
}

impl Styles {
  pub(crate) fn parse(doc: &Xml<'_>) -> Styles {
    let root = doc.root_element();
    let mut styles = Styles::default();
    if let Some(defaults) = child(root, "docDefaults")
      .and_then(|d| child(d, "rPrDefault"))
      .and_then(|d| child(d, "rPr"))
    {
      styles.language = child(defaults, "lang").and_then(val).map(str::to_owned);
      styles.defaults = RunProps::read(Some(defaults));
    }
    for node in root.children().filter(|n| wml(*n) == Some("style")) {
      let Some(id) = attr(node, "styleId") else {
        continue;
      };
      let name = child(node, "name").and_then(val).unwrap_or(id).to_owned();
      let paragraph = child(node, "pPr");
      if attr(node, "type") == Some("paragraph")
        && matches!(attr(node, "default"), Some("1" | "true" | "on"))
      {
        styles.default_paragraph = Some(id.to_owned());
      }
      styles.styles.insert(
        id.to_owned(),
        Style {
          heading: read_heading(paragraph).or_else(|| heading_from_name(&name)),
          based_on: child(node, "basedOn").and_then(val).map(str::to_owned),
          list: read_list(paragraph),
          align: read_align(paragraph),
          page_break_before: on_off(paragraph.and_then(|p| child(p, "pageBreakBefore"))),
          run: RunProps::read(child(node, "rPr")),
          name,
        },
      );
    }
    styles
  }

  /// The style `id` names and every style under it, nearest first.
  fn chain<'a>(&'a self, id: Option<&str>) -> impl Iterator<Item = &'a Style> {
    let mut next = id.and_then(|id| self.styles.get(id));
    let mut steps = 0;
    std::iter::from_fn(move || {
      let style = next?;
      steps += 1;
      next = if steps < MAX_CHAIN {
        style
          .based_on
          .as_deref()
          .and_then(|base| self.styles.get(base))
      } else {
        None
      };
      Some(style)
    })
  }

  /// What a paragraph naming style `id` has; with no id, what the default paragraph style gives.
  pub(crate) fn paragraph(&self, id: Option<&str>) -> ParagraphStyle {
    let id = id.or(self.default_paragraph.as_deref());
    let mut resolved = ParagraphStyle::default();
    let mut page_break_before = None;
    for style in self.chain(id) {
      if resolved.name.is_none() {
        resolved.name = Some(style.name.clone());
      }
      resolved.heading = resolved.heading.or(style.heading);
      resolved.list = resolved.list.or(style.list);
      resolved.align = resolved.align.or(style.align);
      page_break_before = page_break_before.or(style.page_break_before);
      resolved.run = resolved.run.over(&style.run);
    }
    resolved.page_break_before = page_break_before.unwrap_or(false);
    resolved
  }

  /// The run formatting a character style gives.
  pub(crate) fn character(&self, id: &str) -> RunProps {
    self
      .chain(Some(id))
      .fold(RunProps::default(), |resolved, style| {
        resolved.over(&style.run)
      })
  }
}

#[cfg(test)]
mod tests;
