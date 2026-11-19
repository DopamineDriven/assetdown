//! The bytes of a file as they were handed over: all of it, or pieces of it with their offsets.
//!
//! Every reader asks here for bytes at an offset, and is told one of three things: here they
//! are, the file ends before them, or they were not among the pieces given. The last is what an
//! answer's `needs` is made from.

use std::borrow::Cow;

use crate::error::MimeError;

/// A stretch of a file, by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
  pub offset: u64,
  pub length: u64,
  /// `offset` counts back from the end of the file: where a zip keeps its directory.
  pub from_end: bool,
}

impl ByteRange {
  /// `length` bytes starting `offset` bytes into the file.
  pub(crate) fn forward(offset: u64, length: u64) -> Self {
    ByteRange {
      offset,
      length,
      from_end: false,
    }
  }

  /// One range that takes in both, so that a single fetch serves two readers. A range counted
  /// from the end of the file cannot be joined to one counted from its start, and is kept.
  pub(crate) fn spanning(self, other: ByteRange) -> ByteRange {
    if self.from_end || other.from_end {
      return self;
    }
    let start = self.offset.min(other.offset);
    let end =
      (self.offset.saturating_add(self.length)).max(other.offset.saturating_add(other.length));
    ByteRange::forward(start, end - start)
  }

  /// The last `length` bytes of a file whose size is not known.
  pub(crate) fn last(length: u64) -> Self {
    ByteRange {
      offset: length,
      length,
      from_end: true,
    }
  }
}

/// One piece of a file: where it starts, and the bytes that stand there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece<'a> {
  pub offset: u64,
  pub bytes: &'a [u8],
}

/// Why a read came back without bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Missing {
  /// The file ends before the read does.
  PastEnd,
  /// The file may hold the bytes, and they were not given. The range starts at the first byte
  /// that is missing and ends where the read did.
  NotGiven(ByteRange),
}

/// A run of bytes with no gap in it. Borrowed as given, or put together from pieces that met.
struct Segment<'a> {
  offset: u64,
  bytes: Cow<'a, [u8]>,
}

impl Segment<'_> {
  /// The offset of the first byte after the run.
  fn end(&self) -> u64 {
    self.offset.saturating_add(len64(&self.bytes))
  }

  /// Take in a piece that starts inside this run or at its end. Where the two overlap they have
  /// to agree; what the piece holds past the run's end is added to it.
  fn absorb(&mut self, piece: &Piece<'_>) -> Result<(), MimeError> {
    let disagree = |at: u64| {
      MimeError::InvalidRanges(format!(
        "the pieces at offsets {} and {} overlap and differ at offset {at}",
        self.offset, piece.offset
      ))
    };
    // Both lie within the run, whose bytes are in memory, so they fit a `usize`.
    let start = usize::try_from(piece.offset - self.offset).map_err(|_| disagree(piece.offset))?;
    let held = &self.bytes[start..];
    let shared = held.len().min(piece.bytes.len());
    if let Some(at) = (0..shared).find(|&at| held[at] != piece.bytes[at]) {
      return Err(disagree(
        piece.offset.saturating_add(len64(&piece.bytes[..at])),
      ));
    }
    if piece.bytes.len() > shared {
      self
        .bytes
        .to_mut()
        .extend_from_slice(&piece.bytes[shared..]);
    }
    Ok(())
  }
}

fn len64(bytes: &[u8]) -> u64 {
  u64::try_from(bytes.len()).unwrap_or(u64::MAX)
}

/// A file, or as much of one as there is to read.
pub struct Source<'a> {
  /// In order of offset, with a gap between each and the next.
  segments: Vec<Segment<'a>>,
  size: Option<u64>,
}

/// How much is asked for at once where a little was wanted and more is likely to be.
const BLOCK: u64 = 4096;

impl<'a> Source<'a> {
  /// A whole file. Nothing lies past the end of `bytes`, so nothing more is ever asked for.
  pub fn whole(bytes: &'a [u8]) -> Self {
    let segments = match bytes {
      [] => Vec::new(),
      bytes => vec![Segment {
        offset: 0,
        bytes: Cow::Borrowed(bytes),
      }],
    };
    Source {
      segments,
      size: Some(len64(bytes)),
    }
  }

  /// Pieces of a file, in any order, and the size of the file when it is known. Pieces that
  /// touch or overlap are read as one.
  pub fn from_pieces(pieces: &[Piece<'a>], size: Option<u64>) -> Result<Self, MimeError> {
    let mut in_order: Vec<&Piece<'a>> = pieces
      .iter()
      .filter(|piece| !piece.bytes.is_empty())
      .collect();
    in_order.sort_by_key(|piece| piece.offset);

    let mut segments: Vec<Segment<'a>> = Vec::new();
    for piece in in_order {
      let end = piece
        .offset
        .checked_add(len64(piece.bytes))
        .ok_or_else(|| {
          MimeError::InvalidRanges(format!(
            "the piece at offset {} ends past the largest offset a file can have",
            piece.offset
          ))
        })?;
      if let Some(size) = size
        && end > size
      {
        return Err(MimeError::InvalidRanges(format!(
          "the piece at offset {} ends at {end}, past the file's size of {size}",
          piece.offset
        )));
      }
      match segments.last_mut() {
        Some(last) if piece.offset <= last.end() => last.absorb(piece)?,
        _ => segments.push(Segment {
          offset: piece.offset,
          bytes: Cow::Borrowed(piece.bytes),
        }),
      }
    }
    Ok(Source { segments, size })
  }

  /// The size of the file, when it is known.
  pub fn size(&self) -> Option<u64> {
    self.size
  }

  /// How many bytes were given from the start of the file without a gap.
  pub(crate) fn head_len(&self) -> u64 {
    match self.segments.first() {
      Some(first) if first.offset == 0 => first.end(),
      _ => 0,
    }
  }

  /// `length` bytes at `offset`.
  pub(crate) fn read(&self, offset: u64, length: usize) -> Result<&[u8], Missing> {
    // A read of nothing lacks nothing.
    if length == 0 {
      return Ok(&[]);
    }
    let end = u64::try_from(length)
      .ok()
      .and_then(|length| offset.checked_add(length))
      .ok_or(Missing::PastEnd)?;
    if self.size.is_some_and(|size| end > size) {
      return Err(Missing::PastEnd);
    }
    // The run that starts at or before `offset` and is nearest to it.
    let after = self
      .segments
      .partition_point(|segment| segment.offset <= offset);
    let holding = after
      .checked_sub(1)
      .and_then(|at| self.segments.get(at))
      .filter(|segment| offset < segment.end());
    let first_missing = match holding {
      Some(segment) if end <= segment.end() => {
        let start = usize::try_from(offset - segment.offset).map_err(|_| Missing::PastEnd)?;
        return Ok(&segment.bytes[start..start + length]);
      }
      Some(segment) => segment.end(),
      None => offset,
    };
    Err(Missing::NotGiven(ByteRange::forward(
      first_missing,
      end - first_missing,
    )))
  }

  /// The same file with its first `by` bytes left off, so that what stood at `by` stands at
  /// the start. It is how a file with something in front of it is read as the file it is.
  pub(crate) fn shifted(&self, by: u64) -> Source<'_> {
    let segments = self
      .segments
      .iter()
      .filter(|segment| segment.end() > by)
      .map(|segment| {
        let skipped = usize::try_from(by.saturating_sub(segment.offset)).unwrap_or(usize::MAX);
        Segment {
          offset: segment.offset.saturating_sub(by),
          bytes: Cow::Borrowed(segment.bytes.get(skipped..).unwrap_or_default()),
        }
      })
      .collect();
    Source {
      segments,
      size: self.size.map(|size| size.saturating_sub(by)),
    }
  }

  /// The start of the file, for as much of it as was given and no more than `most` bytes.
  pub(crate) fn head(&self, most: usize) -> &[u8] {
    match self.segments.first() {
      Some(first) if first.offset == 0 => &first.bytes[..first.bytes.len().min(most)],
      _ => &[],
    }
  }

  /// The bytes given from `offset` on, for as far as they go without a gap. Empty when the
  /// byte at `offset` was not given.
  #[cfg(feature = "audio")]
  pub(crate) fn given_from(&self, offset: u64) -> &[u8] {
    let after = self
      .segments
      .partition_point(|segment| segment.offset <= offset);
    after
      .checked_sub(1)
      .and_then(|at| self.segments.get(at))
      .and_then(|segment| {
        let start = usize::try_from(offset - segment.offset).ok()?;
        segment.bytes.get(start..)
      })
      .unwrap_or_default()
  }

  /// The whole file, when all of it was given.
  pub(crate) fn whole_file(&self) -> Option<&[u8]> {
    let first = self.segments.first()?;
    (first.offset == 0 && self.size == Some(first.end())).then_some(&*first.bytes)
  }

  /// `range` made at least a block long, and no longer than what is left of the file. It is
  /// what to ask for when a few bytes were wanted and more will be wanted after them, as the
  /// header of one box is followed by what the box holds and then by the next header. A range
  /// that carries straight on from the start of the file already given is made at least as
  /// long as that start, so that a reader working its way through a long header has each ask
  /// double the last and not creep.
  pub(crate) fn block(&self, range: ByteRange) -> ByteRange {
    if range.from_end {
      return range;
    }
    let given = self.head_len();
    let carries_on = range.offset == given;
    let length = range
      .length
      .max(BLOCK)
      .max(if carries_on { given } else { 0 });
    let length = self.size.map_or(length, |size| {
      length
        .min(size.saturating_sub(range.offset))
        .max(range.length)
    });
    ByteRange::forward(range.offset, length)
  }

  /// What is left of the file from `offset` on: to its end when its size is known, and the
  /// next megabyte when it is not.
  pub(crate) fn rest_from(&self, offset: u64) -> ByteRange {
    let length = match self.size {
      Some(size) => size.saturating_sub(offset),
      None => 1 << 20,
    };
    ByteRange::forward(offset, length)
  }

  /// `N` bytes at `offset`, as an array: a header of fixed size, whose fields are then read at
  /// places the compiler can check.
  pub(crate) fn array<const N: usize>(&self, offset: u64) -> Result<[u8; N], Missing> {
    self
      .read(offset, N)?
      .try_into()
      .map_err(|_| Missing::PastEnd)
  }
}

#[cfg(test)]
mod tests;
