//! TIFF's directories: a list of entries, each a tag, a type, a count and either the value or
//! where the value is. A TIFF file is this, and so is the EXIF block of a JPEG or a PNG.
//! BigTIFF is the same with offsets and counts of eight bytes.

use std::cell::Cell;

use crate::source::{Missing, Source};

/// A value whose bytes lie past the end of the file, read as a value that is not there. A
/// program that rewrites a file can leave a pointer dangling, and that takes with it what it
/// pointed at and nothing else. What was not given is still asked for. `lost` is set when a
/// value was left out this way.
pub(crate) fn if_there<T: Default>(
  read: Result<T, Missing>,
  lost: &Cell<bool>,
) -> Result<T, Missing> {
  match read {
    Err(Missing::PastEnd) => {
      lost.set(true);
      Ok(T::default())
    }
    read => read,
  }
}

/// No more entries than this are read from one directory, and no value longer than this.
const MOST_ENTRIES: u64 = 4096;
const LONGEST_VALUE: u64 = 8 << 20;

/// One entry of a directory.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
  pub tag: u16,
  kind: u16,
  count: u64,
  /// The value itself when it fits the entry, or else its offset.
  field: [u8; 8],
}

/// A directory: its entries and the offset of the directory that follows it, which is zero
/// when none does.
#[derive(Debug, Clone, Default)]
pub(crate) struct Directory {
  pub entries: Vec<Entry>,
  pub next: u64,
}

impl Directory {
  pub(crate) fn entry(&self, tag: u16) -> Option<&Entry> {
    self.entries.iter().find(|entry| entry.tag == tag)
  }
}

/// A TIFF structure in a source.
pub(crate) struct Tiff<'s, 'a> {
  source: &'s Source<'a>,
  pub little_endian: bool,
  /// BigTIFF: offsets and counts are eight bytes.
  pub big: bool,
  /// Where the first directory is.
  pub first: u64,
}

/// How many bytes one value of a type takes. Zero for a type that is not one.
fn width(kind: u16) -> u64 {
  match kind {
    1 | 2 | 6 | 7 => 1,
    3 | 8 => 2,
    4 | 9 | 11 | 13 => 4,
    5 | 10 | 12 | 16 | 17 | 18 => 8,
    _ => 0,
  }
}

impl<'s, 'a> Tiff<'s, 'a> {
  /// The structure `source` opens with, when it opens with one.
  pub(crate) fn open(source: &'s Source<'a>) -> Result<Option<Self>, Missing> {
    let [o0, o1, v0, v1, rest @ ..] = source.array::<8>(0)?;
    let little_endian = match [o0, o1] {
      [b'I', b'I'] => true,
      [b'M', b'M'] => false,
      _ => return Ok(None),
    };
    let mut tiff = Tiff {
      source,
      little_endian,
      big: false,
      first: 0,
    };
    match tiff.u16([v0, v1]) {
      42 => tiff.first = u64::from(tiff.u32(rest)),
      // The size of an offset, a zero, and then the first directory's offset in eight bytes.
      43 => {
        tiff.big = true;
        tiff.first = tiff.u64(source.array(8)?);
      }
      _ => return Ok(None),
    }
    Ok(Some(tiff))
  }

  /// The same directories under a header that is not a TIFF's, with the first of them at
  /// `first`. JPEG XR is this.
  pub(crate) fn at(source: &'s Source<'a>, little_endian: bool, first: u64) -> Self {
    Tiff {
      source,
      little_endian,
      big: false,
      first,
    }
  }

  fn u16(&self, bytes: [u8; 2]) -> u16 {
    if self.little_endian {
      u16::from_le_bytes(bytes)
    } else {
      u16::from_be_bytes(bytes)
    }
  }

  fn u32(&self, bytes: [u8; 4]) -> u32 {
    if self.little_endian {
      u32::from_le_bytes(bytes)
    } else {
      u32::from_be_bytes(bytes)
    }
  }

  fn u64(&self, bytes: [u8; 8]) -> u64 {
    if self.little_endian {
      u64::from_le_bytes(bytes)
    } else {
      u64::from_be_bytes(bytes)
    }
  }

  /// The directory at `offset`.
  pub(crate) fn directory(&self, offset: u64) -> Result<Directory, Missing> {
    let (count, entries_at, entry_bytes) = if self.big {
      (self.u64(self.source.array(offset)?), offset + 8, 20u64)
    } else {
      (
        u64::from(self.u16(self.source.array(offset)?)),
        offset + 2,
        12u64,
      )
    };
    let listed = count.min(MOST_ENTRIES);
    let table = self.source.read(
      entries_at,
      usize::try_from(listed * entry_bytes).unwrap_or(0),
    )?;
    let entries = table
      .chunks_exact(usize::try_from(entry_bytes).unwrap_or(12))
      .filter_map(|entry| {
        let tag = self.u16(*entry.first_chunk()?);
        let kind = self.u16(*entry.get(2..)?.first_chunk()?);
        let mut field = [0; 8];
        let count = if self.big {
          field.copy_from_slice(entry.get(12..20)?);
          self.u64(*entry.get(4..)?.first_chunk()?)
        } else {
          field[..4].copy_from_slice(entry.get(8..12)?);
          u64::from(self.u32(*entry.get(4..)?.first_chunk()?))
        };
        Some(Entry {
          tag,
          kind,
          count,
          field,
        })
      })
      .collect();
    // The count is the file's word. A directory that would end past the largest offset there
    // is ends past the end of any file.
    let next_at = count
      .checked_mul(entry_bytes)
      .and_then(|length| entries_at.checked_add(length))
      .ok_or(Missing::PastEnd)?;
    let next = if self.big {
      self.u64(self.source.array(next_at)?)
    } else {
      u64::from(self.u32(self.source.array(next_at)?))
    };
    Ok(Directory { entries, next })
  }

  /// The bytes of an entry's value: in the entry when they fit, at its offset when not.
  pub(crate) fn bytes(&self, entry: &Entry) -> Result<Vec<u8>, Missing> {
    let length = width(entry.kind)
      .saturating_mul(entry.count)
      .min(LONGEST_VALUE);
    let fits = if self.big { 8 } else { 4 };
    let wanted = usize::try_from(length).unwrap_or(0);
    if length <= fits {
      return Ok(entry.field[..wanted].to_vec());
    }
    let offset = if self.big {
      self.u64(entry.field)
    } else {
      let [a, b, c, d, ..] = entry.field;
      u64::from(self.u32([a, b, c, d]))
    };
    Ok(self.source.read(offset, wanted)?.to_vec())
  }

  /// An entry's values as whole numbers, whatever width they are written in.
  pub(crate) fn numbers(&self, entry: &Entry) -> Result<Vec<u64>, Missing> {
    let bytes = self.bytes(entry)?;
    Ok(match width(entry.kind) {
      1 => bytes.iter().map(|byte| u64::from(*byte)).collect(),
      2 => bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u64::from(self.u16(*v)))
        .collect(),
      4 => bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| u64::from(self.u32(*v)))
        .collect(),
      8 if entry.kind != 5 && entry.kind != 10 && entry.kind != 12 => bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|v| self.u64(*v))
        .collect(),
      _ => Vec::new(),
    })
  }

  /// An entry's first value as a whole number.
  pub(crate) fn number(&self, entry: &Entry) -> Result<Option<u64>, Missing> {
    Ok(self.numbers(entry)?.first().copied())
  }

  /// An entry's values as fractions: a numerator over a denominator, four bytes each.
  pub(crate) fn fractions(&self, entry: &Entry) -> Result<Vec<f64>, Missing> {
    if entry.kind != 5 {
      return Ok(self.numbers(entry)?.into_iter().map(|n| n as f64).collect());
    }
    let bytes = self.bytes(entry)?;
    Ok(
      bytes
        .as_chunks::<8>()
        .0
        .iter()
        .filter_map(|pair| {
          let [a, b, c, d, e, f, g, h] = *pair;
          let (over, under) = (self.u32([a, b, c, d]), self.u32([e, f, g, h]));
          (under != 0).then(|| f64::from(over) / f64::from(under))
        })
        .collect(),
    )
  }

  /// An entry's value as text, without the zero that ends it.
  pub(crate) fn text(&self, entry: &Entry) -> Result<Option<String>, Missing> {
    let bytes = self.bytes(entry)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    Ok((!text.is_empty()).then(|| text.to_string()))
  }
}

#[cfg(test)]
mod tests;
