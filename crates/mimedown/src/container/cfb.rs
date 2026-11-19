//! The compound file: a small file system in a file, which is how Word, Excel and PowerPoint
//! kept their documents before 2007. What it holds is told by the names directly under its
//! root.

use std::collections::HashSet;

use crate::source::{ByteRange, Missing, Source};

/// The root of a compound file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Root {
  /// The class of the application that wrote the file, when it recorded one.
  pub class_id: [u8; 16],
  /// The names of the streams and storages directly under the root.
  pub children: Vec<String>,
  /// A part of the directory that was not given, without which the names are not all here.
  pub needs: Option<ByteRange>,
  /// The directory was there to read and opens with a root storage, as every compound file's
  /// does. The eight bytes a file begins with do not say as much.
  pub rooted: bool,
}

const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The number a directory entry gives the root storage.
const ROOT_STORAGE: u8 = 5;

/// A sector number from here up is a mark and not a sector: the end of a chain, a free sector.
const FIRST_MARK: u32 = 0xFFFF_FFFA;

/// How many sectors of the allocation table the header lists itself. A file with more keeps
/// the rest of the list elsewhere, and a directory that far into the file is not followed.
const TABLE_SECTORS_IN_HEADER: u64 = 109;

/// No more sectors of directory than this are read, and no more names than this are listed.
const MOST_DIRECTORY_SECTORS: usize = 1024;
const MOST_CHILDREN: usize = 4096;

/// The file's root, when it is a compound file.
pub(crate) fn root(source: &Source<'_>) -> Result<Option<Root>, Missing> {
  if source.array::<8>(0)? != MAGIC {
    return Ok(None);
  }
  let (entries, needs) = directory(source);
  Ok(Some(Root {
    needs,
    ..root_of(&entries)
  }))
}

/// One entry of the directory: a stream or a storage, and where its neighbors are. The
/// entries under one storage form a tree: `left` and `right` are siblings, `child` is the
/// first entry of what this one holds.
struct Entry {
  name: String,
  /// What the entry is: 1 a storage, 2 a stream, 5 the root storage.
  kind: u8,
  left: u32,
  right: u32,
  child: u32,
  class_id: [u8; 16],
}

fn le32(bytes: [u8; 128], at: usize) -> u32 {
  u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn entry(bytes: [u8; 128]) -> Entry {
  // The name is UTF-16 in the first 64 bytes; its length in bytes, counting the zero that ends
  // it, follows.
  let length = usize::from(u16::from_le_bytes([bytes[64], bytes[65]])).min(64);
  let (units, _) = bytes[..length].as_chunks::<2>();
  let units = units
    .iter()
    .map(|unit| u16::from_le_bytes(*unit))
    .take_while(|unit| *unit != 0);
  let name = char::decode_utf16(units)
    .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
    .collect();
  let mut class_id = [0; 16];
  class_id.copy_from_slice(&bytes[80..96]);
  Entry {
    name,
    kind: bytes[66],
    left: le32(bytes, 68),
    right: le32(bytes, 72),
    child: le32(bytes, 76),
    class_id,
  }
}

/// The entries of the directory, for as far as it could be read, and the range that was not
/// given when that is what stopped it.
fn directory(source: &Source<'_>) -> (Vec<Entry>, Option<ByteRange>) {
  let mut entries = Vec::new();
  let needs = match read_directory(source, &mut entries) {
    Ok(()) | Err(Missing::PastEnd) => None,
    Err(Missing::NotGiven(range)) => Some(range),
  };
  (entries, needs)
}

fn read_directory(source: &Source<'_>, entries: &mut Vec<Entry>) -> Result<(), Missing> {
  let header = source.array::<512>(0)?;
  // Sectors are 512 bytes, or 4,096 in the later version of the format.
  let shift = u16::from_le_bytes([header[30], header[31]]);
  if shift != 9 && shift != 12 {
    return Ok(());
  }
  let sector_bytes = 1u64 << shift;
  // The header takes the place of a sector in front of sector 0.
  let offset_of = |sector: u32| (u64::from(sector) + 1) * sector_bytes;
  let slots_per_sector = sector_bytes / 4;

  let mut sector = u32::from_le_bytes([header[48], header[49], header[50], header[51]]);
  let mut read = HashSet::new();
  while sector < FIRST_MARK && read.len() < MOST_DIRECTORY_SECTORS && read.insert(sector) {
    let bytes = source.read(
      offset_of(sector),
      usize::try_from(sector_bytes).unwrap_or(0),
    )?;
    let (records, _) = bytes.as_chunks::<128>();
    entries.extend(records.iter().map(|record| entry(*record)));

    // The sector that follows this one is in the allocation table, at this sector's slot. The
    // table is itself kept in sectors, which the header lists.
    let table = u64::from(sector) / slots_per_sector;
    if table >= TABLE_SECTORS_IN_HEADER {
      break;
    }
    let listed = 76 + 4 * usize::try_from(table).unwrap_or(0);
    let Some(table_sector) = header
      .get(listed..listed + 4)
      .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
      .map(u32::from_le_bytes)
    else {
      break;
    };
    if table_sector >= FIRST_MARK {
      break;
    }
    let slot = offset_of(table_sector) + (u64::from(sector) % slots_per_sector) * 4;
    sector = u32::from_le_bytes(source.array(slot)?);
  }
  Ok(())
}

/// The root's class and the names directly under it. The root is the first entry.
fn root_of(entries: &[Entry]) -> Root {
  let Some(root) = entries.first() else {
    return Root::default();
  };
  let mut children = Vec::new();
  let mut seen = HashSet::new();
  let mut pending = vec![root.child];
  while let Some(id) = pending.pop() {
    let found = usize::try_from(id).ok().and_then(|at| entries.get(at));
    let Some(entry) = found else {
      continue;
    };
    if children.len() >= MOST_CHILDREN || !seen.insert(id) {
      continue;
    }
    children.push(entry.name.clone());
    pending.extend([entry.left, entry.right]);
  }
  Root {
    class_id: root.class_id,
    children,
    needs: None,
    rooted: root.kind == ROOT_STORAGE,
  }
}

#[cfg(test)]
mod tests;
