//! Files for the tests. A JPEG's header is a run of marked segments, which are written out by
//! hand here; a picture that has to decode is made by the `image` crate's own encoder.

use image::DynamicImage;
use image::codecs::jpeg::{JpegEncoder, PixelDensity};

/// A segment: its marker, its length, and what it holds.
pub(crate) fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
  let length = u16::try_from(payload.len() + 2).unwrap();
  [&[0xFF, marker][..], &length.to_be_bytes(), payload].concat()
}

/// The JFIF segment: a density, and the unit it is in (0 none, 1 to the inch, 2 to the
/// centimeter).
pub(crate) fn jfif(unit: u8, across: u16, down: u16) -> Vec<u8> {
  let payload = [
    &b"JFIF\0\x01\x02"[..],
    &[unit],
    &across.to_be_bytes(),
    &down.to_be_bytes(),
    &[0, 0],
  ]
  .concat();
  segment(0xE0, &payload)
}

/// A frame header: which coding (the marker), how deep a sample is, the size, and how many
/// components a pixel has.
pub(crate) fn frame(marker: u8, precision: u8, width: u16, height: u16, components: u8) -> Vec<u8> {
  let mut payload = vec![precision];
  payload.extend(height.to_be_bytes());
  payload.extend(width.to_be_bytes());
  payload.push(components);
  for id in 1..=components {
    payload.extend([id, 0x11, 0]);
  }
  segment(marker, &payload)
}

/// A value of the Exif directory.
pub(crate) enum Exif {
  Short(u16),
  /// A numerator over a denominator.
  Fraction(u32, u32),
  Text(&'static str),
}

/// The Exif segment, in either byte order, with these entries in its first directory.
pub(crate) fn exif(little: bool, entries: &[(u16, Exif)]) -> Vec<u8> {
  let two = |v: u16| {
    if little {
      v.to_le_bytes()
    } else {
      v.to_be_bytes()
    }
  };
  let four = |v: u32| {
    if little {
      v.to_le_bytes()
    } else {
      v.to_be_bytes()
    }
  };
  let mut tiff = [
    if little { &b"II"[..] } else { &b"MM"[..] },
    &two(42),
    &four(8),
  ]
  .concat();
  let after = 8 + 2 + 12 * entries.len() + 4;
  let mut long = Vec::new();
  tiff.extend(two(u16::try_from(entries.len()).unwrap()));
  for (tag, value) in entries {
    tiff.extend(two(*tag));
    let place = four(u32::try_from(after + long.len()).unwrap());
    match value {
      Exif::Short(v) => {
        tiff.extend([&two(3)[..], &four(1), &two(*v), &[0, 0]].concat());
      }
      Exif::Fraction(over, under) => {
        tiff.extend([&two(5)[..], &four(1), &place].concat());
        long.extend([four(*over), four(*under)].concat());
      }
      Exif::Text(text) => {
        let bytes = [text.as_bytes(), &[0]].concat();
        tiff.extend(
          [
            &two(2)[..],
            &four(u32::try_from(bytes.len()).unwrap()),
            &place,
          ]
          .concat(),
        );
        long.extend(bytes);
      }
    }
  }
  tiff.extend(four(0));
  tiff.extend(long);
  segment(0xE1, &[&b"Exif\0\0"[..], &tiff].concat())
}

/// A file of these segments, opened and closed as a JPEG is, with the start of a scan after
/// them and no picture data.
pub(crate) fn file(segments: &[Vec<u8>]) -> Vec<u8> {
  let scan = segment(0xDA, &[1, 1, 0, 0, 63, 0]);
  [&[0xFF, 0xD8][..], &segments.concat(), &scan, &[0xFF, 0xD9]].concat()
}

/// A picture as a real JPEG, with a density in pixels to the inch when one is given.
pub(crate) fn encoded(image: &DynamicImage, dpi: Option<u16>) -> Vec<u8> {
  let mut out = Vec::new();
  let mut encoder = JpegEncoder::new_with_quality(&mut out, 92);
  if let Some(dpi) = dpi {
    encoder.set_pixel_density(PixelDensity::dpi(dpi));
  }
  // Written by its own kind, so that a picture of gray is kept as gray.
  image.write_with_encoder(encoder).unwrap();
  out
}

/// A JPEG with one more segment put in straight after its opening marker.
pub(crate) fn with_segment(jpeg: &[u8], segment: &[u8]) -> Vec<u8> {
  [&jpeg[..2], segment, &jpeg[2..]].concat()
}

/// Five dots across and seven down, a `#` where there is ink.
#[cfg(feature = "ocr")]
fn glyph(letter: char) -> [&'static str; 7] {
  match letter {
    'T' => [
      "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
    ],
    'O' => [
      ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
    ],
    'A' => [
      ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
    ],
    'L' => [
      "#....", "#....", "#....", "#....", "#....", "#....", "#####",
    ],
    '4' => [
      "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#.",
    ],
    '2' => [
      ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
    ],
    ' ' => ["....."; 7],
    other => panic!("no glyph is drawn for {other:?}"),
  }
}

/// A white picture of `size` with `text` in black, its top left corner at `at`, in a small
/// block face that Tesseract reads. Each dot of a glyph is `dot` pixels square, and a glyph
/// with its gap is six dots wide.
#[cfg(feature = "ocr")]
pub(crate) fn drawn(text: &str, dot: u32, size: (u32, u32), at: (u32, u32)) -> image::GrayImage {
  use image::{GrayImage, Luma};

  let mut picture = GrayImage::from_pixel(size.0, size.1, Luma([255]));
  for (place, letter) in (0u32..).zip(text.chars()) {
    for (row, dots) in (0u32..).zip(glyph(letter)) {
      for (column, _) in (0u32..).zip(dots.chars()).filter(|(_, dot)| *dot == '#') {
        for (down, across) in (0..dot).flat_map(|down| (0..dot).map(move |across| (down, across))) {
          picture.put_pixel(
            at.0 + (place * 6 + column) * dot + across,
            at.1 + row * dot + down,
            Luma([0]),
          );
        }
      }
    }
  }
  picture
}
