//! Reading a compound file: its header, the table that chains its sectors, and its directory.

use crate::error::CfbError;
use crate::write::compound;

/// One stream of a compound file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
  pub name: String,
  pub bytes: Vec<u8>,
}

/// One entry of a storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Child<'f> {
  pub name: &'f str,
  /// A storage, which holds entries of its own. Otherwise a stream.
  pub is_storage: bool,
}

const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// The last sector of a chain points here.
const END: u32 = 0xFFFF_FFFE;
/// Sector numbers from here up are marks, not sectors.
const MARKS: u32 = END - 3;

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

  /// The sectors of a file whose sectors are `size` bytes, with the table that chains them.
  /// The table's own sectors are listed in the header, and past 109 of them in sectors chained
  /// from it, each ending with the number of the next.
  fn of(bytes: &'a [u8], size: usize) -> Option<Self> {
    let per_sector = size / 4;
    let mut sectors = Sectors {
      bytes,
      size,
      table: Vec::new(),
    };
    let most = bytes.len() / size;
    let mut table_sectors: Vec<u32> = (0..109)
      .filter_map(|slot| u32_at(bytes, 0x4C + slot * 4))
      .filter(|number| *number < MARKS)
      .collect();
    let mut more = u32_at(bytes, 0x44)?;
    let mut walked = 0;
    while more < MARKS {
      walked += 1;
      if walked > most {
        return None;
      }
      let listing = sectors.sector(more)?;
      for slot in 0..per_sector - 1 {
        let number = u32_at(listing, slot * 4)?;
        if number < MARKS {
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
    Some(sectors)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
  Root,
  Storage,
  Stream,
  /// An entry of the directory that is not in use.
  Unused,
}

/// One entry of the directory.
struct Entry {
  name: String,
  kind: Kind,
  /// The entries a storage holds, by their numbers. Empty for a stream, and for an entry
  /// nothing leads to.
  children: Vec<usize>,
  first: u32,
  /// None when the stream is said to be longer than this machine can address.
  length: Option<usize>,
}

/// An entry as the directory writes it: the numbers of the entries to its left and its right
/// among those of its storage, and of one entry of those it holds.
struct Links {
  left: u32,
  right: u32,
  child: u32,
}

/// Whether two names are the same to the format, which does not tell upper case from lower.
fn same_name(a: &str, b: &str) -> bool {
  a.chars()
    .flat_map(char::to_uppercase)
    .eq(b.chars().flat_map(char::to_uppercase))
}

/// An open compound file.
pub struct CompoundFile<'a> {
  sectors: Sectors<'a>,
  /// A stream shorter than this is kept in the mini stream.
  cutoff: usize,
  /// The root entry's own stream: the small streams, in sectors of 64 bytes.
  mini_stream: Vec<u8>,
  mini_table: Vec<u32>,
  entries: Vec<Entry>,
  root: usize,
}

const MINI_SECTOR: usize = 64;

impl<'a> CompoundFile<'a> {
  /// Read the header, the tables, and the directory of the compound file in `bytes`. No
  /// stream is read until it is asked for.
  pub fn open(bytes: &'a [u8]) -> Result<CompoundFile<'a>, CfbError> {
    if bytes.get(..8) != Some(&SIGNATURE[..]) {
      return Err(CfbError::NotCompound);
    }
    let header = (|| {
      let shift = u16_at(bytes, 0x1E)?;
      let mini_shift = u16_at(bytes, 0x20)?;
      // Sectors are 512 bytes or 4,096; mini sectors are 64.
      if !matches!(shift, 9 | 12) || mini_shift != 6 {
        return None;
      }
      let size = 1usize << shift;
      let cutoff = usize::try_from(u32_at(bytes, 0x38)?).ok()?;
      (bytes.len() >= size).then_some((size, cutoff))
    })();
    let (size, cutoff) = header.ok_or(CfbError::Malformed("its header cannot be read"))?;
    let sectors =
      Sectors::of(bytes, size).ok_or(CfbError::Malformed("its allocation table cannot be read"))?;

    // The directory: entries of 128 bytes, in a chain of sectors.
    let directory = (|| {
      let mut directory = Vec::new();
      for number in sectors.chain(u32_at(bytes, 0x30)?)? {
        directory.extend_from_slice(sectors.sector(number)?);
      }
      Some(directory)
    })()
    .ok_or(CfbError::Malformed("its directory cannot be read"))?;
    let (mut entries, links): (Vec<Entry>, Vec<Links>) = directory
      .as_chunks::<128>()
      .0
      .iter()
      .map(|entry| {
        let named = usize::from(u16::from_le_bytes([entry[0x40], entry[0x41]])).min(64);
        let units: Vec<u16> = entry[..named]
          .as_chunks::<2>()
          .0
          .iter()
          .map(|pair| u16::from_le_bytes(*pair))
          .take_while(|unit| *unit != 0)
          .collect();
        let u32_in = |at: usize| u32_at(entry, at).unwrap_or(u32::MAX);
        let read = Entry {
          name: String::from_utf16_lossy(&units),
          kind: match entry[0x42] {
            1 => Kind::Storage,
            2 => Kind::Stream,
            5 => Kind::Root,
            _ => Kind::Unused,
          },
          children: Vec::new(),
          first: u32_in(0x74),
          // A file with sectors of 512 bytes keeps a stream's length in four bytes.
          length: usize::try_from(u32_in(0x78)).ok(),
        };
        let links = Links {
          left: u32_in(0x44),
          right: u32_in(0x48),
          child: u32_in(0x4C),
        };
        (read, links)
      })
      .unzip();
    let root = entries
      .iter()
      .position(|entry| entry.kind == Kind::Root)
      .ok_or(CfbError::Malformed("its directory has no root"))?;

    // What each storage holds: the entry it points at, and every entry reached from that one
    // to the left and to the right. An entry is taken the first time it is reached, so that
    // entries that point back at each other are walked once.
    let mut reached = vec![false; entries.len()];
    reached[root] = true;
    let mut open = vec![(root, links[root].child)];
    while let Some((storage, number)) = open.pop() {
      let Some(index) = usize::try_from(number)
        .ok()
        .filter(|index| *index < entries.len())
      else {
        continue;
      };
      if reached[index] {
        continue;
      }
      reached[index] = true;
      entries[storage].children.push(index);
      open.push((storage, links[index].left));
      open.push((storage, links[index].right));
      if entries[index].kind == Kind::Storage {
        open.push((index, links[index].child));
      }
    }

    // Streams under the cutoff are kept in the mini stream, which is the root entry's own
    // stream, in sectors of 64 bytes chained through a table of their own.
    let small = (|| {
      let mini_stream = match entries[root].length? {
        0 => Vec::new(),
        length => sectors.read(entries[root].first, length)?,
      };
      let mut mini_table = Vec::new();
      for number in sectors.chain(u32_at(bytes, 0x3C)?)? {
        let sector = sectors.sector(number)?;
        mini_table.extend((0..size / 4).filter_map(|slot| u32_at(sector, slot * 4)));
      }
      Some((mini_stream, mini_table))
    })();
    let (mini_stream, mini_table) =
      small.ok_or(CfbError::Malformed("its small streams cannot be read"))?;
    Ok(CompoundFile {
      sectors,
      cutoff,
      mini_stream,
      mini_table,
      entries,
      root,
    })
  }

  /// `length` bytes of the mini stream, from the chain of mini sectors that starts at `first`.
  fn mini(&self, first: u32, length: usize) -> Option<Vec<u8>> {
    let most = self.mini_stream.len() / MINI_SECTOR;
    let mut out = Vec::new();
    let mut next = first;
    let mut walked = 0;
    while next != END && out.len() < length {
      walked += 1;
      if walked > most {
        return None;
      }
      let at = usize::try_from(next).ok()?.checked_mul(MINI_SECTOR)?;
      out.extend_from_slice(self.mini_stream.get(at..at + MINI_SECTOR)?);
      next = *self.mini_table.get(usize::try_from(next).ok()?)?;
    }
    (out.len() >= length).then(|| {
      out.truncate(length);
      out
    })
  }

  /// A stream's bytes. None when its chain is broken, or it is said to be longer than its
  /// sectors.
  fn read(&self, entry: &Entry) -> Option<Vec<u8>> {
    match entry.length? {
      0 => Some(Vec::new()),
      length if length < self.cutoff => self.mini(entry.first, length),
      length => self.sectors.read(entry.first, length),
    }
  }

  /// The entry at `path`: names from the root down, with `/` between them. The empty path is
  /// the root.
  fn find(&self, path: &str) -> Option<usize> {
    path
      .split('/')
      .filter(|name| !name.is_empty())
      .try_fold(self.root, |storage, name| {
        self.entries[storage]
          .children
          .iter()
          .copied()
          .find(|child| same_name(&self.entries[*child].name, name))
      })
  }

  /// The bytes of the stream at `path`, which names the storages it lies in from the root
  /// down and then the stream, with `/` between them: `ObjectPool/_1234/Package`. A name
  /// matches whatever its case. None when there is no such stream, or it cannot be read.
  pub fn stream(&self, path: &str) -> Option<Vec<u8>> {
    let entry = &self.entries[self.find(path)?];
    match entry.kind {
      Kind::Stream => self.read(entry),
      Kind::Root | Kind::Storage | Kind::Unused => None,
    }
  }

  /// Whether a storage is at `path`. The empty path is the root, which is one.
  pub fn is_storage(&self, path: &str) -> bool {
    self
      .find(path)
      .is_some_and(|index| matches!(self.entries[index].kind, Kind::Root | Kind::Storage))
  }

  /// What the storage at `path` holds, in no stated order. Nothing when no storage is there.
  pub fn children(&self, path: &str) -> Vec<Child<'_>> {
    let Some(storage) = self.find(path) else {
      return Vec::new();
    };
    self.entries[storage]
      .children
      .iter()
      .map(|child| &self.entries[*child])
      .filter_map(|entry| {
        let is_storage = match entry.kind {
          Kind::Storage => true,
          Kind::Stream => false,
          Kind::Root | Kind::Unused => return None,
        };
        Some(Child {
          name: &entry.name,
          is_storage,
        })
      })
      .collect()
  }

  /// Every stream with its own name, whichever storage it lies in, in the order of the
  /// directory. A stream whose chain is broken, or that is said to be longer than its sectors,
  /// is left out and the rest are still given.
  pub fn streams(&self) -> Vec<Stream> {
    self
      .entries
      .iter()
      .filter(|entry| entry.kind == Kind::Stream)
      .filter_map(|entry| {
        Some(Stream {
          name: entry.name.clone(),
          bytes: self.read(entry)?,
        })
      })
      .collect()
  }

  /// The storage at `path` as a compound file of its own, holding the streams and the storages
  /// it holds here. This is what an embedded workbook or document is: a storage of the file it
  /// is embedded in. A stream that cannot be read is left out, and so is a storage that holds
  /// no stream. None when no storage is at `path`.
  pub fn storage(&self, path: &str) -> Option<Vec<u8>> {
    let top = self.find(path)?;
    if !matches!(self.entries[top].kind, Kind::Root | Kind::Storage) {
      return None;
    }
    let mut held: Vec<(String, Vec<u8>)> = Vec::new();
    let mut open = vec![(top, String::new())];
    while let Some((storage, prefix)) = open.pop() {
      for child in &self.entries[storage].children {
        let entry = &self.entries[*child];
        let path = format!("{prefix}{}", entry.name);
        match entry.kind {
          Kind::Stream => held.extend(self.read(entry).map(|bytes| (path, bytes))),
          Kind::Storage => open.push((*child, format!("{path}/"))),
          Kind::Root | Kind::Unused => {}
        }
      }
    }
    let listed: Vec<(&str, &[u8])> = held
      .iter()
      .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
      .collect();
    compound(&listed).ok()
  }
}

#[cfg(test)]
mod tests;
