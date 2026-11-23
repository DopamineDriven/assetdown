//! What an embedded object keeps a file in. A compound file made for an object holds the
//! file whole in a stream of a known name, or as the packager left it: a file dropped into
//! a document, with its name.

/// The streams of an object's compound file that hold a file whole, in the order they are
/// looked for: a mail attachment, a document of 2007 or later, and what Acrobat embeds.
pub const HOLDING: [&str; 3] = ["AttachContents", "Package", "CONTENTS"];

fn u32_of(bytes: &mut &[u8]) -> Option<u32> {
  let value = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
  *bytes = &bytes[4..];
  Some(value)
}

/// Text up to a zero byte, and the bytes after it.
fn terminated<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
  let end = bytes.iter().position(|byte| *byte == 0)?;
  let text = &bytes[..end];
  *bytes = &bytes[end + 1..];
  Some(text)
}

fn text(bytes: &[u8]) -> Option<String> {
  // Names are kept in the code page of the machine that wrote them; the Western one is the
  // usual, and its letters are these.
  let text: String = bytes.iter().map(|byte| char::from(*byte)).collect();
  let text = text.trim();
  (!text.is_empty()).then(|| text.to_string())
}

/// A file dropped into a document, as the packager keeps it: its label, the path it came
/// from, a path it was copied to, and then its bytes with their length before them. Gives
/// the label, which is the file's name, and the bytes.
pub fn packaged(mut data: &[u8]) -> Option<(Option<String>, &[u8])> {
  data = data.get(2..)?;
  let label = terminated(&mut data)?;
  let _from = terminated(&mut data)?;
  let _kind = u32_of(&mut data)?;
  let _length_of_path = u32_of(&mut data)?;
  let _copied_to = terminated(&mut data)?;
  let length = usize::try_from(u32_of(&mut data)?).ok()?;
  Some((text(label), data.get(..length)?))
}

#[cfg(test)]
mod tests;
