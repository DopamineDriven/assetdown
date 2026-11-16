//! The three layers of a slide.
//!
//! A slide is built on a layout, and a layout on a master. A placeholder on the slide may state
//! nothing but what it is for, and take its box, its font size and its bullets from the
//! placeholder it matches on the layout, which may take them from the master's. Under those sit
//! the master's text styles (one for titles, one for body text, one for everything else) and
//! the deck's defaults. For each thing a shape leaves unsaid, the answer is the nearest one up
//! that chain.

use roxmltree::Node;

use crate::model::{Align, Anchor, Frame, Placeholder, PlaceholderKind};
use crate::xml::{attr, child, descendant, elements, emu, flag, is, name, number, path};

/// A paragraph is on one of nine levels.
pub(crate) const LEVELS: usize = 9;

/// What stands in front of a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Bullet {
  None,
  /// A character, and whether it comes from a symbol font, where it means nothing as text.
  Char {
    text: String,
    symbol: bool,
  },
  /// A number or letter that counts: the scheme (`arabicPeriod`) and where it starts.
  Auto {
    scheme: String,
    start: u32,
  },
}

/// What one level of text is like, as one place states it: `None` where that place is silent.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct LevelStyle {
  /// In points.
  pub size: Option<f64>,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  pub underline: Option<bool>,
  pub strike: Option<bool>,
  pub font: Option<String>,
  pub align: Option<Align>,
  pub bullet: Option<Bullet>,
}

/// Fonts whose characters are pictures: a bullet set in one is not the character it is stored as.
fn is_symbol_font(name: &str) -> bool {
  let name = name.to_ascii_lowercase();
  name.starts_with("wingdings") || name == "symbol" || name == "webdings"
}

impl LevelStyle {
  /// Run properties: `a:rPr`, `a:defRPr` or `a:endParaRPr`.
  pub(crate) fn of_run(properties: Option<Node<'_, '_>>) -> LevelStyle {
    let Some(node) = properties else {
      return LevelStyle::default();
    };
    LevelStyle {
      // Sizes are in hundredths of a point.
      size: number::<f64>(attr(node, "sz")).map(|size| size / 100.0),
      bold: flag(attr(node, "b")),
      italic: flag(attr(node, "i")),
      underline: attr(node, "u").map(|u| u != "none"),
      strike: attr(node, "strike").map(|s| s != "noStrike"),
      // A name that starts with `+` points into the theme and names no font itself.
      font: child(node, "latin")
        .and_then(|latin| attr(latin, "typeface"))
        .filter(|face| !face.is_empty() && !face.starts_with('+'))
        .map(str::to_owned),
      align: None,
      bullet: None,
    }
  }

  /// Paragraph properties: `a:pPr`, or one level of a list style. They hold the alignment, the
  /// bullet, and the run properties text on that level starts from.
  pub(crate) fn of_paragraph(properties: Option<Node<'_, '_>>) -> LevelStyle {
    let Some(node) = properties else {
      return LevelStyle::default();
    };
    let symbol = child(node, "buFont")
      .and_then(|font| attr(font, "typeface"))
      .is_some_and(is_symbol_font);
    let bullet = elements(node).find_map(|bullet| match name(bullet) {
      Some("buNone") => Some(Bullet::None),
      Some("buChar") => Some(Bullet::Char {
        text: attr(bullet, "char").unwrap_or("").to_owned(),
        symbol,
      }),
      Some("buAutoNum") => Some(Bullet::Auto {
        scheme: attr(bullet, "type").unwrap_or("arabicPeriod").to_owned(),
        start: number::<u32>(attr(bullet, "startAt")).unwrap_or(1),
      }),
      _ => None,
    });
    LevelStyle {
      align: match attr(node, "algn") {
        Some("l") => Some(Align::Left),
        Some("ctr") => Some(Align::Center),
        Some("r") => Some(Align::Right),
        Some("just" | "justLow" | "dist" | "thaiDist") => Some(Align::Justify),
        _ => None,
      },
      bullet,
      ..LevelStyle::of_run(child(node, "defRPr"))
    }
  }

  /// `self` where it speaks, `under` where it does not.
  pub(crate) fn over(self, under: &LevelStyle) -> LevelStyle {
    LevelStyle {
      size: self.size.or(under.size),
      bold: self.bold.or(under.bold),
      italic: self.italic.or(under.italic),
      underline: self.underline.or(under.underline),
      strike: self.strike.or(under.strike),
      font: self.font.or_else(|| under.font.clone()),
      align: self.align.or(under.align),
      bullet: self.bullet.or_else(|| under.bullet.clone()),
    }
  }
}

/// A style for each level: a shape's list style, or one of a master's text styles.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ListStyle {
  pub levels: [LevelStyle; LEVELS],
}

impl ListStyle {
  /// From an element that holds `a:lvl1pPr` to `a:lvl9pPr`.
  pub(crate) fn read(node: Option<Node<'_, '_>>) -> ListStyle {
    let mut style = ListStyle::default();
    let Some(node) = node else {
      return style;
    };
    for (level, slot) in style.levels.iter_mut().enumerate() {
      *slot = LevelStyle::of_paragraph(child(node, &format!("lvl{}pPr", level + 1)));
    }
    style
  }

  pub(crate) fn over(self, under: &ListStyle) -> ListStyle {
    let mut levels = self.levels.into_iter();
    ListStyle {
      levels: std::array::from_fn(|level| {
        levels.next().unwrap_or_default().over(&under.levels[level])
      }),
    }
  }
}

/// How a text body sits in its box, as one place states it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct BodyStyle {
  pub left: Option<f64>,
  pub top: Option<f64>,
  pub right: Option<f64>,
  pub bottom: Option<f64>,
  pub anchor: Option<Anchor>,
  pub wrap: Option<bool>,
}

impl BodyStyle {
  /// From `a:bodyPr`.
  pub(crate) fn read(properties: Option<Node<'_, '_>>) -> BodyStyle {
    let Some(node) = properties else {
      return BodyStyle::default();
    };
    BodyStyle {
      left: emu(attr(node, "lIns")),
      top: emu(attr(node, "tIns")),
      right: emu(attr(node, "rIns")),
      bottom: emu(attr(node, "bIns")),
      anchor: match attr(node, "anchor") {
        Some("t") => Some(Anchor::Top),
        Some("ctr") => Some(Anchor::Middle),
        Some("b") => Some(Anchor::Bottom),
        _ => None,
      },
      wrap: attr(node, "wrap").map(|wrap| wrap != "none"),
    }
  }

  pub(crate) fn over(self, under: &BodyStyle) -> BodyStyle {
    BodyStyle {
      left: self.left.or(under.left),
      top: self.top.or(under.top),
      right: self.right.or(under.right),
      bottom: self.bottom.or(under.bottom),
      anchor: self.anchor.or(under.anchor),
      wrap: self.wrap.or(under.wrap),
    }
  }
}

/// A box from `a:xfrm` or `p:xfrm`: where it is, how large, and how far it is turned.
pub(crate) fn read_frame(transform: Option<Node<'_, '_>>) -> Option<Frame> {
  let node = transform?;
  let (offset, extent) = (child(node, "off")?, child(node, "ext")?);
  Some(Frame {
    x: emu(attr(offset, "x"))?,
    y: emu(attr(offset, "y"))?,
    width: emu(attr(extent, "cx"))?,
    height: emu(attr(extent, "cy"))?,
    // Rotation is in 60,000ths of a degree.
    rotation: number::<f64>(attr(node, "rot")).map_or(0.0, |turn| turn / 60_000.0),
  })
}

/// The placeholder a shape says it is, from `p:nvPr`.
pub(crate) fn read_placeholder(non_visual: Option<Node<'_, '_>>) -> Option<Placeholder> {
  let node = descendant(non_visual?, "ph")?;
  Some(Placeholder {
    kind: PlaceholderKind::parse(attr(node, "type")),
    index: number::<u32>(attr(node, "idx")),
  })
}

/// A placeholder as a layout or a master defines it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayerPlaceholder {
  pub placeholder: Placeholder,
  pub frame: Option<Frame>,
  pub list: ListStyle,
  pub body: BodyStyle,
}

/// A layout or a master: its name and the placeholders it defines.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Layer {
  pub name: Option<String>,
  pub placeholders: Vec<LayerPlaceholder>,
}

impl Layer {
  pub(crate) fn read(root: Node<'_, '_>) -> Layer {
    let slide = child(root, "cSld");
    let placeholders = slide
      .into_iter()
      .flat_map(|slide| slide.descendants())
      .filter(|node| is(*node, "sp"))
      .filter_map(|shape| {
        let placeholder = read_placeholder(child(shape, "nvSpPr"))?;
        let body = child(shape, "txBody");
        Some(LayerPlaceholder {
          placeholder,
          frame: read_frame(path(shape, &["spPr", "xfrm"])),
          list: ListStyle::read(body.and_then(|body| child(body, "lstStyle"))),
          body: BodyStyle::read(body.and_then(|body| child(body, "bodyPr"))),
        })
      })
      .collect();
    Layer {
      name: slide
        .and_then(|slide| attr(slide, "name"))
        .filter(|name| !name.is_empty())
        .map(str::to_owned),
      placeholders,
    }
  }

  /// The placeholder of a layout that a slide's placeholder stands for: the one with its
  /// number, or else the one of its kind.
  fn for_slide(&self, placeholder: Placeholder) -> Option<&LayerPlaceholder> {
    let same_kind = |kind: PlaceholderKind| {
      kind == placeholder.kind || (kind.is_title() && placeholder.kind.is_title())
    };
    placeholder
      .index
      .and_then(|index| {
        self
          .placeholders
          .iter()
          .find(|own| own.placeholder.index == Some(index))
      })
      .or_else(|| {
        self
          .placeholders
          .iter()
          .find(|own| same_kind(own.placeholder.kind))
      })
  }

  /// The placeholder of a master that one of `kind` falls back to. A master has a title, a
  /// body, and the three that repeat; anything else that holds content falls back to the body.
  fn for_kind(&self, kind: PlaceholderKind) -> Option<&LayerPlaceholder> {
    let wanted = match kind {
      kind if kind.is_title() => PlaceholderKind::Title,
      kind if kind.is_furniture() => kind,
      _ => PlaceholderKind::Body,
    };
    self
      .placeholders
      .iter()
      .find(|own| own.placeholder.kind == wanted)
  }
}

/// A master: its placeholders and its three text styles.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Master {
  pub layer: Layer,
  pub title: ListStyle,
  pub body: ListStyle,
  pub other: ListStyle,
}

impl Master {
  pub(crate) fn read(root: Node<'_, '_>) -> Master {
    let styles = child(root, "txStyles");
    let style = |name: &str| ListStyle::read(styles.and_then(|styles| child(styles, name)));
    Master {
      layer: Layer::read(root),
      title: style("titleStyle"),
      body: style("bodyStyle"),
      other: style("otherStyle"),
    }
  }
}

/// What a slide's shapes take what they leave unsaid from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layers<'a> {
  pub layout: Option<&'a Layer>,
  pub master: Option<&'a Master>,
  /// The deck's default text style.
  pub defaults: &'a ListStyle,
}

/// What the layers give one shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Inherited {
  pub frame: Option<Frame>,
  pub list: ListStyle,
  pub body: BodyStyle,
}

impl Layers<'_> {
  /// What a shape takes from the layers: as the placeholder it says it is, or, when it is none,
  /// as plain text on the slide.
  pub(crate) fn resolve(&self, placeholder: Option<Placeholder>) -> Inherited {
    let empty = ListStyle::default();
    let master_text = |pick: fn(&Master) -> &ListStyle| self.master.map_or(&empty, pick);
    let Some(placeholder) = placeholder else {
      return Inherited {
        frame: None,
        list: master_text(|m| &m.other).clone().over(self.defaults),
        body: BodyStyle::default(),
      };
    };
    let on_layout = self.layout.and_then(|layout| layout.for_slide(placeholder));
    let on_master = self
      .master
      .and_then(|master| master.layer.for_kind(placeholder.kind));
    let text = match placeholder.kind {
      kind if kind.is_title() => master_text(|m| &m.title),
      kind if kind.is_furniture() => master_text(|m| &m.other),
      _ => master_text(|m| &m.body),
    };
    let own = |pick: fn(&LayerPlaceholder) -> &ListStyle, from: Option<&LayerPlaceholder>| {
      from.map_or_else(ListStyle::default, |found| pick(found).clone())
    };
    Inherited {
      frame: on_layout
        .and_then(|found| found.frame)
        .or_else(|| on_master.and_then(|found| found.frame)),
      list: own(|p| &p.list, on_layout)
        .over(&own(|p| &p.list, on_master))
        .over(text)
        .over(self.defaults),
      body: on_layout
        .map(|found| found.body)
        .unwrap_or_default()
        .over(&on_master.map(|found| found.body).unwrap_or_default()),
    }
  }
}

#[cfg(test)]
mod tests;
