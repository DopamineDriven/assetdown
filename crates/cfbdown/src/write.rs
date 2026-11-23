//! Writing a compound file: sectors of 512 bytes, with the streams under 4,096 bytes kept in
//! the mini stream, as the format has it.

use std::collections::HashMap;

use crate::error::CfbError;

const SECTOR: usize = 512;
const MINI_SECTOR: usize = 64;
/// A stream shorter than this is kept in the mini stream.
const CUTOFF: usize = 4096;
/// The last sector of a chain points here.
const END: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;
/// How the table marks a sector of its own.
const OF_TABLE: u32 = 0xFFFF_FFFD;
/// How the table marks a sector that lists sectors of the table.
const OF_LISTING: u32 = 0xFFFF_FFFC;
/// Sector numbers to one sector of the table.
const PER_SECTOR: usize = SECTOR / 4;
/// Sectors of the table that the header itself lists.
const IN_HEADER: usize = 109;
const ENTRY: usize = 128;
/// A name's letters; the entry keeps one unit more, to end it.
const LONGEST_NAME: usize = 31;

/// What an entry holds.
enum Held<'a> {
  Storage(Vec<usize>),
  Stream(&'a [u8]),
}

struct Node<'a> {
  name: Vec<u16>,
  /// What the format orders the entries of a storage by: a shorter name first, and names of
  /// one length by their letters in upper case.
  key: (usize, Vec<u16>),
  held: Held<'a>,
  left: u32,
  right: u32,
  child: u32,
  start: u32,
}

fn sectors_for(length: usize, size: usize) -> usize {
  length.div_ceil(size)
}

fn number(value: usize) -> Result<u32, CfbError> {
  u32::try_from(value).map_err(|_| CfbError::Unwritable("it is larger than the format holds"))
}

/// A name in the units the format keeps it in, and in upper case, a letter at a time.
fn units_of(name: &str) -> (Vec<u16>, Vec<u16>) {
  let upper: String = name
    .chars()
    .map(|letter| {
      let mut cased = letter.to_uppercase();
      match (cased.next(), cased.next()) {
        (Some(one), None) => one,
        _ => letter,
      }
    })
    .collect();
  (
    name.encode_utf16().collect(),
    upper.encode_utf16().collect(),
  )
}

/// The numbers `sorted` holds, made a tree with as many to the left of each as to its right.
/// Gives the number at the top.
fn balanced(sorted: &[usize], links: &mut [(u32, u32)]) -> Result<u32, CfbError> {
  if sorted.is_empty() {
    return Ok(FREE);
  }
  let middle = sorted.len() / 2;
  let left = balanced(&sorted[..middle], links)?;
  let right = balanced(&sorted[middle + 1..], links)?;
  links[sorted[middle]] = (left, right);
  number(sorted[middle])
}

/// Sectors numbered from the length of `table` on, chained one to the next. Gives the first,
/// or the end mark when there are none.
fn chain(count: usize, table: &mut Vec<u32>) -> Result<u32, CfbError> {
  if count == 0 {
    return Ok(END);
  }
  let first = table.len();
  for sector in 1..count {
    table.push(number(first + sector)?);
  }
  table.push(END);
  number(first)
}

/// An entry of this name, not yet placed among the others.
fn node<'a>(name: &str, held: Held<'a>) -> Result<Node<'a>, CfbError> {
  let (name, upper) = units_of(name);
  if name.is_empty() {
    return Err(CfbError::Unwritable("a name is empty"));
  }
  if name.len() > LONGEST_NAME {
    return Err(CfbError::Unwritable("a name is longer than 31 characters"));
  }
  Ok(Node {
    key: (name.len(), upper),
    name,
    held,
    left: FREE,
    right: FREE,
    child: FREE,
    start: 0,
  })
}

/// A compound file of these streams. Each is named by its path: the storages it lies in from
/// the root down and then its own name, with `/` between them, and a storage is made for
/// every one named. A name is 31 characters at the most, and a storage holds one entry of a
/// name, whatever its case.
pub fn compound(streams: &[(&str, &[u8])]) -> Result<Vec<u8>, CfbError> {
  let mut nodes = vec![node("Root Entry", Held::Storage(Vec::new()))?];
  let mut by_name: HashMap<(usize, Vec<u16>), usize> = HashMap::new();
  for (path, bytes) in streams {
    let mut storage = 0;
    let mut names = path.split('/').peekable();
    while let Some(name) = names.next() {
      let last = names.peek().is_none();
      let held = if last {
        Held::Stream(bytes)
      } else {
        Held::Storage(Vec::new())
      };
      let named = node(name, held)?;
      let known = by_name.get(&(storage, named.key.1.clone())).copied();
      storage = match known {
        Some(index) if !last && matches!(nodes[index].held, Held::Storage(_)) => index,
        Some(_) => {
          return Err(CfbError::Unwritable(
            "two entries of one storage have the same name",
          ));
        }
        None => {
          let index = nodes.len();
          by_name.insert((storage, named.key.1.clone()), index);
          if let Held::Storage(children) = &mut nodes[storage].held {
            children.push(index);
          }
          nodes.push(named);
          index
        }
      };
    }
  }

  // The entries of each storage, in the tree the format looks a name up in.
  let mut links = vec![(FREE, FREE); nodes.len()];
  for index in 0..nodes.len() {
    let Held::Storage(children) = &nodes[index].held else {
      continue;
    };
    let mut sorted = children.clone();
    sorted.sort_by(|a, b| nodes[*a].key.cmp(&nodes[*b].key));
    nodes[index].child = balanced(&sorted, &mut links)?;
  }
  for (node, (left, right)) in nodes.iter_mut().zip(links) {
    node.left = left;
    node.right = right;
  }

  // The mini stream: every small stream, each starting on a mini sector.
  let mut mini_stream = Vec::new();
  let mut mini_table: Vec<u32> = Vec::new();
  for node in &mut nodes {
    let Held::Stream(bytes) = node.held else {
      continue;
    };
    if bytes.is_empty() {
      node.start = END;
    } else if bytes.len() < CUTOFF {
      node.start = chain(sectors_for(bytes.len(), MINI_SECTOR), &mut mini_table)?;
      mini_stream.extend_from_slice(bytes);
      mini_stream.resize(mini_table.len() * MINI_SECTOR, 0);
    }
  }

  // Sectors, in order: those that list the table's sectors past what the header lists, the
  // table, the directory, the table of the mini stream, the mini stream, and the large streams.
  let directory_sectors = sectors_for(nodes.len() * ENTRY, SECTOR);
  let mini_table_sectors = sectors_for(mini_table.len() * 4, SECTOR);
  let mini_stream_sectors = sectors_for(mini_stream.len(), SECTOR);
  let large_sectors: usize = nodes
    .iter()
    .filter_map(|node| match node.held {
      Held::Stream(bytes) if bytes.len() >= CUTOFF => Some(sectors_for(bytes.len(), SECTOR)),
      _ => None,
    })
    .sum();
  let held_sectors = directory_sectors + mini_table_sectors + mini_stream_sectors + large_sectors;
  // The table chains its own sectors and the listing's too, so each is counted until neither
  // number moves.
  let (mut table_sectors, mut listing_sectors) = (0, 0);
  loop {
    let table = (held_sectors + table_sectors + listing_sectors).div_ceil(PER_SECTOR);
    let listing = table.saturating_sub(IN_HEADER).div_ceil(PER_SECTOR - 1);
    if (table, listing) == (table_sectors, listing_sectors) {
      break;
    }
    (table_sectors, listing_sectors) = (table, listing);
  }

  let mut table: Vec<u32> = vec![OF_LISTING; listing_sectors];
  table.resize(listing_sectors + table_sectors, OF_TABLE);
  let directory_at = chain(directory_sectors, &mut table)?;
  let mini_table_at = chain(mini_table_sectors, &mut table)?;
  let mini_stream_at = chain(mini_stream_sectors, &mut table)?;
  for node in &mut nodes {
    if let Held::Stream(bytes) = node.held
      && bytes.len() >= CUTOFF
    {
      node.start = chain(sectors_for(bytes.len(), SECTOR), &mut table)?;
    }
  }
  table.resize(table_sectors * PER_SECTOR, FREE);
  // The root entry's own stream is the mini stream.
  nodes[0].start = mini_stream_at;

  let table_sector = |nth: usize| number(listing_sectors + nth);
  let mut header = vec![0u8; SECTOR];
  header[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
  header[0x18..0x1A].copy_from_slice(&0x3Eu16.to_le_bytes());
  header[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes());
  header[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
  header[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes());
  header[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
  header[0x2C..0x30].copy_from_slice(&number(table_sectors)?.to_le_bytes());
  header[0x30..0x34].copy_from_slice(&directory_at.to_le_bytes());
  header[0x38..0x3C].copy_from_slice(&number(CUTOFF)?.to_le_bytes());
  header[0x3C..0x40].copy_from_slice(&mini_table_at.to_le_bytes());
  header[0x40..0x44].copy_from_slice(&number(mini_table_sectors)?.to_le_bytes());
  let listing_at = if listing_sectors == 0 { END } else { 0 };
  header[0x44..0x48].copy_from_slice(&listing_at.to_le_bytes());
  header[0x48..0x4C].copy_from_slice(&number(listing_sectors)?.to_le_bytes());
  for slot in 0..IN_HEADER {
    let listed = if slot < table_sectors {
      table_sector(slot)?
    } else {
      FREE
    };
    header[0x4C + slot * 4..0x50 + slot * 4].copy_from_slice(&listed.to_le_bytes());
  }

  let mut file = header;
  // Each sector of the listing: the table's sectors that follow those already listed, and
  // then the number of the next sector of the listing.
  for sector in 0..listing_sectors {
    let from = IN_HEADER + sector * (PER_SECTOR - 1);
    for slot in 0..PER_SECTOR - 1 {
      let listed = if from + slot < table_sectors {
        table_sector(from + slot)?
      } else {
        FREE
      };
      file.extend_from_slice(&listed.to_le_bytes());
    }
    let next = if sector + 1 == listing_sectors {
      END
    } else {
      number(sector + 1)?
    };
    file.extend_from_slice(&next.to_le_bytes());
  }
  file.extend(table.iter().flat_map(|next| next.to_le_bytes()));

  for (index, node) in nodes.iter().enumerate() {
    let mut entry = [0u8; ENTRY];
    for (at, unit) in node.name.iter().enumerate() {
      entry[at * 2..at * 2 + 2].copy_from_slice(&unit.to_le_bytes());
    }
    let named = u16::try_from((node.name.len() + 1) * 2).unwrap_or(u16::MAX);
    entry[0x40..0x42].copy_from_slice(&named.to_le_bytes());
    let (kind, size) = match node.held {
      // The root, whose own stream is the mini stream.
      Held::Storage(_) if index == 0 => (5, mini_stream.len()),
      Held::Storage(_) => (1, 0),
      Held::Stream(bytes) => (2, bytes.len()),
    };
    entry[0x42] = kind;
    // Every entry is black: a tree as even as this one is in order without a red entry.
    entry[0x43] = 1;
    entry[0x44..0x48].copy_from_slice(&node.left.to_le_bytes());
    entry[0x48..0x4C].copy_from_slice(&node.right.to_le_bytes());
    entry[0x4C..0x50].copy_from_slice(&node.child.to_le_bytes());
    entry[0x74..0x78].copy_from_slice(&node.start.to_le_bytes());
    entry[0x78..0x7C].copy_from_slice(&number(size)?.to_le_bytes());
    file.extend_from_slice(&entry);
  }
  // An entry of the directory that is not in use leads nowhere.
  let mut unused = [0u8; ENTRY];
  unused[0x44..0x50].fill(0xFF);
  for _ in nodes.len()..directory_sectors * SECTOR / ENTRY {
    file.extend_from_slice(&unused);
  }

  file.extend(mini_table.iter().flat_map(|next| next.to_le_bytes()));
  file.resize(
    (1 + listing_sectors + table_sectors + directory_sectors + mini_table_sectors) * SECTOR,
    0xFF,
  );
  file.extend_from_slice(&mini_stream);
  file.resize(file.len().next_multiple_of(SECTOR), 0);
  for node in &nodes {
    if let Held::Stream(bytes) = node.held
      && bytes.len() >= CUTOFF
    {
      file.extend_from_slice(bytes);
      file.resize(file.len().next_multiple_of(SECTOR), 0);
    }
  }
  Ok(file)
}

#[cfg(test)]
mod tests;
