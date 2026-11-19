//! Zip: the names of an archive's entries, which is what tells a docx from an EPUB from a
//! Wallet pass. Names are stored as they are, so listing them inflates nothing. One small
//! entry can be read out by name, for the type that only its content settles.

use std::io::Read;

use crate::source::{ByteRange, Missing, Source};

/// What a zip lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Listing {
  /// The names of the entries, in the order they were found.
  pub names: Vec<String>,
  /// What a first entry called `mimetype` holds, when it is stored as it is: how an
  /// OpenDocument file and an EPUB name themselves.
  pub declared: Option<String>,
  /// Whether every name is here, or only those the bytes given led to.
  pub complete: bool,
  /// The range that holds the rest of the names.
  pub needs: Option<ByteRange>,
}

/// What opens an entry, an entry's record in the directory, and the record that closes the
/// directory.
const ENTRY: [u8; 4] = *b"PK\x03\x04";
const DIRECTORY_ENTRY: [u8; 4] = *b"PK\x01\x02";
const CLOSING: [u8; 4] = *b"PK\x05\x06";

/// The closing record is 22 bytes and may carry a comment of up to 65,535 after it, so it
/// starts no further from the end of the file than this.
const CLOSING_REACH: u64 = 22 + 0xFFFF;

/// No more names than this are listed, and no more of a directory than this is read.
const MOST_NAMES: usize = 10_000;
const MOST_DIRECTORY_BYTES: u32 = 8 << 20;

/// The longest content a `mimetype` entry is read for.
const LONGEST_DECLARATION: u32 = 255;

/// The file's entries, when it is a zip.
pub(crate) fn listing(source: &Source<'_>) -> Result<Option<Listing>, Missing> {
  let signature = source.array::<4>(0)?;
  if signature != ENTRY && signature != CLOSING {
    return Ok(None);
  }
  let declared = declared(source);
  Ok(Some(match directory(source) {
    Ok((names, complete)) => Listing {
      names,
      declared,
      complete,
      needs: None,
    },
    // Without the directory, each entry's own header leads to the next, for as far as the
    // bytes go.
    Err(why) => {
      let (walked, complete) = walk(source);
      let names = walked.into_iter().map(|entry| entry.name).collect();
      let needs = match why {
        NoDirectory::Ask(range) if !complete => Some(range),
        _ => None,
      };
      Listing {
        names,
        declared,
        complete,
        needs,
      }
    }
  }))
}

/// The entries of a zip by the directory at its end alone, whatever the file opens with: how a
/// zip that something else stands in front of is read. Nothing when no directory is there, or
/// when it lists nothing, since then nothing shows that it is one.
pub(crate) fn listing_by_directory(source: &Source<'_>) -> Result<Option<Listing>, Missing> {
  match directory(source) {
    Ok((names, complete)) if !names.is_empty() => Ok(Some(Listing {
      names,
      declared: declared(source),
      complete,
      needs: None,
    })),
    Ok(_) | Err(NoDirectory::Unreadable) => Ok(None),
    Err(NoDirectory::Ask(range)) => Err(Missing::NotGiven(range)),
  }
}

/// Why the directory at the end of the file was not read.
enum NoDirectory {
  /// It was not among the bytes given. This range holds it, or leads to it.
  Ask(ByteRange),
  /// It is not there, or it is written in a way that is not read here.
  Unreadable,
}

impl From<Missing> for NoDirectory {
  fn from(missing: Missing) -> Self {
    match missing {
      Missing::NotGiven(range) => NoDirectory::Ask(range),
      Missing::PastEnd => NoDirectory::Unreadable,
    }
  }
}

fn le16(low: u8, high: u8) -> usize {
  usize::from(u16::from_le_bytes([low, high]))
}

/// An entry as the directory records it: its name, how many bytes it keeps, and where its
/// own header starts.
struct Record {
  name: String,
  packed: u32,
  at: u32,
}

/// The names in the directory, and whether they are all of them.
fn directory(source: &Source<'_>) -> Result<(Vec<String>, bool), NoDirectory> {
  let (records, complete) = records(source)?;
  let names = records.into_iter().map(|record| record.name).collect();
  Ok((names, complete))
}

/// The directory's records, and whether they are all of them.
fn records(source: &Source<'_>) -> Result<(Vec<Record>, bool), NoDirectory> {
  let Some(size) = source.size() else {
    return Err(NoDirectory::Ask(ByteRange::last(CLOSING_REACH)));
  };
  let closing = closing_record(source, size)?;
  let entries = le16(closing[10], closing[11]);
  let length = u32::from_le_bytes([closing[12], closing[13], closing[14], closing[15]]);
  let offset = u32::from_le_bytes([closing[16], closing[17], closing[18], closing[19]]);
  // All ones in any of the three says the real figure is in a longer record, for an archive
  // too large for these. That record is not read.
  if entries == 0xFFFF || length == u32::MAX || offset == u32::MAX {
    return Err(NoDirectory::Unreadable);
  }
  let wanted = usize::try_from(length.min(MOST_DIRECTORY_BYTES)).unwrap_or(0);
  let mut records = source.read(u64::from(offset), wanted)?;

  let mut names = Vec::new();
  while names.len() < entries.min(MOST_NAMES) {
    let Some((record, rest)) = records.split_first_chunk::<46>() else {
      break;
    };
    if record[..4] != DIRECTORY_ENTRY {
      break;
    }
    let name = le16(record[28], record[29]);
    let after_name = le16(record[30], record[31]) + le16(record[32], record[33]);
    let Some(bytes) = rest.get(..name) else {
      break;
    };
    names.push(Record {
      name: String::from_utf8_lossy(bytes).into_owned(),
      packed: u32::from_le_bytes([record[20], record[21], record[22], record[23]]),
      at: u32::from_le_bytes([record[42], record[43], record[44], record[45]]),
    });
    let Some(next) = rest.get(name + after_name..) else {
      break;
    };
    records = next;
  }
  let complete = names.len() == entries;
  Ok((names, complete))
}

/// The most of an entry that is read out, as it is kept and as it comes out.
const MOST_PART_BYTES: usize = 1 << 20;

/// What the entry called `name` holds. Where it is and how long comes from the directory, or
/// from the entries' own headers when the directory was not given. None when the zip has no
/// such entry, or keeps it in a way or at a size that is not read here.
pub(crate) fn part(source: &Source<'_>, name: &str) -> Result<Option<Vec<u8>>, Missing> {
  let (at, packed) = match records(source) {
    Ok((records, _)) => match records.into_iter().find(|record| record.name == name) {
      Some(record) => (u64::from(record.at), Some(record.packed)),
      None => return Ok(None),
    },
    Err(why) => {
      let ask = || match &why {
        NoDirectory::Ask(range) => Err(Missing::NotGiven(*range)),
        NoDirectory::Unreadable => Ok(None),
      };
      match walk(source).0.into_iter().find(|entry| entry.name == name) {
        // Sizes written after the data are in the directory and nowhere in front of it.
        Some(entry) if entry.packed.is_none() => return ask(),
        Some(entry) => (entry.at, entry.packed),
        None => return ask(),
      }
    }
  };
  let header = source.array::<30>(at)?;
  let Some(header) = entry_header_of(header) else {
    return Ok(None);
  };
  let length = packed
    .and_then(|packed| usize::try_from(packed).ok())
    .filter(|length| *length <= MOST_PART_BYTES);
  let (Some(data_at), Some(length)) = (offset_after(at, &[30, header.name, header.extra]), length)
  else {
    return Ok(None);
  };
  let kept = source.read(data_at, length)?;
  if header.stored {
    return Ok(Some(kept.to_vec()));
  }
  if !header.deflated {
    return Ok(None);
  }
  let most = u64::try_from(MOST_PART_BYTES).unwrap_or(u64::MAX);
  let mut out = Vec::new();
  let read = flate2::read::DeflateDecoder::new(kept)
    .take(most.saturating_add(1))
    .read_to_end(&mut out);
  Ok(
    read
      .ok()
      .filter(|read| *read <= MOST_PART_BYTES)
      .map(|_| out),
  )
}

/// The record that closes the directory, and where it stands. It is the last 22 bytes of
/// nearly every zip; when a comment follows it, it is looked for from the end backward. When
/// the end of the file was not given, all of it that could hold the record is asked for.
fn closing_at(source: &Source<'_>, size: u64) -> Result<Option<(u64, [u8; 22])>, Missing> {
  if let Some(at) = size.checked_sub(22)
    && let Ok(record) = source.array::<22>(at)
    && record[..4] == CLOSING
  {
    return Ok(Some((at, record)));
  }
  let reach = size.min(CLOSING_REACH);
  let start = size - reach;
  let end = match source.read(start, usize::try_from(reach).unwrap_or(0)) {
    Ok(end) => end,
    Err(Missing::NotGiven(_)) => return Err(Missing::NotGiven(ByteRange::forward(start, reach))),
    Err(Missing::PastEnd) => return Ok(None),
  };
  let found = end.windows(22).rposition(|window| window[..4] == CLOSING);
  Ok(found.and_then(|within| {
    let record = <[u8; 22]>::try_from(end.get(within..within + 22)?).ok()?;
    Some((start + u64::try_from(within).ok()?, record))
  }))
}

fn closing_record(source: &Source<'_>, size: u64) -> Result<[u8; 22], NoDirectory> {
  match closing_at(source, size) {
    Ok(Some((_, record))) => Ok(record),
    Ok(None) => Err(NoDirectory::Unreadable),
    Err(missing) => Err(missing.into()),
  }
}

/// Where a zip that ends this file begins, by its own account: the closing record says how
/// long the directory is and how far into the zip it starts, and the directory ends where the
/// record begins. Zero for a zip that is the whole file.
pub(crate) fn start_from_end(source: &Source<'_>) -> Result<Option<u64>, Missing> {
  let Some(size) = source.size() else {
    return Err(Missing::NotGiven(ByteRange::last(CLOSING_REACH)));
  };
  let Some((at, record)) = closing_at(source, size)? else {
    return Ok(None);
  };
  let length = u64::from(u32::from_le_bytes([
    record[12], record[13], record[14], record[15],
  ]));
  let offset = u64::from(u32::from_le_bytes([
    record[16], record[17], record[18], record[19],
  ]));
  Ok(
    at.checked_sub(length)
      .and_then(|directory| directory.checked_sub(offset)),
  )
}

/// One entry's own header, in front of its data.
struct EntryHeader {
  /// The sizes are written after the data, so the header does not say where the data ends.
  sizes_follow: bool,
  stored: bool,
  deflated: bool,
  packed: u32,
  name: usize,
  extra: usize,
}

fn entry_header(source: &Source<'_>, at: u64) -> Option<EntryHeader> {
  entry_header_of(source.array::<30>(at).ok()?)
}

fn entry_header_of(header: [u8; 30]) -> Option<EntryHeader> {
  (header[..4] == ENTRY).then(|| EntryHeader {
    sizes_follow: header[6] & 0x08 != 0,
    stored: header[8] == 0 && header[9] == 0,
    deflated: header[8] == 8 && header[9] == 0,
    packed: u32::from_le_bytes([header[18], header[19], header[20], header[21]]),
    name: le16(header[26], header[27]),
    extra: le16(header[28], header[29]),
  })
}

fn offset_after(at: u64, lengths: &[usize]) -> Option<u64> {
  lengths.iter().try_fold(at, |at, length| {
    at.checked_add(u64::try_from(*length).ok()?)
  })
}

/// An entry as its own header gives it: its name, where the header starts, and how many bytes
/// it keeps, when the header says.
struct Walked {
  name: String,
  at: u64,
  packed: Option<u32>,
}

/// The entries their own headers give, read from the start of the file for as far as each
/// leads to the next; and whether that reached the directory, which is to say every entry.
fn walk(source: &Source<'_>) -> (Vec<Walked>, bool) {
  let mut names = Vec::new();
  let mut at = 0u64;
  while names.len() < MOST_NAMES {
    if let Ok(signature) = source.array::<4>(at)
      && (signature == DIRECTORY_ENTRY || signature == CLOSING)
    {
      return (names, true);
    }
    let Some(header) = entry_header(source, at) else {
      break;
    };
    let Some(name) =
      offset_after(at, &[30]).and_then(|name_at| source.read(name_at, header.name).ok())
    else {
      break;
    };
    names.push(Walked {
      name: String::from_utf8_lossy(name).into_owned(),
      at,
      packed: (!header.sizes_follow).then_some(header.packed),
    });
    if header.sizes_follow {
      break;
    }
    let data = usize::try_from(header.packed).unwrap_or(usize::MAX);
    let Some(next) = offset_after(at, &[30, header.name, header.extra, data]) else {
      break;
    };
    at = next;
  }
  (names, false)
}

/// What a first entry called `mimetype` holds, when it is stored and short.
fn declared(source: &Source<'_>) -> Option<String> {
  let header = entry_header(source, 0)?;
  let readable = header.stored && !header.sizes_follow && header.packed <= LONGEST_DECLARATION;
  if !readable || source.read(30, header.name).ok()? != b"mimetype" {
    return None;
  }
  let content_at = offset_after(0, &[30, header.name, header.extra])?;
  let content = source
    .read(content_at, usize::try_from(header.packed).ok()?)
    .ok()?;
  Some(String::from_utf8_lossy(content).trim().to_string())
}

#[cfg(test)]
mod tests;
