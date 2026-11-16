//! `numbering.xml`: what stands in front of a list paragraph.
//!
//! A paragraph names a numbering instance and a level. The instance points at a definition that
//! says, level by level, how to count (a format and a start) and how to write the count (a
//! pattern such as `%1.%2.`). The marker is that pattern with the running counts filled in.

use std::collections::{HashMap, HashSet};

use contentdown::counting::{letters, roman};
use roxmltree::{Document as Xml, Node};

use crate::model::ListItem;
use crate::xml::{attr, child, number, val, wml};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberFormat {
  Bullet,
  Decimal,
  DecimalZero,
  LowerLetter,
  UpperLetter,
  LowerRoman,
  UpperRoman,
  /// The level shows no number.
  None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Level {
  start: u32,
  format: NumberFormat,
  /// The pattern: `%1.`, `%1.%2`, or the bullet character.
  text: String,
}

#[derive(Debug, Default)]
struct Instance {
  definition: u32,
  /// Levels this instance redefines or restarts.
  overrides: HashMap<u8, (Option<u32>, Option<Level>)>,
}

#[derive(Debug, Default)]
pub(crate) struct Numbering {
  definitions: HashMap<u32, HashMap<u8, Level>>,
  instances: HashMap<u32, Instance>,
}

/// Lists nest nine deep at most.
const LEVELS: usize = 9;

fn read_level(node: Node<'_, '_>) -> Level {
  let format = match child(node, "numFmt").and_then(val) {
    Some("bullet") => NumberFormat::Bullet,
    Some("decimalZero") => NumberFormat::DecimalZero,
    Some("lowerLetter") => NumberFormat::LowerLetter,
    Some("upperLetter") => NumberFormat::UpperLetter,
    Some("lowerRoman") => NumberFormat::LowerRoman,
    Some("upperRoman") => NumberFormat::UpperRoman,
    Some("none") => NumberFormat::None,
    _ => NumberFormat::Decimal,
  };
  Level {
    start: number::<u32>(child(node, "start").and_then(val)).unwrap_or(1),
    format,
    text: child(node, "lvlText")
      .and_then(val)
      .unwrap_or("")
      .to_owned(),
  }
}

fn write_count(format: NumberFormat, n: u32) -> String {
  match format {
    NumberFormat::Decimal => n.to_string(),
    NumberFormat::DecimalZero => format!("{n:02}"),
    NumberFormat::LowerLetter => letters(n),
    NumberFormat::UpperLetter => letters(n).to_uppercase(),
    NumberFormat::LowerRoman => roman(n),
    NumberFormat::UpperRoman => roman(n).to_uppercase(),
    NumberFormat::Bullet | NumberFormat::None => String::new(),
  }
}

/// A bullet as text. Word writes most bullets as a code in a symbol font's private range, which
/// means nothing outside that font.
fn bullet(text: &str) -> String {
  match text.chars().next() {
    Some(c) if !('\u{E000}'..='\u{F8FF}').contains(&c) && !c.is_control() => c.to_string(),
    _ => "•".to_owned(),
  }
}

impl Numbering {
  pub(crate) fn parse(doc: &Xml<'_>) -> Numbering {
    let root = doc.root_element();
    let mut numbering = Numbering::default();
    let levels_of = |node: Node<'_, '_>| -> HashMap<u8, Level> {
      node
        .children()
        .filter(|n| wml(*n) == Some("lvl"))
        .filter_map(|lvl| Some((number::<u8>(attr(lvl, "ilvl"))?, read_level(lvl))))
        .collect()
    };
    for node in root.children() {
      match wml(node) {
        Some("abstractNum") => {
          if let Some(id) = number::<u32>(attr(node, "abstractNumId")) {
            numbering.definitions.insert(id, levels_of(node));
          }
        }
        Some("num") => {
          let (Some(id), Some(definition)) = (
            number::<u32>(attr(node, "numId")),
            number::<u32>(child(node, "abstractNumId").and_then(val)),
          ) else {
            continue;
          };
          let overrides = node
            .children()
            .filter(|n| wml(*n) == Some("lvlOverride"))
            .filter_map(|o| {
              let level = number::<u8>(attr(o, "ilvl"))?;
              let start = number::<u32>(child(o, "startOverride").and_then(val));
              Some((level, (start, child(o, "lvl").map(read_level))))
            })
            .collect();
          numbering.instances.insert(
            id,
            Instance {
              definition,
              overrides,
            },
          );
        }
        _ => {}
      }
    }
    numbering
  }

  /// The level as instance `id` has it: its own redefinition, or the definition's, with any
  /// restart applied.
  fn level(&self, id: u32, level: u8) -> Option<Level> {
    let instance = self.instances.get(&id)?;
    let (start, redefined) = instance
      .overrides
      .get(&level)
      .cloned()
      .unwrap_or((None, None));
    let mut resolved = redefined.or_else(|| {
      self
        .definitions
        .get(&instance.definition)?
        .get(&level)
        .cloned()
    })?;
    if let Some(start) = start {
      resolved.start = start;
    }
    Some(resolved)
  }
}

/// The running count of every list in a part. Instances of one definition count together: a
/// list that is interrupted and picked up again under another instance goes on where it stopped,
/// unless that instance says to start over.
#[derive(Debug, Default)]
pub(crate) struct Counters {
  /// By definition.
  counts: HashMap<u32, [Option<u32>; LEVELS]>,
  /// The instances and levels whose restart has been applied, which happens once.
  restarted: HashSet<(u32, u8)>,
}

impl Counters {
  /// Counts one more paragraph at `level` of list `id` and returns what stands in front of it.
  /// `None` when the document defines no such list.
  pub(crate) fn next(&mut self, numbering: &Numbering, id: u32, level: u8) -> Option<ListItem> {
    let level = level.min(8);
    let at = usize::from(level);
    let definition = numbering.level(id, level)?;
    let instance = numbering.instances.get(&id)?;
    let restart = instance
      .overrides
      .get(&level)
      .and_then(|(start, _)| *start)
      .filter(|_| self.restarted.insert((id, level)));
    let counts = self.counts.entry(instance.definition).or_default();
    counts[at] = Some(match (restart, counts[at]) {
      (Some(start), _) => start,
      (None, Some(count)) => count + 1,
      (None, None) => definition.start,
    });
    // A deeper level starts over each time a shallower one moves on.
    for deeper in &mut counts[at + 1..] {
      *deeper = None;
    }
    let counts = *counts;
    let marker = match definition.format {
      NumberFormat::Bullet => bullet(&definition.text),
      NumberFormat::None => String::new(),
      _ => {
        // `%1` is the count of the first level, written in that level's own format.
        let mut marker = definition.text.clone();
        for upper in 0..=level {
          let Some(written) = numbering.level(id, upper) else {
            continue;
          };
          let count = counts[usize::from(upper)].unwrap_or(written.start);
          marker = marker.replace(
            &format!("%{}", upper + 1),
            &write_count(written.format, count),
          );
        }
        marker
      }
    };
    Some(ListItem {
      level,
      ordered: !matches!(definition.format, NumberFormat::Bullet | NumberFormat::None),
      marker,
    })
  }
}

#[cfg(test)]
mod tests;
