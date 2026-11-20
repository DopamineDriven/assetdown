//! The directories of a TIFF. The file opens with eight bytes that say which way round its
//! numbers are written and where the first directory is; each directory is a list of tagged
//! entries and ends with where the next one is.

/// The file's bytes, read in the byte order its header states. Every read is checked against
/// the end of the file: an offset is the file's word, and a damaged file's word is wrong.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reader<'a> {
  bytes: &'a [u8],
  little: bool,
}

/// What a file opens with.
pub(crate) enum Opening<'a> {
  /// A TIFF, and where its first directory is.
  Classic {
    reader: Reader<'a>,
    first: u64,
  },
  /// The later form of the format, with eight bytes to an offset.
  Big,
  NotTiff,
}

/// What `bytes` opens with.
pub(crate) fn opening(bytes: &[u8]) -> Opening<'_> {
  let little = match bytes.first_chunk::<2>() {
    Some(b"II") => true,
    Some(b"MM") => false,
    _ => return Opening::NotTiff,
  };
  let reader = Reader { bytes, little };
  match (reader.u16(2), reader.u32(4)) {
    (Some(42), Some(first)) => Opening::Classic {
      reader,
      first: u64::from(first),
    },
    (Some(43), _) => Opening::Big,
    _ => Opening::NotTiff,
  }
}

/// No more whole numbers than this are read for one entry: a page of a million strips is no
/// page, and a damaged count is not a reason to read the whole file as numbers.
const MOST_NUMBERS: u32 = 1 << 20;

/// No more fractions than this are read for one entry. A resolution is one; nothing a page
/// states in fractions is more than a handful.
const MOST_FRACTIONS: u32 = 64;

/// No longer a text than this is read for one entry.
const MOST_TEXT: u32 = 1 << 16;

/// One entry of a directory: a tag, the type and number of its values, and the four bytes that
/// hold the values or say where they are.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Entry {
  pub tag: u16,
  kind: u16,
  count: u32,
  field: [u8; 4],
}

/// A directory: its entries, and where the next directory is. Zero is no next directory.
#[derive(Debug, Clone)]
pub(crate) struct Directory {
  entries: Vec<Entry>,
  pub next: u64,
}

impl Directory {
  pub(crate) fn entry(&self, tag: u16) -> Option<&Entry> {
    self.entries.iter().find(|entry| entry.tag == tag)
  }
}

impl<'a> Reader<'a> {
  /// A reader of `bytes`, whose numbers run low byte first or high byte first.
  pub(crate) fn over(bytes: &'a [u8], little: bool) -> Self {
    Reader { bytes, little }
  }

  /// Whether the file's numbers run low byte first.
  pub(crate) fn is_little(&self) -> bool {
    self.little
  }

  /// How many bytes the file has.
  pub(crate) fn len(&self) -> u64 {
    u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
  }

  /// `length` bytes of the file from `offset`. None when they do not all lie within it.
  pub(crate) fn stretch(&self, offset: u64, length: u64) -> Option<&'a [u8]> {
    let start = usize::try_from(offset).ok()?;
    let end = start.checked_add(usize::try_from(length).ok()?)?;
    self.bytes.get(start..end)
  }

  fn array<const N: usize>(&self, at: u64) -> Option<[u8; N]> {
    let start = usize::try_from(at).ok()?;
    self.bytes.get(start..)?.first_chunk::<N>().copied()
  }

  fn u16(&self, at: u64) -> Option<u16> {
    let bytes = self.array(at)?;
    Some(if self.little {
      u16::from_le_bytes(bytes)
    } else {
      u16::from_be_bytes(bytes)
    })
  }

  fn u32(&self, at: u64) -> Option<u32> {
    let bytes = self.array(at)?;
    Some(if self.little {
      u32::from_le_bytes(bytes)
    } else {
      u32::from_be_bytes(bytes)
    })
  }

  /// How many entries the directory at `at` says it has. None when that does not lie within
  /// the file.
  pub(crate) fn entries_at(&self, at: u64) -> Option<u64> {
    self.u16(at).map(u64::from)
  }

  /// The directory at `at`. None when it does not lie within the file.
  pub(crate) fn directory(&self, at: u64) -> Option<Directory> {
    let count = u64::from(self.u16(at)?);
    let entries_at = at.checked_add(2)?;
    let entries = (0..count)
      .map(|index| {
        let entry_at = entries_at.checked_add(index * 12)?;
        Some(Entry {
          tag: self.u16(entry_at)?,
          kind: self.u16(entry_at.checked_add(2)?)?,
          count: self.u32(entry_at.checked_add(4)?)?,
          field: self.array(entry_at.checked_add(8)?)?,
        })
      })
      .collect::<Option<Vec<Entry>>>()?;
    let next = self.u32(entries_at.checked_add(count * 12)?)?;
    Some(Directory {
      entries,
      next: u64::from(next),
    })
  }

  /// An entry's values as whole numbers, when they are whole numbers of one, two, or four
  /// bytes. They stand in the entry when they fit its four bytes, and where it points when not.
  pub(crate) fn numbers(&self, entry: &Entry) -> Option<Vec<u64>> {
    let width: u64 = match entry.kind {
      1 => 1,
      3 => 2,
      4 => 4,
      _ => return None,
    };
    if entry.count > MOST_NUMBERS {
      return None;
    }
    let count = u64::from(entry.count);
    let held = Reader {
      bytes: &entry.field,
      little: self.little,
    };
    let (from, start) = match count.checked_mul(width)? <= 4 {
      true => (held, 0),
      false => (*self, u64::from(self.u32_of(entry.field))),
    };
    (0..count)
      .map(|index| {
        let at = start.checked_add(index * width)?;
        match width {
          1 => from.array::<1>(at).map(|[byte]| u64::from(byte)),
          2 => from.u16(at).map(u64::from),
          _ => from.u32(at).map(u64::from),
        }
      })
      .collect()
  }

  /// An entry's value as text: bytes up to the zero that ends them, each a character. None
  /// when the entry is not text, the text does not lie within the file, or nothing is written
  /// in it.
  pub(crate) fn text(&self, entry: &Entry) -> Option<String> {
    if entry.kind != 2 || entry.count > MOST_TEXT {
      return None;
    }
    let length = usize::try_from(entry.count).ok()?;
    let bytes = match length <= 4 {
      true => entry.field.get(..length)?,
      false => {
        let start = usize::try_from(self.u32_of(entry.field)).ok()?;
        self.bytes.get(start..start.checked_add(length)?)?
      }
    };
    let written: String = bytes
      .iter()
      .take_while(|byte| **byte != 0)
      .map(|byte| char::from(*byte))
      .collect();
    let written = written.trim();
    (!written.is_empty()).then(|| written.to_string())
  }

  /// An entry's values as fractions, each a numerator over a denominator of four bytes apiece.
  /// Eight bytes never fit an entry, so they stand where it points.
  pub(crate) fn fractions(&self, entry: &Entry) -> Option<Vec<(u32, u32)>> {
    if entry.kind != 5 || entry.count > MOST_FRACTIONS {
      return None;
    }
    let start = u64::from(self.u32_of(entry.field));
    (0..u64::from(entry.count))
      .map(|index| {
        let at = start.checked_add(index.checked_mul(8)?)?;
        Some((self.u32(at)?, self.u32(at.checked_add(4)?)?))
      })
      .collect()
  }

  fn u32_of(&self, bytes: [u8; 4]) -> u32 {
    if self.little {
      u32::from_le_bytes(bytes)
    } else {
      u32::from_be_bytes(bytes)
    }
  }
}

#[cfg(test)]
mod tests;
