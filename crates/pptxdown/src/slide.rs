//! A slide's shapes.
//!
//! A slide is a tree: shapes, pictures and frames that hold a table, a chart or SmartArt, any of
//! them inside groups. Every one has a box, stated on the shape or taken from the layers, and a
//! group moves and scales the boxes of what it holds. The file stores shapes back to front;
//! [`order`] puts them the way a slide is read.

use std::collections::HashMap;

use ooxmldown::{PackageError, Relationship, parse_xml, read_chart};
use roxmltree::Node;

use crate::body::{Links, read_paragraphs};
use crate::error::PresentationError;
use crate::inherit::{BodyStyle, Layers, ListStyle, read_frame, read_placeholder};
use crate::model::{
  ChartShape, Diagram, Frame, Group, Insets, MediaKind, MediaShape, Picture, Shape, ShapeInfo,
  Table, TableCell, TableRow, TextShape,
};
use crate::source::{Source, optional_part, relationships_of};
use crate::xml::{
  attr, child, chosen, descendant, elements, emu, flag, is, is_alternate, name, number, path,
  rel_attr,
};

/// Groups nest this deep at most.
const MAX_DEPTH: u32 = 32;
/// A slide holds this many shapes at most.
const MAX_SHAPES: u32 = 50_000;
/// The space PowerPoint leaves between a box's edges and its text when nothing says otherwise.
const DEFAULT_INSETS: Insets = Insets {
  left: 7.2,
  top: 3.6,
  right: 7.2,
  bottom: 3.6,
};

/// How the boxes inside a group map onto the slide: scaled, then moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Transform {
  scale_x: f64,
  scale_y: f64,
  move_x: f64,
  move_y: f64,
}

impl Transform {
  pub(crate) const IDENTITY: Transform = Transform {
    scale_x: 1.0,
    scale_y: 1.0,
    move_x: 0.0,
    move_y: 0.0,
  };

  /// What moves a part's boxes to where the part is shown, with no scaling.
  pub(crate) fn offset(x: f64, y: f64) -> Transform {
    Transform {
      move_x: x,
      move_y: y,
      ..Transform::IDENTITY
    }
  }

  fn apply(&self, frame: Frame) -> Frame {
    Frame {
      x: frame.x * self.scale_x + self.move_x,
      y: frame.y * self.scale_y + self.move_y,
      width: frame.width * self.scale_x,
      height: frame.height * self.scale_y,
      rotation: frame.rotation,
    }
  }

  /// A group's own box on the slide, and the transform for what the group holds. A group
  /// states its box and the coordinate space its children are written in; the two differ when
  /// the group has been moved or resized since its children were placed.
  fn group(&self, transform: Option<Node<'_, '_>>) -> (Option<Frame>, Transform) {
    let Some(own) = read_frame(transform) else {
      return (None, *self);
    };
    let space = transform.and_then(|node| {
      let (offset, extent) = (child(node, "chOff")?, child(node, "chExt")?);
      Some((
        emu(attr(offset, "x"))?,
        emu(attr(offset, "y"))?,
        emu(attr(extent, "cx"))?,
        emu(attr(extent, "cy"))?,
      ))
    });
    let (child_x, child_y, child_width, child_height) =
      space.unwrap_or((own.x, own.y, own.width, own.height));
    let scale = |outer: f64, inner: f64| if inner > 0.0 { outer / inner } else { 1.0 };
    let (scale_x, scale_y) = (
      scale(own.width, child_width),
      scale(own.height, child_height),
    );
    let inner = Transform {
      scale_x: self.scale_x * scale_x,
      scale_y: self.scale_y * scale_y,
      move_x: self.scale_x * (own.x - child_x * scale_x) + self.move_x,
      move_y: self.scale_y * (own.y - child_y * scale_y) + self.move_y,
    };
    (Some(self.apply(own)), inner)
  }
}

/// The pictures and clips a deck uses, each target once.
#[derive(Debug, Default)]
pub(crate) struct Store {
  /// The part of an embedded picture or the address of a linked one, and which it is.
  pub pictures: Vec<(String, bool)>,
  picture_at: HashMap<String, u32>,
  pub clips: Vec<(String, bool, MediaKind)>,
  clip_at: HashMap<String, u32>,
}

impl Store {
  fn picture(&mut self, target: &str, linked: bool) -> u32 {
    if let Some(&at) = self.picture_at.get(target) {
      return at;
    }
    let at = u32::try_from(self.pictures.len()).unwrap_or(u32::MAX);
    self.pictures.push((target.to_owned(), linked));
    self.picture_at.insert(target.to_owned(), at);
    at
  }

  fn clip(&mut self, target: &str, linked: bool, kind: MediaKind) -> u32 {
    if let Some(&at) = self.clip_at.get(target) {
      return at;
    }
    let at = u32::try_from(self.clips.len()).unwrap_or(u32::MAX);
    self.clips.push((target.to_owned(), linked, kind));
    self.clip_at.insert(target.to_owned(), at);
    at
  }
}

/// Reads the shape tree of one part: a slide, or the drawing of a diagram.
pub(crate) struct TreeReader<'a> {
  pub source: &'a mut dyn Source,
  /// The relationships of the part being read, by id.
  pub rels: &'a HashMap<String, Relationship>,
  pub layers: Layers<'a>,
  /// The number of each slide, by its part.
  pub slides: &'a HashMap<String, u32>,
  pub store: &'a mut Store,
  pub warnings: &'a mut Vec<String>,
  /// How many shapes have been met, which is the next one's place in the stack.
  pub met: u32,
}

impl TreeReader<'_> {
  fn links(&self) -> Links<'_> {
    Links {
      rels: self.rels,
      slides: self.slides,
    }
  }

  /// The shapes under `tree`, in the order the file stores them.
  pub(crate) fn shapes(
    &mut self,
    tree: Node<'_, '_>,
    transform: &Transform,
    depth: u32,
  ) -> Result<Vec<Shape>, PresentationError> {
    if depth > MAX_DEPTH {
      return Err(PresentationError::LimitExceeded(format!(
        "shapes are grouped more than {MAX_DEPTH} levels deep"
      )));
    }
    let mut out = Vec::new();
    for node in elements(tree) {
      if is_alternate(node) {
        if let Some(branch) = chosen(node) {
          out.extend(self.shapes(branch, transform, depth + 1)?);
        }
        continue;
      }
      let found = match name(node) {
        Some("sp") => self.text_shape(node, transform),
        Some("pic") => self.picture(node, transform),
        Some("graphicFrame") => self.graphic_frame(node, transform, depth)?,
        Some("grpSp") => self.group(node, transform, depth)?,
        _ => continue,
      };
      if self.met > MAX_SHAPES {
        return Err(PresentationError::LimitExceeded(format!(
          "a slide holds more than {MAX_SHAPES} shapes"
        )));
      }
      out.extend(found);
    }
    Ok(out)
  }

  /// What every shape has. `non_visual` is the element that names the shape; `frame` its box
  /// on the slide, when it or the layers state one.
  fn info(&mut self, non_visual: Option<Node<'_, '_>>, frame: Option<Frame>) -> ShapeInfo {
    let properties = non_visual.and_then(|node| child(node, "cNvPr"));
    let text = |local: &str| {
      properties
        .and_then(|node| attr(node, local))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
    };
    let stacking = self.met;
    self.met = self.met.saturating_add(1);
    ShapeInfo {
      frame,
      stacking,
      name: text("name"),
      alt_text: text("descr").or_else(|| text("title")),
      placeholder: read_placeholder(non_visual),
      link: properties
        .and_then(|node| child(node, "hlinkClick"))
        .and_then(|link| self.links().target(link)),
    }
  }

  /// A shape that holds text. One with no text in it is not reported.
  fn text_shape(&mut self, node: Node<'_, '_>, transform: &Transform) -> Option<Shape> {
    let non_visual = child(node, "nvSpPr");
    let inherited = self.layers.resolve(read_placeholder(non_visual));
    let frame = read_frame(path(node, &["spPr", "xfrm"]))
      .map(|frame| transform.apply(frame))
      .or(inherited.frame);
    let info = self.info(non_visual, frame);
    let body = child(node, "txBody")?;
    let list = ListStyle::read(child(body, "lstStyle")).over(&inherited.list);
    let paragraphs = read_paragraphs(body, &list, &self.links());
    if paragraphs.iter().all(|paragraph| paragraph.runs.is_empty()) {
      return None;
    }
    let properties = child(body, "bodyPr");
    let style = BodyStyle::read(properties).over(&inherited.body);
    Some(Shape::Text(TextShape {
      info,
      paragraphs,
      insets: Insets {
        left: style.left.unwrap_or(DEFAULT_INSETS.left),
        top: style.top.unwrap_or(DEFAULT_INSETS.top),
        right: style.right.unwrap_or(DEFAULT_INSETS.right),
        bottom: style.bottom.unwrap_or(DEFAULT_INSETS.bottom),
      },
      anchor: style.anchor.unwrap_or_default(),
      wrap: style.wrap.unwrap_or(true),
      // The scale is in thousandths of a percent.
      font_scale: properties
        .and_then(|node| child(node, "normAutofit"))
        .and_then(|fit| number::<f64>(attr(fit, "fontScale")))
        .map_or(1.0, |scale| scale / 100_000.0),
    }))
  }

  /// The picture a blip points at: embedded in the package, or linked to from it.
  fn blip(&mut self, blip: Node<'_, '_>) -> Option<u32> {
    let relationship = rel_attr(blip, "embed")
      .or_else(|| rel_attr(blip, "link"))
      .filter(|id| !id.is_empty())
      .and_then(|id| self.rels.get(id))?;
    Some(
      self
        .store
        .picture(&relationship.target, relationship.external),
    )
  }

  /// The clip a picture stands for, when it is the poster of an audio or a video clip. An
  /// embedded clip is named by the media element in the extension list, a linked one by the
  /// file element.
  fn clip(&mut self, properties: Node<'_, '_>) -> Option<u32> {
    let (file, kind) = elements(properties).find_map(|node| match name(node) {
      Some("videoFile" | "quickTimeFile") => Some((node, MediaKind::Video)),
      Some("audioFile" | "wavAudioFile") => Some((node, MediaKind::Audio)),
      _ => None,
    })?;
    let id = descendant(properties, "media")
      .and_then(|media| rel_attr(media, "embed").or_else(|| rel_attr(media, "link")))
      .or_else(|| rel_attr(file, "link"))
      .or_else(|| rel_attr(file, "embed"))
      .filter(|id| !id.is_empty())?;
    let relationship = self.rels.get(id)?;
    Some(
      self
        .store
        .clip(&relationship.target, relationship.external, kind),
    )
  }

  /// A picture, or the clip it is the poster of.
  fn picture(&mut self, node: Node<'_, '_>, transform: &Transform) -> Option<Shape> {
    let non_visual = child(node, "nvPicPr");
    let inherited = self.layers.resolve(read_placeholder(non_visual));
    let frame = read_frame(path(node, &["spPr", "xfrm"]))
      .map(|frame| transform.apply(frame))
      .or(inherited.frame);
    let info = self.info(non_visual, frame);
    let blip = path(node, &["blipFill", "blip"]);
    let image = blip.and_then(|blip| self.blip(blip));
    let clip = non_visual
      .and_then(|node| child(node, "nvPr"))
      .and_then(|properties| self.clip(properties));
    if let Some(media) = clip {
      return Some(Shape::Media(MediaShape {
        info,
        media,
        poster: image,
      }));
    }
    // A picture drawn from an SVG keeps the SVG beside a raster copy for older applications.
    let vector = blip
      .and_then(|blip| descendant(blip, "svgBlip"))
      .and_then(|svg| self.blip(svg));
    Some(Shape::Picture(Picture {
      info,
      image: image?,
      vector,
      background: false,
    }))
  }

  /// The picture a slide has behind everything, when it has one: it covers the slide.
  pub(crate) fn background(
    &mut self,
    slide: Node<'_, '_>,
    width: f64,
    height: f64,
  ) -> Option<Shape> {
    let blip = path(slide, &["bg", "bgPr", "blipFill", "blip"])?;
    let image = self.blip(blip)?;
    let frame = Frame {
      x: 0.0,
      y: 0.0,
      width,
      height,
      rotation: 0.0,
    };
    Some(Shape::Picture(Picture {
      info: self.info(None, Some(frame)),
      image,
      vector: None,
      background: true,
    }))
  }

  /// A frame that holds a table, a chart, SmartArt or an embedded object.
  fn graphic_frame(
    &mut self,
    node: Node<'_, '_>,
    transform: &Transform,
    depth: u32,
  ) -> Result<Option<Shape>, PresentationError> {
    let non_visual = child(node, "nvGraphicFramePr");
    let inherited = self.layers.resolve(read_placeholder(non_visual));
    let frame = read_frame(child(node, "xfrm"))
      .map(|frame| transform.apply(frame))
      .or(inherited.frame);
    let info = self.info(non_visual, frame);
    let Some(data) = path(node, &["graphic", "graphicData"]) else {
      return Ok(None);
    };
    if let Some(table) = child(data, "tbl") {
      return Ok(Some(Shape::Table(self.table(table, info))));
    }
    if let Some(chart) = child(data, "chart") {
      return self.chart(chart, info);
    }
    if let Some(parts) = child(data, "relIds") {
      return self.diagram(parts, info, depth);
    }
    // An embedded object is shown as a picture of itself.
    let image = descendant(data, "blip").and_then(|blip| self.blip(blip));
    Ok(image.map(|image| {
      Shape::Picture(Picture {
        info,
        image,
        vector: None,
        background: false,
      })
    }))
  }

  /// A table. A cell that another is merged over is present in the file and marked; it is not
  /// listed, and the cell that covers it carries the span.
  fn table(&mut self, table: Node<'_, '_>, info: ShapeInfo) -> Table {
    let list = self.layers.resolve(None).list;
    let span =
      |cell: Node<'_, '_>, local: &str| number::<u32>(attr(cell, local)).unwrap_or(1).max(1);
    let covered = |cell: Node<'_, '_>| {
      flag(attr(cell, "hMerge")) == Some(true) || flag(attr(cell, "vMerge")) == Some(true)
    };
    let rows = elements(table)
      .filter(|row| is(*row, "tr"))
      .map(|row| TableRow {
        height: emu(attr(row, "h")).unwrap_or(0.0),
        cells: elements(row)
          .filter(|cell| is(*cell, "tc"))
          .zip(0u32..)
          .filter(|(cell, _)| !covered(*cell))
          .map(|(cell, column)| TableCell {
            column,
            col_span: span(cell, "gridSpan"),
            row_span: span(cell, "rowSpan"),
            paragraphs: child(cell, "txBody")
              .map(|body| read_paragraphs(body, &list, &self.links()))
              .unwrap_or_default(),
          })
          .collect(),
      })
      .collect();
    Table {
      info,
      columns: child(table, "tblGrid")
        .into_iter()
        .flat_map(elements)
        .filter(|column| is(*column, "gridCol"))
        .map(|column| emu(attr(column, "w")).unwrap_or(0.0))
        .collect(),
      rows,
    }
  }

  /// The part a relationship of this part points at, when it stays inside the package.
  fn part_of(&self, id: Option<&str>) -> Option<String> {
    let relationship = self.rels.get(id.filter(|id| !id.is_empty())?)?;
    (!relationship.external).then(|| relationship.target.clone())
  }

  /// A chart, read from its own part into data.
  fn chart(
    &mut self,
    chart: Node<'_, '_>,
    info: ShapeInfo,
  ) -> Result<Option<Shape>, PresentationError> {
    let Some(part) = self.part_of(rel_attr(chart, "id")) else {
      return Ok(None);
    };
    let Some(bytes) = optional_part(self.source, &part, self.warnings)? else {
      return Ok(None);
    };
    match read_chart(&bytes) {
      Ok(chart) => Ok(Some(Shape::Chart(ChartShape { info, chart }))),
      Err(PackageError::Malformed(detail)) => {
        self.warnings.push(format!("{part} was left out: {detail}"));
        Ok(None)
      }
      Err(limit) => Err(limit.into()),
    }
  }

  /// SmartArt. Its content is in a data part, and what the application drew from that content
  /// is in a drawing part the data part names. The drawing is read as shapes; a deck that
  /// keeps none gives the text of its nodes.
  fn diagram(
    &mut self,
    parts: Node<'_, '_>,
    info: ShapeInfo,
    depth: u32,
  ) -> Result<Option<Shape>, PresentationError> {
    let mut nodes = Vec::new();
    let mut drawing = None;
    if let Some(part) = self.part_of(rel_attr(parts, "dm"))
      && let Some(bytes) = optional_part(self.source, &part, self.warnings)?
      && let Ok(doc) = parse_xml(&bytes)
    {
      // A point is a node of the diagram unless it says it is something else: the document,
      // or the link between two nodes.
      let points = doc.descendants().filter(|node| {
        is(*node, "pt") && matches!(attr(*node, "type"), None | Some("node" | "asst"))
      });
      for point in points {
        let text = child(point, "t")
          .map(|body| {
            read_paragraphs(body, &ListStyle::default(), &self.links())
              .iter()
              .map(crate::model::Paragraph::text)
              .collect::<Vec<_>>()
              .join("\n")
          })
          .unwrap_or_default();
        if !text.trim().is_empty() {
          nodes.push(text);
        }
      }
      drawing = doc
        .descendants()
        .find(|node| is(*node, "dataModelExt"))
        .and_then(|extension| self.part_of(attr(extension, "relId")));
    }
    let mut shapes = Vec::new();
    if let Some(part) = drawing
      && let Some(bytes) = optional_part(self.source, &part, self.warnings)?
      && let Ok(doc) = parse_xml(&bytes)
      && let Some(tree) = descendant(doc.root_element(), "spTree")
    {
      let rels = relationships_of(self.source, &part)?;
      // The drawing's boxes are measured from the corner of the diagram's own frame.
      let placed = info.frame.map_or(Transform::IDENTITY, |frame| {
        Transform::offset(frame.x, frame.y)
      });
      let mut inner = TreeReader {
        source: &mut *self.source,
        rels: &rels,
        layers: Layers {
          layout: None,
          master: None,
          defaults: self.layers.defaults,
        },
        slides: self.slides,
        store: &mut *self.store,
        warnings: &mut *self.warnings,
        met: self.met,
      };
      shapes = order(inner.shapes(tree, &placed, depth + 1)?);
      self.met = inner.met;
    }
    if shapes.is_empty() && nodes.is_empty() {
      return Ok(None);
    }
    Ok(Some(Shape::Diagram(Diagram {
      info,
      nodes: if shapes.is_empty() { nodes } else { Vec::new() },
      shapes,
    })))
  }

  /// A group: its own box, and what it holds, moved and scaled into the slide's space.
  fn group(
    &mut self,
    node: Node<'_, '_>,
    transform: &Transform,
    depth: u32,
  ) -> Result<Option<Shape>, PresentationError> {
    let (frame, inner) = transform.group(path(node, &["grpSpPr", "xfrm"]));
    let info = self.info(child(node, "nvGrpSpPr"), frame);
    let shapes = self.shapes(node, &inner, depth + 1)?;
    Ok((!shapes.is_empty()).then(|| {
      Shape::Group(Group {
        info,
        shapes: order(shapes),
      })
    }))
  }
}

/// Shapes in the order a slide is read: the title first, then in rows from the top, left to
/// right within a row. Two shapes are in one row when their tops are closer than half the
/// height of the shorter one. A shape with no box keeps its stored place after the ones that
/// have one.
pub(crate) fn order(shapes: Vec<Shape>) -> Vec<Shape> {
  let is_title = |shape: &Shape| {
    shape
      .info()
      .placeholder
      .is_some_and(|placeholder| placeholder.kind.is_title())
  };
  let is_background = |shape: &Shape| matches!(shape, Shape::Picture(p) if p.background);
  let (mut first, rest): (Vec<Shape>, Vec<Shape>) = shapes
    .into_iter()
    .partition(|shape| is_background(shape) || is_title(shape));
  // The background stays behind the title.
  first.sort_by_key(|shape| !is_background(shape));
  let (mut placed, unplaced): (Vec<(Frame, Shape)>, Vec<Shape>) =
    rest
      .into_iter()
      .fold((Vec::new(), Vec::new()), |mut acc, shape| {
        match shape.info().frame {
          Some(frame) => acc.0.push((frame, shape)),
          None => acc.1.push(shape),
        }
        acc
      });
  placed.sort_by(|(a, _), (b, _)| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
  let mut rows: Vec<Vec<(Frame, Shape)>> = Vec::new();
  for (frame, shape) in placed {
    let joins = rows
      .last()
      .and_then(|row| row.first())
      .is_some_and(|(top, _)| frame.y - top.y <= 0.5 * top.height.min(frame.height));
    match rows.last_mut().filter(|_| joins) {
      Some(row) => row.push((frame, shape)),
      None => rows.push(vec![(frame, shape)]),
    }
  }
  for row in &mut rows {
    row.sort_by(|(a, _), (b, _)| a.x.total_cmp(&b.x));
  }
  first
    .into_iter()
    .chain(rows.into_iter().flatten().map(|(_, shape)| shape))
    .chain(unplaced)
    .collect()
}

#[cfg(test)]
mod tests;
