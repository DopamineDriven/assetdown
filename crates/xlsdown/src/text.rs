//! Strings as a workbook keeps them: a count of characters, a byte that says whether each is
//! one byte or two, and after the characters whatever formatting the string carries. A string
//! too long for its record runs on in the next, which says again how its characters are kept.

/// The data of a record and of those that carry it on, read as one.
pub(crate) struct Parts<'a> {
  parts: Vec<&'a [u8]>,
  part: usize,
  at: usize,
}

impl<'a> Parts<'a> {
  pub fn of(first: &'a [u8], more: Vec<&'a [u8]>) -> Self {
    let mut parts = vec![first];
    parts.extend(more);
    Parts {
      parts,
      part: 0,
      at: 0,
    }
  }

  pub fn one(data: &'a [u8]) -> Self {
    Parts::of(data, Vec::new())
  }

  fn current(&self) -> &'a [u8] {
    self.parts.get(self.part).copied().unwrap_or(&[])
  }

  /// Moves on to the next part with something in it once this one is used up. False when
  /// nothing is left.
  fn settle(&mut self) -> bool {
    while self.at >= self.current().len() {
      if self.part + 1 >= self.parts.len() {
        return false;
      }
      self.part += 1;
      self.at = 0;
    }
    true
  }

  pub fn u8(&mut self) -> Option<u8> {
    if !self.settle() {
      return None;
    }
    let byte = self.current()[self.at];
    self.at += 1;
    Some(byte)
  }

  pub fn u16(&mut self) -> Option<u16> {
    Some(u16::from_le_bytes([self.u8()?, self.u8()?]))
  }

  pub fn u32(&mut self) -> Option<u32> {
    Some(u32::from_le_bytes([
      self.u8()?,
      self.u8()?,
      self.u8()?,
      self.u8()?,
    ]))
  }

  fn skip(&mut self, mut count: usize) -> Option<()> {
    while count > 0 {
      if !self.settle() {
        return None;
      }
      let take = count.min(self.current().len() - self.at);
      self.at += take;
      count -= take;
    }
    Some(())
  }

  /// `count` characters, each two bytes when `wide` and one when not.
  fn characters(&mut self, count: usize, mut wide: bool) -> Option<String> {
    let mut units: Vec<u16> = Vec::with_capacity(count);
    while units.len() < count {
      if self.at >= self.current().len() {
        // The characters run on in the next record, which opens with how they are kept there.
        self.part += 1;
        wide = *self.current().first()? & 1 == 1;
        self.at = 1;
        continue;
      }
      let rest = &self.current()[self.at..];
      if wide {
        units.push(u16::from_le_bytes([rest[0], *rest.get(1)?]));
        self.at += 2;
      } else {
        units.push(u16::from(rest[0]));
        self.at += 1;
      }
    }
    Some(String::from_utf16_lossy(&units))
  }

  /// The characters of a string whose count has been read. What it carries besides them (runs
  /// of formatting, and a note on how it is read aloud) is passed over.
  pub fn counted(&mut self, count: usize) -> Option<String> {
    let flags = self.u8()?;
    let runs = match flags & 0x08 {
      0 => 0,
      _ => usize::from(self.u16()?),
    };
    let extra = match flags & 0x04 {
      0 => 0,
      _ => usize::try_from(self.u32()?).ok()?,
    };
    let text = self.characters(count, flags & 0x01 != 0)?;
    self.skip(runs * 4)?;
    self.skip(extra)?;
    Some(text)
  }

  /// A string with its count in two bytes.
  pub fn string(&mut self) -> Option<String> {
    let count = usize::from(self.u16()?);
    self.counted(count)
  }

  /// A string with its count in one byte.
  pub fn short_string(&mut self) -> Option<String> {
    let count = usize::from(self.u8()?);
    self.counted(count)
  }
}
