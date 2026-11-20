//! A page's pixels, read out of the file. A page keeps them in strips, each a run of rows coded
//! on its own; what comes out of a strip is samples, and what the samples mean is the page's to
//! say.

use image::{DynamicImage, GrayImage, RgbImage, RgbaImage};

use crate::directory::{Directory, Reader};
use crate::error::TiffError;
use crate::tiff::{Compression, Page, Photometric};

/// No page of more pixels than this is read: a sheet of the largest paper there is, scanned
/// finely, has fewer.
const MOST_PIXELS: u64 = 1 << 28;

/// What the samples of a pixel are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Samples {
  /// One bit: black or white.
  Bit,
  /// Eight bits of gray.
  Gray,
  /// Eight bits that pick a color from the page's table.
  Palette,
  /// Red, green, and blue, eight bits each.
  Rgb,
  /// The same and a fourth sample, which says how much of the pixel shows when `alpha`.
  RgbAndOne { alpha: bool },
}

impl Samples {
  /// How many bytes a row of `width` pixels is kept in.
  fn row_bytes(self, width: u64) -> u64 {
    match self {
      Samples::Bit => width.div_ceil(8),
      Samples::Gray | Samples::Palette => width,
      Samples::Rgb => width * 3,
      Samples::RgbAndOne { .. } => width * 4,
    }
  }
}

/// The pixels of `page`, which `directory` describes.
pub(crate) fn of(
  reader: &Reader<'_>,
  directory: &Directory,
  page: &Page,
) -> Result<DynamicImage, TiffError> {
  let n = page.number;
  let unsupported = |what: String| Err(TiffError::UnsupportedFormat(what));
  let malformed = |what: String| TiffError::Malformed(what);
  let numbers = |tag: u16| directory.entry(tag).and_then(|entry| reader.numbers(entry));
  let number = |tag: u16| numbers(tag).and_then(|values| values.first().copied());

  match page.compression {
    Compression::None | Compression::Lzw | Compression::Group4 => {}
    Compression::OldJpeg => {
      return unsupported(format!(
        "page {n} is kept as old-style JPEG, which is not read"
      ));
    }
    Compression::Other(coding) => {
      return unsupported(format!(
        "page {n} is kept in coding {coding}, which is not read"
      ));
    }
    coding => {
      return unsupported(format!(
        "page {n} is kept as {coding:?}, which is not read yet"
      ));
    }
  }
  if let Some(bits) = page
    .bits_per_sample
    .iter()
    .find(|bits| !matches!(bits, 1 | 8))
  {
    return unsupported(format!(
      "page {n} has samples of {bits} bits, which are not read yet"
    ));
  }
  let shows_through = matches!(number(338), Some(1 | 2));
  let samples = match (
    page.bits_per_sample.as_slice(),
    page.samples_per_pixel,
    page.photometric,
  ) {
    ([1], 1, _) => Samples::Bit,
    ([8], 1, Some(Photometric::Palette)) => Samples::Palette,
    ([8], 1, _) => Samples::Gray,
    ([8, 8, 8], 3, Some(Photometric::Rgb)) => Samples::Rgb,
    ([8, 8, 8, 8], 4, Some(Photometric::Rgb)) => Samples::RgbAndOne {
      alpha: shows_through,
    },
    (bits, count, _) => {
      return unsupported(format!(
        "page {n} has {count} samples to a pixel of {bits:?} bits, which are not read yet"
      ));
    }
  };
  let (width, height) = (u64::from(page.width), u64::from(page.height));
  if width * height > MOST_PIXELS {
    return Err(TiffError::LimitExceeded(format!(
      "page {n} is {width} by {height} pixels, more than are read"
    )));
  }

  // A page is kept in strips or in tiles, and says which by the entry it lists them in.
  if directory.entry(324).is_some() {
    return unsupported(format!("page {n} is kept in tiles, which are not read yet"));
  }
  // Where the strips are, how long each is, and how many rows a strip holds.
  let Some(offsets) = numbers(273) else {
    return Err(malformed(format!(
      "page {n} does not say where its pixels are"
    )));
  };
  // An old writer may leave the lengths out: a strip then runs to the next one, or to the end
  // of the file.
  let lengths = numbers(279)
    .filter(|lengths| lengths.len() >= offsets.len())
    .unwrap_or_else(|| {
      let ends_at = |offset: u64| {
        let next = offsets
          .iter()
          .copied()
          .filter(|other| *other > offset)
          .min();
        next.unwrap_or(reader.len())
      };
      offsets
        .iter()
        .map(|offset| ends_at(*offset).saturating_sub(*offset))
        .collect()
    });
  // Samples kept as what each adds to the one before it in its row.
  let as_differences = number(317) == Some(2);
  let rows_to_a_strip = number(278).filter(|rows| *rows > 0).unwrap_or(height);
  // The low end of a byte first: each byte is turned end for end before it is read.
  let low_end_first = number(266) == Some(2);

  let row_bytes = samples.row_bytes(width);
  let mut kept: Vec<u8> = Vec::new();
  let mut rows = 0u64;
  for (index, offset) in offsets.iter().enumerate() {
    let here = rows_to_a_strip.min(height - rows);
    if here == 0 {
      break;
    }
    let strip = index + 1;
    let data = lengths
      .get(index)
      .and_then(|length| reader.stretch(*offset, *length))
      .ok_or_else(|| {
        malformed(format!(
          "strip {strip} of page {n} does not lie within the file"
        ))
      })?;
    let needed = usize::try_from(here * row_bytes)
      .map_err(|_| malformed(format!("strip {strip} of page {n} is too long to hold")))?;
    let turned: Vec<u8>;
    let data = match low_end_first {
      true => {
        turned = data.iter().map(|byte| byte.reverse_bits()).collect();
        turned.as_slice()
      }
      false => data,
    };
    let unpacked = || {
      malformed(format!(
        "strip {strip} of page {n} does not unpack to the rows it should hold"
      ))
    };
    let from = kept.len();
    match page.compression {
      Compression::Lzw => kept.extend(lzw(data, needed).ok_or_else(unpacked)?),
      Compression::Group4 => {
        let fax = fax(data, page.width, here).filter(|rows| rows.len() == needed);
        kept.extend(fax.ok_or_else(unpacked)?);
      }
      _ => kept.extend(data.get(..needed).ok_or_else(|| {
        malformed(format!(
          "strip {strip} of page {n} holds fewer bytes than its rows need"
        ))
      })?),
    }
    if as_differences && samples != Samples::Bit {
      let step = usize::try_from(samples.row_bytes(1)).unwrap_or(1);
      let row_length = usize::try_from(row_bytes).unwrap_or(usize::MAX).max(1);
      for row in kept[from..].chunks_mut(row_length) {
        for at in step..row.len() {
          row[at] = row[at].wrapping_add(row[at - step]);
        }
      }
    }
    rows += here;
  }
  if rows < height {
    return Err(malformed(format!(
      "the strips of page {n} hold {rows} of its {height} rows"
    )));
  }

  let short = || malformed(format!("page {n} holds fewer samples than its pixels need"));
  let image = match samples {
    Samples::Bit => {
      // What a set bit is: black where zero is white, which is how a fax is kept.
      let (unset, set) = match page.photometric {
        Some(Photometric::BlackIsZero) => (0, 255),
        _ => (255, 0),
      };
      let row_bytes = usize::try_from(row_bytes).map_err(|_| short())?;
      let across = usize::try_from(width).map_err(|_| short())?;
      let pixels: Vec<u8> = kept
        .chunks_exact(row_bytes.max(1))
        .flat_map(|row| {
          (0..across).map(move |at| match row[at / 8] & (0x80 >> (at % 8)) {
            0 => unset,
            _ => set,
          })
        })
        .collect();
      DynamicImage::ImageLuma8(
        GrayImage::from_raw(page.width, page.height, pixels).ok_or_else(short)?,
      )
    }
    Samples::Gray => {
      if page.photometric == Some(Photometric::WhiteIsZero) {
        kept.iter_mut().for_each(|sample| *sample = 255 - *sample);
      }
      DynamicImage::ImageLuma8(
        GrayImage::from_raw(page.width, page.height, kept).ok_or_else(short)?,
      )
    }
    Samples::Palette => {
      // Every red, then every green, then every blue, each of sixteen bits: the high eight
      // are the color.
      let table = numbers(320)
        .filter(|table| table.len() >= 3 * 256)
        .ok_or_else(|| {
          malformed(format!(
            "page {n} picks its colors from a table it does not have"
          ))
        })?;
      let color = |at: usize| u8::try_from(table[at] >> 8).unwrap_or(u8::MAX);
      let pixels: Vec<u8> = kept
        .iter()
        .flat_map(|picked| {
          let picked = usize::from(*picked);
          [color(picked), color(256 + picked), color(512 + picked)]
        })
        .collect();
      DynamicImage::ImageRgb8(
        RgbImage::from_raw(page.width, page.height, pixels).ok_or_else(short)?,
      )
    }
    Samples::Rgb => {
      DynamicImage::ImageRgb8(RgbImage::from_raw(page.width, page.height, kept).ok_or_else(short)?)
    }
    Samples::RgbAndOne { alpha: true } => DynamicImage::ImageRgba8(
      RgbaImage::from_raw(page.width, page.height, kept).ok_or_else(short)?,
    ),
    Samples::RgbAndOne { alpha: false } => {
      let pixels: Vec<u8> = kept
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[red, green, blue, _]| [*red, *green, *blue])
        .collect();
      DynamicImage::ImageRgb8(
        RgbImage::from_raw(page.width, page.height, pixels).ok_or_else(short)?,
      )
    }
  };
  Ok(as_looked_at(image, page))
}

/// How far apart a page's two resolutions may be and its pixels still count as square: a
/// machine that writes 204 across and 196 down means the same of both.
const SQUARE_WITHIN: f64 = 0.05;

/// The page as it is to be looked at: its pixels made square, and turned the way its
/// orientation says.
fn as_looked_at(image: DynamicImage, page: &Page) -> DynamicImage {
  use image::imageops::FilterType;
  use image::metadata::Orientation;

  // A fax can have fewer rows to the inch than pixels across. The sparser way is stretched to
  // match the denser, each row or pixel given again as the machine that prints it does.
  let stretched = |side: u32, by: f64| {
    let longer = (f64::from(side) * by).round();
    // Rounded, and no larger than a side can be.
    (longer.is_finite() && longer >= 1.0 && longer <= f64::from(u32::MAX)).then_some(longer as u32)
  };
  let mut image = match page.resolution {
    Some(resolution) if resolution.x > resolution.y * (1.0 + SQUARE_WITHIN) => {
      match stretched(image.height(), resolution.x / resolution.y) {
        Some(height) => image.resize_exact(image.width(), height, FilterType::Nearest),
        None => image,
      }
    }
    Some(resolution) if resolution.y > resolution.x * (1.0 + SQUARE_WITHIN) => {
      match stretched(image.width(), resolution.y / resolution.x) {
        Some(width) => image.resize_exact(width, image.height(), FilterType::Nearest),
        None => image,
      }
    }
    _ => image,
  };
  if let Some(turn) = Orientation::from_exif(page.orientation) {
    image.apply_orientation(turn);
  }
  image
}

/// What `data` unpacks to with LZW, when that is `needed` bytes or more. No more than `needed`
/// are made: packed data can claim to be any size.
fn lzw(data: &[u8], needed: usize) -> Option<Vec<u8>> {
  let mut decoder = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
  let mut out = vec![0; needed];
  let (mut read, mut written) = (0, 0);
  while written < needed {
    let step = decoder.decode_bytes(data.get(read..)?, &mut out[written..]);
    read += step.consumed_in;
    written += step.consumed_out;
    match step.status {
      Ok(weezl::LzwStatus::Ok) => {}
      Ok(weezl::LzwStatus::Done | weezl::LzwStatus::NoProgress) | Err(_) => break,
    }
  }
  (written == needed).then_some(out)
}

/// Rows from the fax decoder, packed a bit to a pixel with a set bit for black: what the same
/// rows would be kept as with no compression.
struct Packed {
  width: usize,
  row_bytes: usize,
  rows: Vec<u8>,
}

impl hayro_ccitt::Decoder for Packed {
  fn push_row(&mut self, changes: &[u32]) {
    let start = self.rows.len();
    self.rows.resize(start + self.row_bytes, 0);
    let row = &mut self.rows[start..];
    let place = |change: u32| {
      usize::try_from(change)
        .unwrap_or(usize::MAX)
        .min(self.width)
    };
    // A row starts white, and each change of color is listed: black runs from one to the next.
    for run in changes.chunks(2) {
      let to = run.get(1).map_or(self.width, |change| place(*change));
      for at in place(run[0])..to {
        row[at / 8] |= 0x80 >> (at % 8);
      }
    }
  }
}

/// The rows `data` decodes to as CCITT Group 4, packed a bit to a pixel. A strip's first row is
/// coded against a white row and not against the strip before it.
fn fax(data: &[u8], width: u32, rows: u64) -> Option<Vec<u8>> {
  use hayro_ccitt::{DecodeSettings, DecoderContext, EncodingMode};

  let across = usize::try_from(width).ok()?;
  let mut packed = Packed {
    width: across,
    row_bytes: across.div_ceil(8),
    rows: Vec::new(),
  };
  let settings = DecodeSettings {
    columns: width,
    rows: u32::try_from(rows).ok()?,
    end_of_block: true,
    end_of_line: false,
    rows_are_byte_aligned: false,
    encoding: EncodingMode::Group4,
    strict: false,
  };
  // An error is a damaged tail: the rows that came through are what there is, and the caller
  // counts them.
  let _ = hayro_ccitt::decode(data, &mut packed, &mut DecoderContext::new(settings));
  Some(packed.rows)
}

#[cfg(test)]
mod tests;
