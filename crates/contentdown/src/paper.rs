//! Standard paper sizes, and the one nearest to a page.
//!
//! A format crate knows a page's width and height in points: a PDF from its page box, a word
//! document from its section properties. The table and the matcher are the same for all of them,
//! so they live here.

/// A standard paper size: the ISO 216 A and B series and the five North American sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaperSize {
  A0,
  A1,
  A2,
  A3,
  A4,
  A5,
  A6,
  A7,
  A8,
  A9,
  A10,
  B0,
  B1,
  B2,
  B3,
  B4,
  B5,
  B6,
  B7,
  B8,
  B9,
  B10,
  Letter,
  Legal,
  Tabloid,
  Executive,
  Statement,
  /// No standard size is within [`TOLERANCE`] of the page.
  Unknown,
}

/// Portrait sizes in points: the size, its short side and its long side. The ISO sizes are their
/// millimeters at 72 / 25.4 points each, rounded; the North American sizes are their inches at 72.
const SIZES: [(PaperSize, f64, f64); 27] = [
  (PaperSize::A0, 2384.0, 3370.0),
  (PaperSize::A1, 1684.0, 2384.0),
  (PaperSize::A2, 1191.0, 1684.0),
  (PaperSize::A3, 842.0, 1191.0),
  (PaperSize::A4, 595.0, 842.0),
  (PaperSize::A5, 420.0, 595.0),
  (PaperSize::A6, 298.0, 420.0),
  (PaperSize::A7, 210.0, 298.0),
  (PaperSize::A8, 147.0, 210.0),
  (PaperSize::A9, 105.0, 147.0),
  (PaperSize::A10, 74.0, 105.0),
  (PaperSize::B0, 2835.0, 4008.0),
  (PaperSize::B1, 2004.0, 2835.0),
  (PaperSize::B2, 1417.0, 2004.0),
  (PaperSize::B3, 1001.0, 1417.0),
  (PaperSize::B4, 709.0, 1001.0),
  (PaperSize::B5, 499.0, 709.0),
  (PaperSize::B6, 354.0, 499.0),
  (PaperSize::B7, 249.0, 354.0),
  (PaperSize::B8, 176.0, 249.0),
  (PaperSize::B9, 125.0, 176.0),
  (PaperSize::B10, 88.0, 125.0),
  (PaperSize::Letter, 612.0, 792.0),
  (PaperSize::Legal, 612.0, 1008.0),
  (PaperSize::Tabloid, 792.0, 1224.0),
  (PaperSize::Executive, 522.0, 756.0),
  (PaperSize::Statement, 396.0, 612.0),
];

impl PaperSize {
  /// The size's own dimensions in points, portrait: its short side, then its long side. `None`
  /// for [`PaperSize::Unknown`].
  pub fn dimensions(self) -> Option<(f64, f64)> {
    SIZES
      .iter()
      .find(|&&(size, _, _)| size == self)
      .map(|&(_, short, long)| (short, long))
  }
}

/// How far either side of a page may be from a standard size's, as a fraction of that side. A
/// scan is rarely the exact size (a letter page scanned at 200 dpi comes out 613.44 points wide),
/// and past about 3% one page can be in reach of two sizes at once, A5 and Statement first.
const TOLERANCE: f64 = 0.03;

/// The standard size nearest to a page of `width` by `height` points, in either orientation, or
/// [`PaperSize::Unknown`] when even the nearest is further than [`TOLERANCE`] on one of its sides
/// or the page has no positive, finite size.
pub fn paper_size(width: f64, height: f64) -> PaperSize {
  let measurable = |side: f64| side.is_finite() && side > 0.0;
  if !measurable(width) || !measurable(height) {
    return PaperSize::Unknown;
  }
  let (short, long) = if width <= height {
    (width, height)
  } else {
    (height, width)
  };
  SIZES
    .iter()
    .map(|&(size, w, h)| {
      let error = ((short - w).abs() / w).max((long - h).abs() / h);
      (size, error)
    })
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .filter(|&(_, error)| error <= TOLERANCE)
    .map_or(PaperSize::Unknown, |(size, _)| size)
}

/// The standard sizes on either side of a `width` by `height` page, by area: the largest one no
/// larger than the page, then the smallest one larger than it. A side is `None` when the page is
/// beyond that end of the table (smaller than A10, or larger than B0), and both are when the page
/// has no positive, finite size. This is for a page [`paper_size`] calls unknown: it says which
/// two sizes the page lies between.
pub fn paper_neighbors(width: f64, height: f64) -> (Option<PaperSize>, Option<PaperSize>) {
  let area = width * height;
  if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
    return (None, None);
  }
  let areas = || SIZES.iter().map(|&(size, w, h)| (size, w * h));
  let smaller = areas()
    .filter(|&(_, a)| a <= area)
    .max_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(size, _)| size);
  let larger = areas()
    .filter(|&(_, a)| a > area)
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(size, _)| size);
  (smaller, larger)
}

#[cfg(test)]
mod tests;
