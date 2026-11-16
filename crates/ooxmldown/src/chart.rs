//! A chart part read into data.
//!
//! A chart in a workbook, a document or a presentation is the same kind of part: what sort of
//! chart it is, its title, and its series. Each series points into a workbook for its numbers
//! and also carries the values the application last showed, cached beside the reference. This
//! reads the cache, so a chart's data is available without opening the workbook behind it.

use roxmltree::Node;

use crate::error::PackageError;
use crate::package::parse_xml;

/// A series holds this many points at most; a cache that claims more is cut short.
const MAX_POINTS: usize = 100_000;

/// What sort of chart a series is drawn as. The 3-D forms are the same kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
  pub kind: ChartKind,
  pub name: Option<String>,
  /// The label of each point: a category, or the x value of a scatter or bubble point as written.
  pub categories: Vec<String>,
  /// The value of each point; `None` where the cache holds none.
  pub values: Vec<Option<f64>>,
}

/// A chart as data. A chart that mixes kinds (bars with a line over them) has series of each.
#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
  pub title: Option<String>,
  pub series: Vec<ChartSeries>,
}

fn named(node: Node<'_, '_>, name: &str) -> bool {
  node.is_element() && node.tag_name().name() == name
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
  node.children().find(|c| named(*c, name))
}

fn kind_of(element: &str) -> Option<ChartKind> {
  let name = element.strip_suffix("Chart")?;
  Some(match name.strip_suffix("3D").unwrap_or(name) {
    "area" => ChartKind::Area,
    "bar" => ChartKind::Bar,
    "bubble" => ChartKind::Bubble,
    "doughnut" => ChartKind::Doughnut,
    "line" => ChartKind::Line,
    "pie" | "ofPie" => ChartKind::Pie,
    "radar" => ChartKind::Radar,
    "scatter" => ChartKind::Scatter,
    "stock" => ChartKind::Stock,
    "surface" => ChartKind::Surface,
    _ => ChartKind::Other,
  })
}

/// The points a reference caches, in order, with `None` where the cache skips an index.
fn cached(reference: Node<'_, '_>) -> Vec<Option<String>> {
  // A cache or a literal list; of a label with several levels, the innermost one.
  let Some(cache) = reference.descendants().find(|n| {
    matches!(
      n.tag_name().name(),
      "strCache" | "numCache" | "strLit" | "numLit" | "lvl"
    ) && n.is_element()
  }) else {
    return Vec::new();
  };
  let points: Vec<(usize, String)> = cache
    .children()
    .filter(|n| named(*n, "pt"))
    .filter_map(|point| {
      let index = point.attribute("idx")?.trim().parse::<usize>().ok()?;
      let value = child(point, "v")?.text().unwrap_or("").to_owned();
      (index < MAX_POINTS).then_some((index, value))
    })
    .collect();
  let stated = child(cache, "ptCount")
    .and_then(|count| count.attribute("val"))
    .and_then(|count| count.trim().parse::<usize>().ok());
  let seen = points.iter().map(|(index, _)| index + 1).max().unwrap_or(0);
  let mut out = vec![None; stated.unwrap_or(seen).max(seen).min(MAX_POINTS)];
  for (index, value) in points {
    if let Some(slot) = out.get_mut(index) {
      *slot = Some(value);
    }
  }
  out
}

/// Text written in place: a rich title, or a series name given as a value.
fn written(node: Node<'_, '_>) -> Option<String> {
  let text: String = node
    .descendants()
    .filter(|n| named(*n, "t") || named(*n, "v"))
    .filter_map(|n| n.text())
    .collect::<Vec<_>>()
    .join("");
  let text = text.trim();
  (!text.is_empty()).then(|| text.to_owned())
}

fn series(node: Node<'_, '_>, kind: ChartKind) -> ChartSeries {
  let labels = child(node, "cat").or_else(|| child(node, "xVal"));
  let numbers = child(node, "val").or_else(|| child(node, "yVal"));
  ChartSeries {
    kind,
    name: child(node, "tx").and_then(written),
    categories: labels
      .map(cached)
      .unwrap_or_default()
      .into_iter()
      .map(Option::unwrap_or_default)
      .collect(),
    values: numbers
      .map(cached)
      .unwrap_or_default()
      .into_iter()
      .map(|value| value.and_then(|v| v.trim().parse::<f64>().ok()))
      .collect(),
  }
}

/// Reads a chart part. A part that is XML but holds no chart gives a chart with no series.
pub fn read_chart(bytes: &[u8]) -> Result<Chart, PackageError> {
  let doc = parse_xml(bytes)?;
  let chart = child(doc.root_element(), "chart");
  let title = chart
    .and_then(|chart| child(chart, "title"))
    .and_then(written);
  let series = chart
    .and_then(|chart| child(chart, "plotArea"))
    .into_iter()
    .flat_map(|plot| plot.children())
    .filter_map(|group| Some((group, kind_of(group.tag_name().name())?)))
    .flat_map(|(group, kind)| {
      group
        .children()
        .filter(|n| named(*n, "ser"))
        .map(move |node| series(node, kind))
    })
    .collect();
  Ok(Chart { title, series })
}

#[cfg(test)]
mod tests;
