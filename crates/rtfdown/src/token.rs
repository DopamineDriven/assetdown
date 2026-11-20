//! RTF as a run of tokens: groups opening and closing, control words, and text.

/// One token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Token<'a> {
  /// `{`: a group opens.
  Open,
  /// `}`: the group closes.
  Close,
  /// A backslash and letters, with the number written after them when there is one: `\li-720`.
  Word {
    name: &'a str,
    parameter: Option<i32>,
  },
  /// A backslash and one character that is not a letter: `\{`, `\~`, `\*`.
  Symbol(u8),
  /// `\'hh`: one byte of text, written as two hex digits.
  Hex(u8),
  /// The bytes after `\binN`, as they are.
  Binary(&'a [u8]),
  /// A run of text, with no line end in it: a line end in RTF is not text.
  Text(&'a [u8]),
}

/// The tokens of a file, in order.
pub(crate) struct Tokens<'a> {
  bytes: &'a [u8],
  at: usize,
}

impl<'a> Tokens<'a> {
  pub(crate) fn over(bytes: &'a [u8]) -> Self {
    Tokens { bytes, at: 0 }
  }

  /// A control word's letters, its number, and the one space that may end it.
  fn word(&mut self) -> Token<'a> {
    let letters = |byte: &&u8| byte.is_ascii_alphabetic();
    let start = self.at;
    self.at += self.bytes[start..].iter().take_while(letters).count();
    // Letters only, so the bytes are text.
    let name = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default();
    let digits_at = self.at;
    if self.bytes.get(self.at) == Some(&b'-') {
      self.at += 1;
    }
    let digits = self.bytes[self.at..]
      .iter()
      .take_while(|byte| byte.is_ascii_digit())
      .count();
    let parameter = match digits {
      // A hyphen with no digit after it is text, and not part of the word.
      0 => {
        self.at = digits_at;
        None
      }
      _ => {
        self.at += digits;
        let written = std::str::from_utf8(&self.bytes[digits_at..self.at]).unwrap_or_default();
        // A number too long to hold is held as the largest there is, with its sign.
        Some(written.parse().unwrap_or(match written.starts_with('-') {
          true => i32::MIN,
          false => i32::MAX,
        }))
      }
    };
    if self.bytes.get(self.at) == Some(&b' ') {
      self.at += 1;
    }
    if name == "bin" {
      let count = parameter
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0);
      let end = self.at.saturating_add(count).min(self.bytes.len());
      let held = &self.bytes[self.at..end];
      self.at = end;
      return Token::Binary(held);
    }
    Token::Word { name, parameter }
  }
}

impl<'a> Iterator for Tokens<'a> {
  type Item = Token<'a>;

  fn next(&mut self) -> Option<Token<'a>> {
    loop {
      let byte = *self.bytes.get(self.at)?;
      match byte {
        b'{' => {
          self.at += 1;
          return Some(Token::Open);
        }
        b'}' => {
          self.at += 1;
          return Some(Token::Close);
        }
        b'\r' | b'\n' => self.at += 1,
        b'\\' => {
          let next = *self.bytes.get(self.at + 1)?;
          self.at += 1;
          if next.is_ascii_alphabetic() {
            return Some(self.word());
          }
          self.at += 1;
          if next != b'\'' {
            return Some(Token::Symbol(next));
          }
          // The two bytes after the apostrophe are taken whatever they are; where they are
          // not both hex digits, or the file ends first, nothing is made of them.
          let digit = |at: usize| char::from(*self.bytes.get(at)?).to_digit(16);
          let pair = digit(self.at).zip(digit(self.at + 1));
          self.at = (self.at + 2).min(self.bytes.len());
          if let Some((high, low)) = pair {
            // Two hex digits make a number under 256.
            return Some(Token::Hex((high * 16 + low) as u8));
          }
        }
        _ => {
          let start = self.at;
          let plain = |byte: &&u8| !matches!(byte, b'{' | b'}' | b'\\' | b'\r' | b'\n');
          self.at += self.bytes[start..].iter().take_while(plain).count();
          return Some(Token::Text(&self.bytes[start..self.at]));
        }
      }
    }
  }
}

#[cfg(test)]
mod tests;
