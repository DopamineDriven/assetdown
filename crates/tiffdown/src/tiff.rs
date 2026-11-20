//! A TIFF file, opened: its pages and what each says of itself.

use std::collections::HashSet;

use contentdown::PaperSize;
use image::DynamicImage;

use crate::directory::{self, Directory, Opening, Reader};
use crate::error::TiffError;
use crate::raster;

/// One page of a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
  /// Where the page stands in the file, from 1.
  pub number: u32,
  /// The page's size in pixels.
  pub width: u32,
  pub height: u32,
  /// How many pixels there are to the inch, across and down. None when the file states no
  /// resolution, or states one in no unit.
  pub resolution: Option<Resolution>,
  /// How many bits each sample of a pixel has, and how many samples a pixel has.
  pub bits_per_sample: Vec<u16>,
  pub samples_per_pixel: u16,
  /// How the page's pixels are kept.
  pub compression: Compression,
  /// What a sample's value means. None when the file does not say.
  pub photometric: Option<Photometric>,
  /// How the page is to be turned for display, as the format numbers it from 1 to 8: 1 is as
  /// it lies.
  pub orientation: u8,
  /// The program that wrote the page, as it names itself.
  pub software: Option<String>,
  /// When the page was written, as the file records it: `2015-03-04T10:22:33`. The format
  /// has no place for a time zone, so there is none.
  pub date_time: Option<String>,
}

impl Page {
  /// The page's width and height in points, of which there are 72 to the inch, as it lies in
  /// the file. None for a page that states no resolution.
  pub fn size_in_points(&self) -> Option<(f64, f64)> {
    let resolution = self.resolution?;
    Some((
      f64::from(self.width) * 72.0 / resolution.x,
      f64::from(self.height) * 72.0 / resolution.y,
    ))
  }

  /// The standard paper the page is nearest to. None for a page that states no resolution.
  pub fn paper_size(&self) -> Option<PaperSize> {
    let (width, height) = self.size_in_points()?;
    Some(contentdown::paper_size(width, height))
  }
}

/// A resolution, in pixels to the inch. A fax can have fewer down than across: its pixels are
/// then not square.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolution {
  pub x: f64,
  pub y: f64,
}

/// How a page's pixels are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
  /// As they are.
  None,
  /// A fax machine's first coding, a line at a time, with no marks between the lines.
  ModifiedHuffman,
  /// CCITT Group 3 (T.4).
  Group3,
  /// CCITT Group 4 (T.6): what a scanned or faxed page usually is.
  Group4,
  Lzw,
  /// JPEG as the format first kept it, which was withdrawn.
  OldJpeg,
  Jpeg,
  Deflate,
  PackBits,
  /// A coding this crate has no name for, by the number the file gives it.
  Other(u16),
}

/// What a sample's value means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Photometric {
  /// One sample, where zero is white: a faxed page.
  WhiteIsZero,
  /// One sample, where zero is black.
  BlackIsZero,
  Rgb,
  /// One sample that picks a color from the file's table.
  Palette,
  /// One bit that says where another picture shows through.
  Mask,
  Cmyk,
  YCbCr,
  CieLab,
  /// A meaning this crate has no name for, by the number the file gives it.
  Other(u16),
}

/// A TIFF file.
#[derive(Debug, Clone)]
pub struct Tiff {
  bytes: Vec<u8>,
  /// Whether the file's numbers run low byte first.
  little: bool,
  pages: Vec<Page>,
  /// The directory that describes each page, in step with `pages`: where a page's pixels are
  /// is read from it when they are asked for.
  directories: Vec<Directory>,
  warnings: Vec<String>,
}

impl Tiff {
  pub fn open(bytes: Vec<u8>) -> Result<Tiff, TiffError> {
    let unsupported = |what: &str| Err(TiffError::UnsupportedFormat(what.to_string()));
    let (reader, first) = match directory::opening(&bytes) {
      Opening::Classic { reader, first } => (reader, first),
      Opening::Big => return unsupported("the file is a BigTIFF, which is not read yet"),
      Opening::NotTiff => return unsupported("the file does not open as a TIFF does"),
    };
    let mut pages = Vec::new();
    let mut directories = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // Said once, however many directories it is true of.
    let mut warn = |warning: &str| {
      if !warnings.iter().any(|said| said == warning) {
        warnings.push(warning.to_string());
      }
    };
    let mut read = HashSet::new();
    let mut at = first;
    let mut entries_left = MOST_ENTRIES;
    while at != 0 {
      // A directory is counted against what is left before it is read, not after.
      entries_left = entries_left.saturating_sub(reader.entries_at(at).unwrap_or(0));
      if read.len() == MOST_DIRECTORIES || entries_left == 0 {
        warn(
          "the file has more directories than are followed, so the pages listed may not be all of them",
        );
        break;
      }
      if read.contains(&at) {
        warn("its directories lead back to one already read, where the list of its pages stops");
        break;
      }
      let Some(directory) = reader.directory(at) else {
        if read.is_empty() {
          break;
        }
        warn(
          "a directory lies past the end of the file, so the pages listed may not be all of them",
        );
        break;
      };
      read.insert(at);
      if is_a_page(&reader, &directory) {
        let number = u32::try_from(pages.len() + 1).unwrap_or(u32::MAX);
        match page(&reader, &directory, number) {
          Some(page) => {
            pages.push(page);
            directories.push(directory.clone());
          }
          None => warn("a directory states no size and is not listed as a page"),
        }
      }
      at = directory.next;
    }
    if read.is_empty() {
      return Err(TiffError::Malformed(
        "the first directory does not lie within the file".to_string(),
      ));
    }
    let little = reader.is_little();
    Ok(Tiff {
      bytes,
      little,
      pages,
      directories,
      warnings,
    })
  }

  /// The file's pages, in order.
  pub fn pages(&self) -> &[Page] {
    &self.pages
  }

  /// What was wrong with the file, where it could still be read.
  pub fn warnings(&self) -> &[String] {
    &self.warnings
  }

  /// The pixels of the page of this number, counted from 1, as the page is to be looked at:
  /// turned the way its orientation says, and with square pixels. A page of one bit or of
  /// gray comes as gray with white at 255, whichever way round the file keeps it.
  pub fn raster(&self, number: u32) -> Result<DynamicImage, TiffError> {
    let (page, directory) = self.found(number)?;
    raster::of(&Reader::over(&self.bytes, self.little), directory, page)
  }

  /// The page of this number, counted from 1, and the directory it was read from.
  pub(crate) fn found(&self, number: u32) -> Result<(&Page, &Directory), TiffError> {
    let found = usize::try_from(number)
      .ok()
      .and_then(|number| number.checked_sub(1))
      .and_then(|index| Some((self.pages.get(index)?, self.directories.get(index)?)));
    found.ok_or_else(|| {
      let (count, pages) = match self.pages.len() {
        1 => (1, "page"),
        count => (count, "pages"),
      };
      TiffError::LimitExceeded(format!(
        "page {number} was asked for, and the file has {count} {pages}"
      ))
    })
  }

  /// The page of this number as a PNG, with the pixels `raster` gives.
  pub fn png(&self, number: u32) -> Result<Vec<u8>, TiffError> {
    let image = self.raster(number)?;
    let mut png = std::io::Cursor::new(Vec::new());
    image
      .write_to(&mut png, image::ImageFormat::Png)
      .map_err(|error| {
        TiffError::Malformed(format!(
          "page {number} could not be written as a PNG: {error}"
        ))
      })?;
    Ok(png.into_inner())
  }

  /// How many bytes the file has.
  pub fn byte_size(&self) -> u64 {
    u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
  }
}

/// No more directories than this are followed. A file of scanned pages has a directory to a
/// page, and one of tens of thousands is not one that was scanned.
const MOST_DIRECTORIES: usize = 65_536;

/// No more entries than this are read over all of a file's directories. A page takes a few
/// dozen, so this is room for every page the cap above allows; what it stops is a file whose
/// directories each claim tens of thousands and lie over one another, which costs a megabyte
/// to write and minutes to read.
const MOST_ENTRIES: u64 = 1 << 22;

/// The first of an entry's whole numbers. None when the directory has no such entry, or its
/// values are not whole numbers that lie within the file.
fn number(reader: &Reader<'_>, directory: &Directory, tag: u16) -> Option<u64> {
  let values = reader.numbers(directory.entry(tag)?)?;
  values.first().copied()
}

/// Whether a directory describes a page. A file may keep a smaller copy of a page beside it,
/// or a mask that says where a page shows through, each in a directory of its own.
fn is_a_page(reader: &Reader<'_>, directory: &Directory) -> bool {
  // The newer marking is a set of bits: 1 a reduced copy, 2 one page of several, 4 a mask.
  let marked = number(reader, directory, 254).unwrap_or(0);
  // The older one is a number: 2 is a reduced copy.
  let old_marking = number(reader, directory, 255);
  marked & 0b101 == 0 && old_marking != Some(2)
}

/// What a directory says of the page it describes. What it leaves out is what the format says
/// a file means by leaving it out.
fn page(reader: &Reader<'_>, directory: &Directory, number_of_page: u32) -> Option<Page> {
  let short = |tag: u16| number(reader, directory, tag).and_then(|value| u16::try_from(value).ok());
  let text = |tag: u16| reader.text(directory.entry(tag)?);
  // A page has a size. A directory that states none describes something else.
  let side = |tag: u16| {
    number(reader, directory, tag)
      .and_then(|value| u32::try_from(value).ok())
      .filter(|side| *side > 0)
  };
  let (width, height) = (side(256)?, side(257)?);
  let bits_per_sample = directory
    .entry(258)
    .and_then(|entry| reader.numbers(entry))
    .map(|values| {
      values
        .into_iter()
        .filter_map(|bits| u16::try_from(bits).ok())
        .collect::<Vec<u16>>()
    })
    .filter(|bits| !bits.is_empty())
    .unwrap_or_else(|| vec![1]);
  let compression = match short(259).unwrap_or(1) {
    1 => Compression::None,
    2 => Compression::ModifiedHuffman,
    3 => Compression::Group3,
    4 => Compression::Group4,
    5 => Compression::Lzw,
    6 => Compression::OldJpeg,
    7 => Compression::Jpeg,
    8 | 32946 => Compression::Deflate,
    32773 => Compression::PackBits,
    other => Compression::Other(other),
  };
  let photometric = short(262).map(|meaning| match meaning {
    0 => Photometric::WhiteIsZero,
    1 => Photometric::BlackIsZero,
    2 => Photometric::Rgb,
    3 => Photometric::Palette,
    4 => Photometric::Mask,
    5 => Photometric::Cmyk,
    6 => Photometric::YCbCr,
    8 => Photometric::CieLab,
    other => Photometric::Other(other),
  });
  // How many of them there are to one of the unit: 2 the inch, 3 the centimeter. In no unit
  // (1) the two numbers say only what shape a pixel is.
  let to_the_inch = match short(296).unwrap_or(2) {
    2 => Some(1.0),
    3 => Some(2.54),
    _ => None,
  };
  let density = |tag: u16| {
    let (over, under) = *reader.fractions(directory.entry(tag)?)?.first()?;
    (over != 0 && under != 0).then(|| f64::from(over) / f64::from(under))
  };
  let resolution = match (density(282), density(283), to_the_inch) {
    (Some(across), Some(down), Some(scale)) => Some(Resolution {
      x: across * scale,
      y: down * scale,
    }),
    _ => None,
  };
  Some(Page {
    number: number_of_page,
    width,
    height,
    resolution,
    bits_per_sample,
    samples_per_pixel: short(277).unwrap_or(1),
    compression,
    photometric,
    orientation: short(274)
      .and_then(|turn| u8::try_from(turn).ok())
      .filter(|turn| (1..=8).contains(turn))
      .unwrap_or(1),
    software: text(305),
    date_time: text(306).and_then(|written| when(&written)),
  })
}

/// When a page was written, in the usual shape for a date. The format writes
/// `2015:03:04 10:22:33`; a date written some other way is kept as it stands, and one of
/// nothing but zeros is a machine that did not know the time.
fn when(written: &str) -> Option<String> {
  if !written.chars().any(|c| c.is_ascii_digit() && c != '0') {
    return None;
  }
  let marks = [(4, b':'), (7, b':'), (10, b' '), (13, b':'), (16, b':')];
  let bytes = written.as_bytes();
  let usual = bytes.len() == 19
    && bytes.iter().enumerate().all(|(at, byte)| {
      match marks.iter().find(|(place, _)| *place == at) {
        Some((_, mark)) => byte == mark,
        None => byte.is_ascii_digit(),
      }
    });
  // Every byte was a digit or a mark, so each is a character and the places below are too.
  Some(match usual {
    true => format!(
      "{}-{}-{}T{}",
      &written[..4],
      &written[5..7],
      &written[8..10],
      &written[11..]
    ),
    false => written.to_string(),
  })
}

#[cfg(test)]
mod tests;
