//! Whether a text is JSON. `[` and `{` begin a good many things that are not, so the text is
//! read as JSON would be, and it is JSON when it reads through.

/// What a text that begins like JSON turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reading {
  /// One value, and nothing after it.
  Value,
  /// A value on each line: JSON Lines.
  Lines,
  Neither,
}

/// No value is nested deeper than this.
const DEEPEST: usize = 256;

/// Why reading a value stopped.
enum Stop {
  /// The text ended inside the value.
  Ended,
  /// The text is not JSON here.
  Wrong,
}

struct Cursor<'a> {
  bytes: &'a [u8],
  at: usize,
}

impl Cursor<'_> {
  fn skip_space(&mut self) {
    while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
      self.at += 1;
    }
  }

  fn peek(&self) -> Result<u8, Stop> {
    self.bytes.get(self.at).copied().ok_or(Stop::Ended)
  }

  fn word(&mut self, word: &[u8]) -> Result<(), Stop> {
    let rest = &self.bytes[self.at..];
    if rest.starts_with(word) {
      self.at += word.len();
      Ok(())
    } else if word.starts_with(rest) {
      Err(Stop::Ended)
    } else {
      Err(Stop::Wrong)
    }
  }

  fn string(&mut self) -> Result<(), Stop> {
    self.at += 1;
    loop {
      match self.peek()? {
        b'"' => {
          self.at += 1;
          return Ok(());
        }
        b'\\' => self.at += 2,
        byte if byte < 0x20 => return Err(Stop::Wrong),
        _ => self.at += 1,
      }
    }
  }

  fn number(&mut self) -> Result<(), Stop> {
    let start = self.at;
    while matches!(
      self.bytes.get(self.at),
      Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
    ) {
      self.at += 1;
    }
    let digits = self.bytes[start..self.at].iter().any(u8::is_ascii_digit);
    if digits { Ok(()) } else { Err(Stop::Wrong) }
  }

  fn value(&mut self, depth: usize) -> Result<(), Stop> {
    if depth > DEEPEST {
      return Err(Stop::Wrong);
    }
    self.skip_space();
    match self.peek()? {
      b'"' => self.string(),
      b't' => self.word(b"true"),
      b'f' => self.word(b"false"),
      b'n' => self.word(b"null"),
      b'-' | b'0'..=b'9' => self.number(),
      open @ (b'{' | b'[') => {
        let close = if open == b'{' { b'}' } else { b']' };
        self.at += 1;
        self.skip_space();
        if self.peek()? == close {
          self.at += 1;
          return Ok(());
        }
        loop {
          // An object's member is a name, a colon and a value.
          if open == b'{' {
            self.skip_space();
            if self.peek()? != b'"' {
              return Err(Stop::Wrong);
            }
            self.string()?;
            self.skip_space();
            if self.peek()? != b':' {
              return Err(Stop::Wrong);
            }
            self.at += 1;
          }
          self.value(depth + 1)?;
          self.skip_space();
          match self.peek()? {
            b',' => self.at += 1,
            byte if byte == close => {
              self.at += 1;
              return Ok(());
            }
            _ => return Err(Stop::Wrong),
          }
        }
      }
      _ => Err(Stop::Wrong),
    }
  }
}

/// What `text` is, read as JSON. `cut` says the text stops short of the file's end, and then a
/// value that reads true as far as the text goes is taken for one.
pub(crate) fn read(text: &str, cut: bool) -> Reading {
  let mut cursor = Cursor {
    bytes: text.as_bytes(),
    at: 0,
  };
  match cursor.value(0) {
    Err(Stop::Ended) if cut => return Reading::Value,
    Err(_) => return Reading::Neither,
    Ok(()) => {}
  }
  cursor.skip_space();
  if cursor.at == text.len() {
    return Reading::Value;
  }
  // Something follows the first value. It is JSON Lines when every line is a value.
  let mut lines = text
    .lines()
    .filter(|line| !line.trim().is_empty())
    .peekable();
  let mut count = 0;
  while let Some(line) = lines.next() {
    let mut cursor = Cursor {
      bytes: line.as_bytes(),
      at: 0,
    };
    let last = lines.peek().is_none();
    match cursor.value(0) {
      Ok(()) => {
        cursor.skip_space();
        if cursor.at != line.len() {
          return Reading::Neither;
        }
      }
      // The last line may be cut off with the text.
      Err(Stop::Ended) if cut && last => {}
      Err(_) => return Reading::Neither,
    }
    count += 1;
  }
  if count >= 2 {
    Reading::Lines
  } else {
    Reading::Neither
  }
}
