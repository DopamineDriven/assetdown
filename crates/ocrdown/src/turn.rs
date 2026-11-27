//! Which way up a picture's text is. A scan can come in on its side or upside down: the picture
//! is read as given, Tesseract's own detector is asked when that read is not convincing, and the
//! other turns are tried when nothing has read well.

use image::DynamicImage;

use crate::resolution::{DEFAULT_SOURCE_PPI, set_resolution};

/// Mean Tesseract confidence (0..100) below which a page that did produce
/// text is OCR'd again rotated. Upright copier scans score 80-95; a landscape
/// page scanned in portrait scores 35-45 and comes back as punctuation soup.
pub(crate) const ROTATION_RETRY_CONFIDENCE: f32 = 55.0;

/// Tesseract's orientation detector is trusted from this confidence; below it the blind retry
/// loop decides. Measured on copier scans: upright pages 3.9–13.8, turned pages 6.0–12.2.
const OSD_MIN_CONFIDENCE: f32 = 3.0;
/// An upright pass scoring at least this mean word confidence is accepted without consulting the
/// detector. Measured: correctly oriented reads score ~96 on real and synthetic rasters; upside-down
/// and sideways reads scored 40–62. Keeps the ~1 s detector off the common, upright page.
const OSD_SKIP_CONFIDENCE: f32 = 80.0;

/// A turn that was expected stands without a search only when at least this many words read
/// at it. A stamp or a mark of a word or two reads "well" at more than one turn; measured over
/// scanned pages, the pictures that read at the turn their page is shown at held 265 words at
/// the median, and 1 in 200 held fewer than ten.
const EXPECTED_MIN_WORDS: usize = 10;

/// Whether what a picture read at the turn it was expected to need is good enough to stop at.
pub(crate) fn expected_is_convincing(words: usize, mean_confidence: f32) -> bool {
  words >= EXPECTED_MIN_WORDS && upright_is_convincing(words, mean_confidence)
}

/// Whether an upright pass is good enough to skip orientation detection altogether.
pub(crate) fn upright_is_convincing(words: usize, mean_confidence: f32) -> bool {
  words > 0 && mean_confidence >= OSD_SKIP_CONFIDENCE
}

/// Who decided a raster's orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationSource {
  /// The first pass read upright well enough.
  Upright,
  /// The caller said which turn the picture was likely to need, and it read best at that turn.
  Expected,
  /// Tesseract's orientation and script detector named the turn.
  Osd,
  /// The confidence retry over the quarter and half turns found a better score.
  Retry,
}

impl OrientationSource {
  /// How the decision is put in a warning: `upright`, `as expected`, `by OSD`, `by retry`.
  pub fn label(self) -> &'static str {
    match self {
      OrientationSource::Upright => "upright",
      OrientationSource::Expected => "as expected",
      OrientationSource::Osd => "by OSD",
      OrientationSource::Retry => "by retry",
    }
  }
}

/// `detect_os` reports how the text currently lies (0, 90, 180 or 270 degrees); the clockwise turn
/// that puts it upright is the complement. The CLI prints "Orientation 270 / Rotate 90".
fn osd_correction(orient_deg: i32) -> u32 {
  ((360 - orient_deg.rem_euclid(360)) % 360) as u32
}

/// The turn to try first, when the detector is confident enough to be worth one OCR pass.
pub(crate) fn pick_first_rotation(osd: Option<(u32, f32)>) -> Option<u32> {
  osd
    .filter(|(_, confidence)| *confidence >= OSD_MIN_CONFIDENCE)
    .map(|(degrees, _)| degrees)
}

/// Orientation and confidence from Tesseract's OSD model, or `None` when `osd.traineddata` is not
/// installed, the image is too sparse for the detector, or anything else goes wrong. The detector
/// sees the full raster: shrinking it to 1200 px drops its confidence below the trust threshold
/// while saving almost no time.
pub(crate) fn osd_rotation(
  img: &DynamicImage,
  datapath: &str,
  ppi: Option<i32>,
) -> Option<(u32, f32)> {
  use tesseract_rs::TessPageSegMode;

  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let tess = tesseract_rs::TesseractAPI::new();
  tess.init(datapath, "osd").ok()?;
  tess.set_page_seg_mode(TessPageSegMode::PSM_OSD_ONLY).ok()?;
  tess
    .set_image(rgb.as_raw(), w as i32, h as i32, 3, (w * 3) as i32)
    .ok()?;
  // The detector's non-text filter reads the resolution off the Leptonica image itself, which a
  // raw-pixel image lacks, so Tesseract still prints its "Invalid resolution 0 dpi" line here;
  // the settings below reach everything else the detector does.
  set_resolution(&tess, ppi.unwrap_or(DEFAULT_SOURCE_PPI));
  let (orient_deg, orient_conf, _script, _script_conf) = tess.detect_os().ok()?;
  Some((osd_correction(orient_deg), orient_conf))
}

/// Rotations tried, in order, when the upright pass scores too low. Most
/// rotated scans are a landscape sheet fed in portrait, so the quarter turns
/// come first and the half turn is only tried when neither of them is good.
pub(crate) const ROTATIONS: [u32; 3] = [90, 270, 180];

pub(crate) fn rotate_image(img: &DynamicImage, degrees: u32) -> DynamicImage {
  match degrees {
    90 => img.rotate90(),
    180 => img.rotate180(),
    270 => img.rotate270(),
    _ => img.clone(),
  }
}

/// A box in a raster that was turned `degrees` clockwise (now `rotated_w` × `rotated_h` pixels),
/// mapped back onto the raster as painted. Boxes are `[left, top, right, bottom]`, y down.
pub fn unrotate_pixel_box(b: [i32; 4], degrees: u32, rotated_w: i32, rotated_h: i32) -> [i32; 4] {
  let [l, t, r, bt] = b;
  match degrees {
    90 => [t, rotated_w - r, bt, rotated_w - l],
    270 => [rotated_h - bt, l, rotated_h - t, r],
    180 => [rotated_w - r, rotated_h - bt, rotated_w - l, rotated_h - t],
    _ => b,
  }
}

#[cfg(test)]
mod tests;
