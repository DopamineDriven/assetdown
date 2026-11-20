//! A compound file: the container an embedded object is kept in. It is a small file system of
//! its own, with sectors chained through an allocation table and a directory of named streams.
//! Only what an embedded object needs is read here: every stream by its name, whichever
//! storage it lies in.

/// One stream of a compound file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stream {
  pub name: String,
  pub bytes: Vec<u8>,
}

const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// The last sector of a chain points here.
const END: u32 = 0xFFFF_FFFE;
/// No more directory entries than this are listed. An attached file has a handful.
const MOST_ENTRIES: usize = 4096;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The file's sectors and the table that chains them.
struct Sectors<'a> {
  bytes: &'a [u8],
  size: usize,
  table: Vec<u32>,
}

impl<'a> Sectors<'a> {
  /// The bytes of one sector. The first lies after a header as long as a sector.
  fn sector(&self, number: u32) -> Option<&'a [u8]> {
    let at = usize::try_from(number)
      .ok()?
      .checked_add(1)?
      .checked_mul(self.size)?;
    self.bytes.get(at..at.checked_add(self.size)?)
  }

  /// The sectors of a chain, in order. None when it leaves the file, or runs longer than the
  /// file has sectors, which a chain that leads back on itself does.
  fn chain(&self, first: u32) -> Option<Vec<u32>> {
    let most = self.bytes.len() / self.size;
    let mut chain = Vec::new();
    let mut next = first;
    while next != END {
      if chain.len() >= most {
        return None;
      }
      chain.push(next);
      next = *self.table.get(usize::try_from(next).ok()?)?;
    }
    Some(chain)
  }

  /// `length` bytes from the chain that starts at `first`.
  fn read(&self, first: u32, length: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for number in self.chain(first)? {
      if out.len() >= length {
        break;
      }
      out.extend_from_slice(self.sector(number)?);
    }
    (out.len() >= length).then(|| {
      out.truncate(length);
      out
    })
  }
}

/// The streams of the compound file in `bytes`, each with its name. None when the bytes are
/// not a compound file, or its tables cannot be read. A stream whose own chain is broken, or
/// that is said to be longer than its sectors, is left out and the rest are still given.
pub(crate) fn streams(bytes: &[u8]) -> Option<Vec<Stream>> {
  if bytes.get(..8)? != SIGNATURE {
    return None;
  }
  let shift = u16_at(bytes, 0x1E)?;
  let mini_shift = u16_at(bytes, 0x20)?;
  // Sectors are 512 bytes or 4,096; mini sectors are 64.
  if !matches!(shift, 9 | 12) || mini_shift != 6 {
    return None;
  }
  let size = 1usize << shift;
  let mini_size = 1usize << mini_shift;
  if bytes.len() < size {
    return None;
  }
  let cutoff = usize::try_from(u32_at(bytes, 0x38)?).ok()?;
  let per_sector = size / 4;

  // The table's own sectors are listed in the header, and past 109 of them in sectors chained
  // from it, each ending with the number of the next.
  let mut sectors = Sectors {
    bytes,
    size,
    table: Vec::new(),
  };
  let most = bytes.len() / size;
  let mut table_sectors: Vec<u32> = (0..109)
    .filter_map(|slot| u32_at(bytes, 0x4C + slot * 4))
    .filter(|number| *number < END - 3)
    .collect();
  let mut more = u32_at(bytes, 0x44)?;
  let mut walked = 0;
  while more < END - 3 {
    walked += 1;
    if walked > most {
      return None;
    }
    let listing = sectors.sector(more)?;
    for slot in 0..per_sector - 1 {
      let number = u32_at(listing, slot * 4)?;
      if number < END - 3 {
        table_sectors.push(number);
      }
    }
    more = u32_at(listing, (per_sector - 1) * 4)?;
  }
  if table_sectors.len() > most {
    return None;
  }
  let mut table = Vec::with_capacity(table_sectors.len() * per_sector);
  for number in table_sectors {
    let sector = sectors.sector(number)?;
    table.extend((0..per_sector).filter_map(|slot| u32_at(sector, slot * 4)));
  }
  sectors.table = table;

  // The directory: entries of 128 bytes, in a chain of sectors.
  let mut directory = Vec::new();
  for number in sectors.chain(u32_at(bytes, 0x30)?)? {
    directory.extend_from_slice(sectors.sector(number)?);
  }
  let entries: Vec<&[u8]> = directory
    .as_chunks::<128>()
    .0
    .iter()
    .map(|entry| entry.as_slice())
    .take(MOST_ENTRIES)
    .collect();

  // Streams under the cutoff are kept in the mini stream, which is the root entry's own
  // stream, in sectors of 64 bytes chained through a table of their own.
  let root = entries.iter().find(|entry| entry[0x42] == 5)?;
  let mini_stream = match (
    u32_at(root, 0x74)?,
    usize::try_from(u32_at(root, 0x78)?).ok()?,
  ) {
    (_, 0) => Vec::new(),
    (first, length) => sectors.read(first, length)?,
  };
  let mut mini_table = Vec::new();
  for number in sectors.chain(u32_at(bytes, 0x3C)?)? {
    let sector = sectors.sector(number)?;
    mini_table.extend((0..per_sector).filter_map(|slot| u32_at(sector, slot * 4)));
  }
  let mini = |first: u32, length: usize| -> Option<Vec<u8>> {
    let most = mini_stream.len() / mini_size;
    let mut out = Vec::new();
    let mut next = first;
    let mut walked = 0;
    while next != END && out.len() < length {
      walked += 1;
      if walked > most {
        return None;
      }
      let at = usize::try_from(next).ok()?.checked_mul(mini_size)?;
      out.extend_from_slice(mini_stream.get(at..at + mini_size)?);
      next = *mini_table.get(usize::try_from(next).ok()?)?;
    }
    (out.len() >= length).then(|| {
      out.truncate(length);
      out
    })
  };

  let mut found = Vec::new();
  for entry in entries.iter().filter(|entry| entry[0x42] == 2) {
    let named = usize::from(u16_at(entry, 0x40)?).min(64);
    let units: Vec<u16> = entry[..named]
      .as_chunks::<2>()
      .0
      .iter()
      .map(|pair| u16::from_le_bytes(*pair))
      .take_while(|unit| *unit != 0)
      .collect();
    let name = String::from_utf16_lossy(&units);
    let first = u32_at(entry, 0x74)?;
    // A file with sectors of 512 bytes keeps a stream's length in four bytes.
    let Ok(length) = usize::try_from(u32_at(entry, 0x78)?) else {
      continue;
    };
    let bytes = match length {
      0 => Some(Vec::new()),
      length if length < cutoff => mini(first, length),
      length => sectors.read(first, length),
    };
    if let Some(bytes) = bytes {
      found.push(Stream { name, bytes });
    }
  }
  Some(found)
}

#[cfg(test)]
mod tests;
