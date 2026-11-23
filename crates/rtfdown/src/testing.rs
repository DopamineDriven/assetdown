//! Files for the tests, built by hand.

/// A compound file of these streams, each in the root storage.
pub(crate) fn compound(streams: &[(&str, &[u8])]) -> Vec<u8> {
  cfbdown::compound(streams).unwrap()
}

/// What `\objdata` holds for an embedded object: the header that goes before its own data.
pub(crate) fn object_data(class: &str, native: &[u8]) -> Vec<u8> {
  let text = |text: &str| {
    let mut out = u32::try_from(text.len() + 1)
      .unwrap()
      .to_le_bytes()
      .to_vec();
    out.extend(text.as_bytes());
    out.push(0);
    out
  };
  let mut out = 0x0501u32.to_le_bytes().to_vec();
  out.extend(2u32.to_le_bytes());
  out.extend(text(class));
  out.extend(0u32.to_le_bytes());
  out.extend(0u32.to_le_bytes());
  out.extend(u32::try_from(native.len()).unwrap().to_le_bytes());
  out.extend(native);
  out
}

/// Bytes as RTF writes them: two hex digits to a byte, broken into lines.
pub(crate) fn hex(bytes: &[u8]) -> String {
  let digits: Vec<String> = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  digits
    .chunks(32)
    .map(|line| line.concat())
    .collect::<Vec<_>>()
    .join("\r\n")
}

/// A document with this after its opening.
pub(crate) fn document(body: &str) -> Vec<u8> {
  format!("{{\\rtf1\\ansi\\deff0 {body}}}").into_bytes()
}

/// An embedded object of this class with this in its `\objdata`, and a picture standing for it.
pub(crate) fn object(class: &str, data: &[u8]) -> String {
  format!(
    "{{\\object\\objemb{{\\*\\objclass {class}}}\\objw100\\objh100{{\\*\\objdata {}}}{{\\result{{\\pict\\wmetafile8 0102}}}}}}",
    hex(data)
  )
}
