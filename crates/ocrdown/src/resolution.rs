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

#[cfg(test)]
mod tests;
