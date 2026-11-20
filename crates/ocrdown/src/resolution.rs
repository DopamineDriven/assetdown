//! The resolution a picture is read at. Tesseract sizes its guesses about text from it, and
//! guesses badly when told nothing.

/// Resolution assumed for a raster whose painted size is unknown: a scan, not Tesseract's 70.
pub const DEFAULT_SOURCE_PPI: i32 = 300;
pub(crate) const MIN_SOURCE_PPI: i32 = 70;
pub(crate) const MAX_SOURCE_PPI: i32 = 1200;

/// Pixels per inch of a raster painted `points` wide. Tesseract sizes its text estimates from
/// the source resolution and guesses 70 dpi when told nothing, which is wrong for every scan.
pub fn source_ppi(pixels: u32, points: f64) -> i32 {
  if pixels == 0 || !points.is_finite() || points <= 0.0 {
    return DEFAULT_SOURCE_PPI;
  }
  (f64::from(pixels) / (points / 72.0))
    .round()
    .clamp(f64::from(MIN_SOURCE_PPI), f64::from(MAX_SOURCE_PPI)) as i32
}

/// Tell a Tesseract instance the raster's resolution. `user_defined_dpi` is read before the
/// image's own resolution on every path, the orientation detector included, and the source
/// resolution after `set_image` covers the recognizer's size estimates.
pub(crate) fn set_resolution(tess: &tesseract_rs::TesseractAPI, ppi: i32) {
  let _ = tess.set_variable("user_defined_dpi", &ppi.to_string());
  let _ = tess.set_source_resolution(ppi);
}

/// The resolution a small picture is brought up to before it is read. Tesseract is built for
/// pages near 300 pixels to the inch and reads a page at 100 badly. Measured on six invoice
/// pages rendered as JPEG, against the words of the PDFs they were rendered from: enlarging to
/// 200 found more words at every resolution below it (32% to 75% at 72, 58% to 92% at 100,
/// 91% to 92% at 150), enlarging further to 300 found fewer again below 150, and a picture
/// already at 200 gained nothing.
pub const WORKING_PPI: i32 = 200;

/// A picture within a twentieth of the working resolution is left as it is: a page rendered
/// at 200 to the inch comes out a pixel or two short of it by rounding.
const NEAR_ENOUGH: f64 = 0.95;

/// No picture is enlarged past this many pixels.
const MOST_PIXELS: u64 = 1 << 28;

/// `image` enlarged to [`WORKING_PPI`], when it has fewer pixels to the inch than that across
/// a page `width_in_points` wide. None when it has enough already or nearly enough, and when the page has no
/// width to measure it by. The page's size in points is the same before and after, so words
/// read from the larger picture are placed as they would be from the smaller.
pub fn enlarged(image: &image::DynamicImage, width_in_points: f64) -> Option<image::DynamicImage> {
  let measurable = width_in_points.is_finite() && width_in_points > 0.0;
  if !measurable || image.width() == 0 || image.height() == 0 {
    return None;
  }
  let ppi = f64::from(image.width()) * 72.0 / width_in_points;
  if ppi >= f64::from(WORKING_PPI) * NEAR_ENOUGH {
    return None;
  }
  let by = f64::from(WORKING_PPI) / ppi;
  let side = |pixels: u32| {
    let longer = (f64::from(pixels) * by).round();
    // Rounded, and no larger than a side can be.
    (longer >= 1.0 && longer <= f64::from(u32::MAX)).then_some(longer as u32)
  };
  let (width, height) = (side(image.width())?, side(image.height())?);
  if u64::from(width) * u64::from(height) > MOST_PIXELS {
    return None;
  }
  Some(image.resize_exact(width, height, image::imageops::FilterType::Lanczos3))
}

#[cfg(test)]
mod tests;
