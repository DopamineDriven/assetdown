//! A JPEG as a page: what the file says of it, the size it is read at, and its picture.

use contentdown::PaperSize;
use image::DynamicImage;
use image::metadata::Orientation;

use crate::error::JpegError;
use crate::header::{self, Coding, Header, Resolution};

/// What a pixel's components are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
  /// One component: what a scan in black and white or in gray is.
  Gray,
  /// Three components, kept as a brightness and two differences of color: what a JPEG in
  /// color nearly always is.
  YCbCr,
  /// Three components kept as red, green, and blue, which the file has to say it does.
  Rgb,
  /// The four inks of a press.
  Cmyk,
  /// The four inks, with the three colored ones kept as a brightness and two differences.
  Ycck,
  /// Another count of components.
  Other(u8),
}

/// What the file says of its page. Nothing of the picture is decoded to make this.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
  /// The picture's size in pixels, as it lies in the file: before its orientation turns it.
  pub width: u32,
  pub height: u32,
  /// How many pixels there are to the inch, as the file states it. None when it states none,
  /// or states one in no unit. It is not always a measurement: see [`Page::size_as_read`].
  pub resolution: Option<Resolution>,
  pub bits_per_sample: u8,
  /// How many components a pixel has.
  pub components: u8,
  pub color: Color,
  pub coding: Coding,
  /// How the picture is to be turned for display, numbered from 1 to 8 as Exif does: 1 is as
  /// it lies.
  pub orientation: u8,
  /// The program that wrote the file, as it names itself.
  pub software: Option<String>,
  /// When the file was written, as it records it: `2015-03-04T10:22:33`. The format has no
  /// place for a time zone, so there is none.
  pub date_time: Option<String>,
}

/// The size a page is read at, in points, as the page is to be looked at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
  pub width: f64,
  pub height: f64,
  /// None when the size is the one the file's resolution makes. Otherwise the file states no
  /// resolution that can be believed, and the picture is taken to be a sheet of this paper,
  /// the one its shape is nearest to.
  pub assumed: Option<PaperSize>,
}

/// A sheet's sides in inches are believed between these: a narrow receipt, and a ledger sheet
/// with a little to spare.
const SHORTEST_SIDE: f64 = 2.0;
const LONGEST_SIDE: f64 = 18.0;

/// Resolutions that software writes when it has measured nothing. A page they make is
/// believed only when it is a standard sheet.
const DEFAULTS: [f64; 2] = [72.0, 96.0];

/// The papers a page is taken to be when its size is not known.
const SHEETS: [PaperSize; 3] = [PaperSize::Letter, PaperSize::A4, PaperSize::Legal];

impl Page {
  /// The page's width and height in points, of which there are 72 to the inch, as it lies in
  /// the file and by the resolution the file states. None for a page that states none.
  pub fn size_in_points(&self) -> Option<(f64, f64)> {
    let resolution = self.resolution?;
    Some((
      f64::from(self.width) * 72.0 / resolution.x,
      f64::from(self.height) * 72.0 / resolution.y,
    ))
  }

  /// The standard paper the stated size is nearest to. None for a page that states no
  /// resolution.
  pub fn paper_size(&self) -> Option<PaperSize> {
    let (width, height) = self.size_in_points()?;
    Some(contentdown::paper_size(width, height))
  }

  /// Whether the size the stated resolution makes is one a sheet could have.
  fn believed(&self) -> Option<(f64, f64)> {
    let resolution = self.resolution?;
    let (width, height) = self.size_in_points()?;
    let (short, long) = (width.min(height) / 72.0, width.max(height) / 72.0);
    let could_be = short >= SHORTEST_SIDE && long <= LONGEST_SIDE;
    let measured = !DEFAULTS.contains(&resolution.x) || resolution.x != resolution.y;
    let standard = contentdown::paper_size(width, height) != PaperSize::Unknown;
    (could_be && (measured || standard)).then_some((width, height))
  }

  /// The size the page is read at, as it is to be looked at: turned the way its orientation
  /// says.
  ///
  /// It is the size the file's resolution makes when that is a size a sheet could have. A
  /// resolution of 72 or 96, which software writes when it has measured nothing, is believed
  /// only when it makes a standard sheet. Otherwise the picture is taken to be a sheet of the
  /// paper its shape is nearest to (letter, A4, or legal), and `assumed` names it.
  pub fn size_as_read(&self) -> PageSize {
    // Orientations 5 to 8 keep the page on its side.
    let on_its_side = (5..=8).contains(&self.orientation);
    let looked_at = |(width, height): (f64, f64)| match on_its_side {
      true => (height, width),
      false => (width, height),
    };
    if let Some(stated) = self.believed() {
      let (width, height) = looked_at(stated);
      return PageSize {
        width,
        height,
        assumed: None,
      };
    }
    let (width, height) = looked_at((f64::from(self.width), f64::from(self.height)));
    let shape = width.min(height) / width.max(height);
    let nearest = SHEETS
      .iter()
      .filter_map(|sheet| Some((*sheet, sheet.dimensions()?)))
      .min_by(|(_, a), (_, b)| {
        let away = |(short, long): (f64, f64)| (short / long - shape).abs();
        away(*a).total_cmp(&away(*b))
      });
    // The list is not empty and every paper in it has a size.
    let (sheet, (short, long)) = nearest.unwrap_or((PaperSize::Letter, (612.0, 792.0)));
    let (width, height) = match width > height {
      true => (long, short),
      false => (short, long),
    };
    PageSize {
      width,
      height,
      assumed: Some(sheet),
    }
  }
}

/// What the components of a pixel are, from how many there are and what the Adobe segment
/// says of them.
fn color(header: &Header) -> Color {
  match (header.components, header.adobe_transform) {
    (1, _) => Color::Gray,
    (3, Some(0)) => Color::Rgb,
    (3, _) => Color::YCbCr,
    (4, Some(2)) => Color::Ycck,
    (4, _) => Color::Cmyk,
    (other, _) => Color::Other(other),
  }
}

/// No picture of more pixels than this is decoded. A letter page at 1,200 pixels to the inch
/// is half of it; what it stops is a header that claims a size no page has.
const MOST_PIXELS: u64 = 1 << 28;

/// A JPEG file: one page.
pub struct Jpeg {
  bytes: Vec<u8>,
  page: Page,
}

impl Jpeg {
  /// Reads the file's header. Fails with `unsupported format: …` for bytes that are not a
  /// JPEG, and with `malformed file: …` for one whose header is damaged. A picture kept in a
  /// coding that is not read still opens: its page is listed, and [`Jpeg::raster`] refuses it.
  pub fn open(bytes: Vec<u8>) -> Result<Jpeg, JpegError> {
    let header = header::read(&bytes)?;
    let page = Page {
      width: header.width,
      height: header.height,
      resolution: header.resolution,
      bits_per_sample: header.precision,
      components: header.components,
      color: color(&header),
      coding: header.coding,
      orientation: header.orientation,
      software: header.software,
      date_time: header.date_time,
    };
    Ok(Jpeg { bytes, page })
  }

  /// What the file says of its page.
  pub fn page(&self) -> &Page {
    &self.page
  }

  /// How many bytes the file has.
  pub fn byte_size(&self) -> u64 {
    u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
  }

  /// The picture as it is to be looked at: turned the way its orientation says. A picture of
  /// gray comes as gray, and one in color as red, green, and blue.
  pub fn raster(&self) -> Result<DynamicImage, JpegError> {
    let unsupported = |what: String| Err(JpegError::UnsupportedFormat(what));
    let page = &self.page;
    match page.coding {
      Coding::Baseline | Coding::Extended | Coding::Progressive => {}
      Coding::Lossless => {
        return unsupported(
          "the picture is kept without loss, a coding of JPEG that is not read".to_string(),
        );
      }
      Coding::Other(marker) => {
        return unsupported(format!(
          "the picture is kept in a coding of JPEG that is not read (frame marker {marker:#04X})"
        ));
      }
    }
    if page.bits_per_sample != 8 {
      return unsupported(format!(
        "the picture has samples of {} bits, and 8 are read",
        page.bits_per_sample
      ));
    }
    let pixels = u64::from(page.width) * u64::from(page.height);
    if pixels > MOST_PIXELS {
      return Err(JpegError::LimitExceeded(format!(
        "the picture has {pixels} pixels, and no more than {MOST_PIXELS} are decoded"
      )));
    }
    let mut image = image::load_from_memory_with_format(&self.bytes, image::ImageFormat::Jpeg)
      .map_err(|error| {
        JpegError::Malformed(format!("the picture could not be decoded: {error}"))
      })?;
    if let Some(turn) = Orientation::from_exif(page.orientation) {
      image.apply_orientation(turn);
    }
    Ok(image)
  }

  /// The picture as a PNG, with the pixels `raster` gives.
  pub fn png(&self) -> Result<Vec<u8>, JpegError> {
    let image = self.raster()?;
    let mut png = std::io::Cursor::new(Vec::new());
    image
      .write_to(&mut png, image::ImageFormat::Png)
      .map_err(|error| {
        JpegError::Malformed(format!(
          "the picture could not be written as a PNG: {error}"
        ))
      })?;
    Ok(png.into_inner())
  }
}

#[cfg(test)]
mod tests;
