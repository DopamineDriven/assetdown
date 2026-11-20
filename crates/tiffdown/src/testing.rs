//! Files for the tests, built by hand: a TIFF is a header and a chain of directories, and each
//! directory is a list of tagged values.

/// The value of one entry.
pub(crate) enum Value {
  Short(Vec<u16>),
  Long(Vec<u32>),
  /// Text, which the file ends with a zero byte.
  Text(&'static str),
  /// Numerators over denominators.
  Fractions(Vec<(u32, u32)>),
  /// A page's data in pieces, for the entry that lists where its strips or its tiles are. The
  /// pieces are written after the directory, and the entry that says how long each one is
  /// (279 for strips, 325 for tiles) is written with it.
  Pieces(Vec<Vec<u8>>),
}

fn two(little: bool, value: u16) -> [u8; 2] {
  if little {
    value.to_le_bytes()
  } else {
    value.to_be_bytes()
  }
}

fn four(little: bool, value: u32) -> [u8; 4] {
  if little {
    value.to_le_bytes()
  } else {
    value.to_be_bytes()
  }
}

/// The number the format gives a value's type, how many values there are, and their bytes.
fn written(little: bool, value: &Value) -> (u16, u32, Vec<u8>) {
  let count = |length: usize| u32::try_from(length).unwrap();
  match value {
    Value::Short(values) => (
      3,
      count(values.len()),
      values.iter().flat_map(|v| two(little, *v)).collect(),
    ),
    Value::Long(values) => (
      4,
      count(values.len()),
      values.iter().flat_map(|v| four(little, *v)).collect(),
    ),
    Value::Text(text) => (2, count(text.len() + 1), [text.as_bytes(), &[0]].concat()),
    Value::Fractions(values) => (
      5,
      count(values.len()),
      values
        .iter()
        .flat_map(|(over, under)| [four(little, *over), four(little, *under)].concat())
        .collect(),
    ),
    Value::Pieces(_) => unreachable!("pieces are placed by the builder"),
  }
}

/// A TIFF of these directories, chained in order. A value too long for its entry stands after
/// the directory that names it. The first directory is at offset 8.
pub(crate) fn tiff(little: bool, directories: &[Vec<(u16, Value)>]) -> Vec<u8> {
  let mut file = [
    if little { &b"II"[..] } else { &b"MM"[..] },
    &two(little, 42),
    &four(little, 8),
  ]
  .concat();
  for (index, entries) in directories.iter().enumerate() {
    let at = file.len();
    // The lengths of a page's pieces are an entry of their own.
    let lengths: Vec<(u16, Value)> = entries
      .iter()
      .filter_map(|(tag, value)| match value {
        Value::Pieces(pieces) => {
          let lengths = pieces
            .iter()
            .map(|piece| u32::try_from(piece.len()).unwrap());
          Some((
            if *tag == 273 { 279 } else { 325 },
            Value::Long(lengths.collect()),
          ))
        }
        _ => None,
      })
      .collect();
    let mut entries: Vec<&(u16, Value)> = entries.iter().chain(&lengths).collect();
    entries.sort_by_key(|(tag, _)| *tag);
    let length = 2 + 12 * entries.len() + 4;
    let mut directory = two(little, u16::try_from(entries.len()).unwrap()).to_vec();
    let mut after = Vec::new();
    for (tag, value) in entries {
      // A page's pieces go after the directory first, and the entry lists where each landed.
      let placed;
      let value = match value {
        Value::Pieces(pieces) => {
          let mut offsets = Vec::new();
          for piece in pieces {
            offsets.push(u32::try_from(at + length + after.len()).unwrap());
            after.extend(piece);
            if after.len() % 2 == 1 {
              after.push(0);
            }
          }
          placed = Value::Long(offsets);
          &placed
        }
        other => other,
      };
      let (kind, count, mut bytes) = written(little, value);
      directory.extend(two(little, *tag));
      directory.extend(two(little, kind));
      directory.extend(four(little, count));
      if bytes.len() <= 4 {
        bytes.resize(4, 0);
        directory.extend(bytes);
      } else {
        let offset = u32::try_from(at + length + after.len()).unwrap();
        directory.extend(four(little, offset));
        after.extend(bytes);
        // Values start on an even offset.
        if after.len() % 2 == 1 {
          after.push(0);
        }
      }
    }
    let last = index + 1 == directories.len();
    let next = if last { 0 } else { at + length + after.len() };
    directory.extend(four(little, u32::try_from(next).unwrap()));
    file.extend(directory);
    file.extend(after);
  }
  file
}

/// A page of this size and nothing else said of it.
pub(crate) fn sized(width: u32, height: u32) -> Vec<(u16, Value)> {
  vec![
    (256, Value::Long(vec![width])),
    (257, Value::Long(vec![height])),
  ]
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
