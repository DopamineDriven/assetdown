//! A document's pictures, as their bytes. A picture set in the text is kept in the `Data`
//! stream, at a place the properties of its mark in the text state. The picture of a shape
//! that floats over the text is listed in the store of the drawing data, which says where
//! in the main stream it is kept. Either way the picture is a record of the drawing format,
//! and a metafile in one is kept compressed.

use std::collections::HashSet;
use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::pieces::Unit;
use crate::props::{Stated, properties};

/// The mark in the text that stands for a picture.
const PICTURE: u16 = 0x01;
/// A metafile may not inflate to more than this many bytes.
const MAX_METAFILE: u64 = 1 << 28;
/// The kinds of record a drawing is made of that hold others, the entry of the store that
/// goes before a picture, and the store itself.
const HOLDERS: [u16; 5] = [0xF000, 0xF001, 0xF002, 0xF003, 0xF004];
const STORE_ENTRY: u16 = 0xF007;

/// What a picture's bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureFormat {
  Png,
  Jpeg,
  Tiff,
  /// An enhanced metafile: drawing commands, not pixels.
  Emf,
  /// A Windows metafile.
  Wmf,
  /// A bitmap with no file header, as Windows keeps one in memory.
  Dib,
  /// A QuickDraw picture.
  MacPict,
}

/// A picture of the document, as its bytes. A metafile is not drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
  pub format: PictureFormat,
  pub bytes: Vec<u8>,
  /// The width and height the picture is shown at, in twentieths of a point, where the file
  /// states both: a picture set in the text states them, and a floating one does not.
  pub goal: Option<(i32, i32)>,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// One record of a drawing: its instance, its kind, and its data.
struct Drawn<'a> {
  instance: u16,
  kind: u16,
  data: &'a [u8],
}

/// The record `bytes` opens with, and what follows it.
fn drawn(bytes: &[u8]) -> Option<(Drawn<'_>, &[u8])> {
  let length = usize::try_from(u32_at(bytes, 4)?).ok()?;
  let data = bytes.get(8..8usize.checked_add(length)?)?;
  let record = Drawn {
    instance: u16_at(bytes, 0)? >> 4,
    kind: u16_at(bytes, 2)?,
    data,
  };
  Some((record, &bytes[8 + length..]))
}

/// The picture a record holds, when it is one. The record opens with a mark of who the
/// picture is, or two of them, which the record's instance tells apart. A bitmap follows a
/// byte after that; a metafile follows 34 bytes that say how long it is and how it is kept.
fn picture(record: &Drawn<'_>) -> Option<(PictureFormat, Vec<u8>)> {
  let (format, twice, metafile) = match record.kind {
    0xF01A => (PictureFormat::Emf, 0x3D5, true),
    0xF01B => (PictureFormat::Wmf, 0x217, true),
    0xF01C => (PictureFormat::MacPict, 0x543, true),
    0xF01D => (PictureFormat::Jpeg, 0x46B, false),
    0xF02A => (PictureFormat::Jpeg, 0x6E3, false),
    0xF01E => (PictureFormat::Png, 0x6E1, false),
    0xF01F => (PictureFormat::Dib, 0x7A9, false),
    0xF029 => (PictureFormat::Tiff, 0x6E5, false),
    _ => return None,
  };
  let marks = if record.instance == twice { 32 } else { 16 };
  if !metafile {
    return Some((format, record.data.get(marks + 1..)?.to_vec()));
  }
  let kept = record.data.get(marks + 34..)?;
  let compressed = *record.data.get(marks + 32)? == 0;
  if !compressed {
    return Some((format, kept.to_vec()));
  }
  let mut whole = Vec::new();
  ZlibDecoder::new(kept)
    .take(MAX_METAFILE)
    .read_to_end(&mut whole)
    .ok()?;
  Some((format, whole))
}

/// Every picture in a run of drawing records, in order: those it holds itself, and those of
/// the records it holds. `delayed` is the stream a store's entry points into for a picture
/// it does not hold itself.
fn pictures_in(records: &[u8], delayed: Option<&[u8]>) -> Vec<(PictureFormat, Vec<u8>)> {
  let mut found = Vec::new();
  // What is still to be looked through, the first of it last, so that a record that holds
  // others is gone through before what follows it.
  let mut open: Vec<&[u8]> = vec![records];
  while let Some(held) = open.pop() {
    let Some((record, rest)) = drawn(held) else {
      continue;
    };
    open.push(rest);
    if let Some(picture) = picture(&record) {
      found.push(picture);
    } else if HOLDERS.contains(&record.kind) {
      open.push(record.data);
    } else if record.kind == STORE_ENTRY {
      // 36 bytes about the picture, a name, and then the picture, when the entry holds it.
      // When it does not, the 28th byte on says where it is kept in the other stream.
      let name = record.data.get(33).map_or(0, |length| usize::from(*length));
      let within = record.data.get(36 + name..).filter(|rest| !rest.is_empty());
      let elsewhere = || {
        let at = usize::try_from(u32_at(record.data, 28)?).ok()?;
        delayed?.get(at..)
      };
      // Only the picture itself is taken from there, and not what follows it.
      if let Some((record, _)) = within.or_else(elsewhere).and_then(drawn) {
        found.extend(picture(&record));
      }
    }
  }
  found
}

/// A picture set in the text, from where its mark says it is in the `Data` stream: its
/// length, the length of its header, what kind it is, and in the header the size it is to
/// be shown at and how far each way it is scaled, in thousandths.
fn inline(data: &[u8], at: usize) -> Option<Picture> {
  let length = usize::try_from(u32_at(data, at)?).ok()?;
  let kept = data.get(at..at.checked_add(length)?)?;
  let header = usize::from(u16_at(kept, 4)?);
  let goal = (|| {
    let side = |at: usize, scale: usize| {
      let length = i32::from(u16_at(kept, at)?.cast_signed());
      let scale = i32::from(u16_at(kept, scale)?);
      Some(length * scale / 1000).filter(|shown| *shown > 0)
    };
    Some((side(0x1C, 0x20)?, side(0x1E, 0x22)?))
  })();
  let records = match u16_at(kept, 6)? {
    // A shape, which holds the picture.
    0x64 => kept.get(header..)?,
    // The same, after the name of the file the picture is linked to, with its length.
    0x66 => {
      let name = usize::from(*kept.get(header)?);
      kept.get(header + 1 + name..)?
    }
    // A Windows metafile as it is, from before pictures were kept in shapes.
    0x08 => {
      return Some(Picture {
        format: PictureFormat::Wmf,
        bytes: kept.get(header..)?.to_vec(),
        goal,
      });
    }
    _ => return None,
  };
  let (format, bytes) = pictures_in(records, None).into_iter().next()?;
  Some(Picture {
    format,
    bytes,
    goal,
  })
}

/// The document's pictures: those set in the text, in the order of their marks, and then
/// those of floating shapes, in the order of the drawing's store. The picture that stands
/// for an embedded object, or for the data of a form's field, is not among them.
pub(crate) fn read(
  units: &[Unit],
  characters: &Stated<'_>,
  data: Option<&[u8]>,
  drawings: Option<&[u8]>,
  main: &[u8],
) -> Vec<Picture> {
  let mut found = Vec::new();
  let mut seen = HashSet::new();
  for unit in units.iter().filter(|unit| unit.code == PICTURE) {
    let mut place = None;
    let mut stands_for_other = false;
    for property in properties(characters.at(unit.at)) {
      match property.code {
        // Where the picture is in the `Data` stream.
        0x6A03 => {
          place = property
            .value
            .get(..4)
            .and_then(|at| usize::try_from(u32::from_le_bytes(at.try_into().ok()?)).ok());
        }
        // The mark is an embedded object's, or a form field's data.
        0x080A | 0x0856 | 0x0806 => stands_for_other |= property.u8() != Some(0),
        _ => {}
      }
    }
    if let (Some(place), false, Some(data)) = (place, stands_for_other, data)
      && seen.insert(place)
    {
      found.extend(inline(data, place));
    }
  }
  // The drawing data opens with what the drawings share, the store among it.
  let shared = drawings
    .and_then(drawn)
    .filter(|(record, _)| record.kind == 0xF000);
  if let Some((shared, _)) = shared {
    let floating = pictures_in(shared.data, Some(main));
    found.extend(floating.into_iter().map(|(format, bytes)| Picture {
      format,
      bytes,
      goal: None,
    }));
  }
  found
}
