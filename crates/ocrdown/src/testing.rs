//! Pictures for the tests: words drawn in a small block face, which Tesseract reads.

use image::{DynamicImage, GrayImage, Luma};

/// Five dots across and seven down, a `#` where there is ink.
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

/// A white picture of `size` with `text` in black, its top left corner at `at`. Each dot of a
/// glyph is `dot` pixels square, and a glyph with its gap is six dots wide.
pub(crate) fn drawn(text: &str, dot: u32, size: (u32, u32), at: (u32, u32)) -> DynamicImage {
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
  DynamicImage::ImageLuma8(picture)
}
