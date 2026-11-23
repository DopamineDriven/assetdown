//! The stream of records a workbook is: each a number, the length of its data, and the data.

pub(crate) const BOF: u16 = 0x0809;
pub(crate) const EOF: u16 = 0x000A;
/// More of the record before it, which was too long for one.
pub(crate) const CONTINUE: u16 = 0x003C;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Record<'a> {
  pub id: u16,
  pub data: &'a [u8],
}

impl<'a> Record<'a> {
  pub fn u16_at(&self, at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
      self.data.get(at..at + 2)?.try_into().ok()?,
    ))
  }

  pub fn u32_at(&self, at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
      self.data.get(at..at + 4)?.try_into().ok()?,
    ))
  }
}

/// The records of a stream from one place on. It ends at the end of the stream, or at a
/// record that would run past it.
pub(crate) struct Records<'a> {
  stream: &'a [u8],
  at: usize,
}

impl<'a> Records<'a> {
  pub fn from(stream: &'a [u8], at: usize) -> Self {
    Records { stream, at }
  }

  /// The record at `at`, and where the one after it starts.
  fn read(&self, at: usize) -> Option<(Record<'a>, usize)> {
    let header = self.stream.get(at..at.checked_add(4)?)?;
    let id = u16::from_le_bytes([header[0], header[1]]);
    let length = usize::from(u16::from_le_bytes([header[2], header[3]]));
    let data = self.stream.get(at + 4..at + 4 + length)?;
    Some((Record { id, data }, at + 4 + length))
  }

  /// The number of the record that comes next, without taking it.
  pub fn next_id(&self) -> Option<u16> {
    self.read(self.at).map(|(record, _)| record.id)
  }

  /// The data of the records that carry on the one just taken, in order.
  pub fn continued(&mut self) -> Vec<&'a [u8]> {
    let mut more = Vec::new();
    while self.next_id() == Some(CONTINUE) {
      more.extend(self.next().map(|record| record.data));
    }
    more
  }
}

impl<'a> Iterator for Records<'a> {
  type Item = Record<'a>;

  fn next(&mut self) -> Option<Record<'a>> {
    let (record, next) = self.read(self.at)?;
    self.at = next;
    Some(record)
  }
}
