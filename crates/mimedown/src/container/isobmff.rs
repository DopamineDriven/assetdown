//! ISO base media: a file made of boxes, each a length and a type of four letters. MP4, MOV,
//! HEIC and AVIF are all this, and differ in the brands their first box declares.

use crate::source::{Missing, Source};

/// The brands a file declares for itself in its `ftyp` box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Brands {
  pub major: [u8; 4],
  pub compatible: Vec<[u8; 4]>,
}

/// How a file of boxes opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Opening {
  /// The type of the first box.
  pub first: [u8; 4],
  /// What the first box declares, when it is an `ftyp`.
  pub brands: Option<Brands>,
}

/// The types a file may open with when it has no `ftyp`: a QuickTime movie from before there
/// were brands, or a piece of a fragmented file. These are the ones `videodown` reads.
const OPENERS: [&[u8; 4]; 12] = [
  b"moov", b"mdat", b"free", b"skip", b"wide", b"pnot", b"uuid", b"styp", b"sidx", b"moof",
  b"meta", b"pdin",
];

/// The types that can follow a first box at the top of a file, beyond the openers.
const FOLLOWERS: [&[u8; 4]; 5] = [b"ftyp", b"mfra", b"prft", b"emsg", b"ssix"];

/// No `ftyp` is longer than this: it is a few brands of four bytes each.
const LONGEST_FILE_TYPE: u32 = 4096;

/// No box but the media itself is this long. Four characters of text read as a length come to
/// at least this much, so it is also what keeps `    free(pointer);` from being a box.
const LONGEST_BOX: u64 = 0x2000_0000;

/// How the file opens, when it opens as ISO base media does.
pub(crate) fn opening(source: &Source<'_>) -> Result<Option<Opening>, Missing> {
  let [s0, s1, s2, s3, first @ ..] = source.array::<8>(0)?;
  let stated = u32::from_be_bytes([s0, s1, s2, s3]);
  if first == *b"ftyp" {
    let brands = file_type(source, 0, stated)?;
    return Ok(brands.map(|brands| Opening {
      first,
      brands: Some(brands),
    }));
  }
  if !OPENERS.contains(&&first) {
    return Ok(None);
  }
  let opening = Opening {
    first,
    brands: None,
  };
  let is_media = first == *b"mdat";
  let length = match stated {
    // The box is the rest of the file, which only the media may be.
    0 => return Ok(is_media.then_some(opening)),
    // The length is too long for four bytes and stands in the eight after the type.
    1 => u64::from_be_bytes(source.array(8)?),
    stated => u64::from(stated),
  };
  let header = if stated == 1 { 16 } else { 8 };
  if length < header || (!is_media && length >= LONGEST_BOX) {
    return Ok(None);
  }
  // A type of four letters is not much to go on, so the box has to end where another begins,
  // or where the file does.
  match source.array::<8>(length) {
    Ok([_, _, _, _, next @ ..]) => {
      let follows = OPENERS.contains(&&next) || FOLLOWERS.contains(&&next);
      Ok(follows.then_some(opening))
    }
    Err(Missing::PastEnd) => Ok((source.size() == Some(length)).then_some(opening)),
    Err(missing) => Err(missing),
  }
}

/// The boxes in `bytes`, one after another: each its type and what it holds.
pub(crate) fn boxes(mut bytes: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
  std::iter::from_fn(move || {
    let (head, rest) = bytes.split_first_chunk::<8>()?;
    let stated = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
    let kind = [head[4], head[5], head[6], head[7]];
    let (payload, after) = match stated {
      // The box is the rest of what holds it.
      0 => (rest, &rest[rest.len()..]),
      // The length is in the eight bytes after the type.
      1 => {
        let (long, rest) = rest.split_first_chunk::<8>()?;
        let length = usize::try_from(u64::from_be_bytes(*long))
          .ok()?
          .checked_sub(16)?;
        (rest.get(..length)?, rest.get(length..)?)
      }
      stated => {
        let length = usize::try_from(stated).ok()?.checked_sub(8)?;
        (rest.get(..length)?, rest.get(length..)?)
      }
    };
    bytes = after;
    Some((kind, payload))
  })
}

/// One box of a file: its type, where what it holds begins, and how long that is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoxAt {
  pub kind: [u8; 4],
  pub payload_at: u64,
  /// How long what it holds is. A box that runs to the end of a file whose size is not known
  /// is as long as anything can be.
  pub payload: u64,
  /// Where the box after it begins.
  pub next: u64,
}

/// The box that begins at `at`. None for one that states a length shorter than its own
/// header.
pub(crate) fn box_at(source: &Source<'_>, at: u64) -> Result<Option<BoxAt>, Missing> {
  let [s0, s1, s2, s3, kind @ ..] = source.array::<8>(at)?;
  let (header, length) = match u32::from_be_bytes([s0, s1, s2, s3]) {
    // The box is the rest of the file.
    0 => (
      8,
      source
        .size()
        .map_or(u64::MAX, |size| size.saturating_sub(at)),
    ),
    // The length is in the eight bytes after the type.
    1 => (16, u64::from_be_bytes(source.array(at + 8)?)),
    stated => (8, u64::from(stated)),
  };
  if length < header {
    return Ok(None);
  }
  Ok(Some(BoxAt {
    kind,
    payload_at: at + header,
    payload: length - header,
    next: at.saturating_add(length),
  }))
}

/// The brands declared by an `ftyp` box that stands at `at`, when one stands there. In the
/// JPEG 2000 family it follows a signature box.
pub(crate) fn brands_at(source: &Source<'_>, at: u64) -> Result<Option<Brands>, Missing> {
  let [s0, s1, s2, s3, kind @ ..] = source.array::<8>(at)?;
  if kind != *b"ftyp" {
    return Ok(None);
  }
  file_type(source, at, u32::from_be_bytes([s0, s1, s2, s3]))
}

/// The brands of an `ftyp` box at `at` that says it is `stated` bytes long: a major brand, a
/// version, and then as many compatible brands as fit.
fn file_type(source: &Source<'_>, at: u64, stated: u32) -> Result<Option<Brands>, Missing> {
  if !(16..=LONGEST_FILE_TYPE).contains(&stated) || !stated.is_multiple_of(4) {
    return Ok(None);
  }
  let Ok(length) = usize::try_from(stated - 8) else {
    return Ok(None);
  };
  let body = source.read(at + 8, length)?;
  let (brands, _) = body.as_chunks::<4>();
  let [major, _version, compatible @ ..] = brands else {
    return Ok(None);
  };
  let printable = major.iter().all(|byte| (0x20..=0x7E).contains(byte));
  Ok(printable.then(|| Brands {
    major: *major,
    compatible: compatible.to_vec(),
  }))
}

#[cfg(test)]
mod tests;
