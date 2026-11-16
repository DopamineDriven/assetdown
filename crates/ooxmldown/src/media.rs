//! What an embedded picture is, read from its first bytes.
//!
//! An Office package keeps a picture in the format it was inserted in. The part's extension and
//! content type are whatever the writing application put there, so the bytes are asked instead.

/// A picture's format, and its size in pixels when the format is a raster one and its header
/// states the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
  pub mime: &'static str,
  pub width: Option<u32>,
  pub height: Option<u32>,
}

impl ImageInfo {
  fn sized(mime: &'static str, width: u32, height: u32) -> Self {
    ImageInfo {
      mime,
      width: Some(width),
      height: Some(height),
    }
  }

  fn unsized_(mime: &'static str) -> Self {
    ImageInfo {
      mime,
      width: None,
      height: None,
    }
  }
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
  Some(u32::from(u16::from_be_bytes([
    *b.get(at)?,
    *b.get(at + 1)?,
  ])))
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
  Some(u32::from(u16::from_le_bytes([
    *b.get(at)?,
    *b.get(at + 1)?,
  ])))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn le24(b: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes([
    *b.get(at)?,
    *b.get(at + 1)?,
    *b.get(at + 2)?,
    0,
  ]))
}

/// A JPEG's size is in its first start-of-frame segment, which follows any number of others.
fn jpeg_size(b: &[u8]) -> Option<(u32, u32)> {
  let mut at = 2;
  loop {
    // Markers may be padded with any number of 0xFF bytes.
    while *b.get(at)? == 0xFF {
      at += 1;
    }
    let marker = *b.get(at)?;
    at += 1;
    // A start-of-frame is C0 to CF, except the Huffman table, JPG and arithmetic markers.
    if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
      return Some((be16(b, at + 5)?, be16(b, at + 3)?));
    }
    // Restart and start/end markers stand alone; every other segment states its length.
    if !matches!(marker, 0xD0..=0xD9 | 0x01) {
      at += usize::try_from(be16(b, at)?).ok()?;
    }
  }
}

/// The three forms a WebP takes, each with the size in a different place.
fn webp_size(b: &[u8]) -> Option<(u32, u32)> {
  match b.get(12..16)? {
    b"VP8 " => Some((le16(b, 26)? & 0x3FFF, le16(b, 28)? & 0x3FFF)),
    b"VP8L" => {
      let bits = le32(b, 21)?;
      Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
    }
    b"VP8X" => Some((le24(b, 24)? + 1, le24(b, 27)? + 1)),
    _ => None,
  }
}

/// What `bytes` is, or `None` when it is no picture format an Office document embeds.
pub fn image_info(bytes: &[u8]) -> Option<ImageInfo> {
  let b = bytes;
  if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
    return Some(match (be32(b, 16), be32(b, 20)) {
      (Some(width), Some(height)) => ImageInfo::sized("image/png", width, height),
      _ => ImageInfo::unsized_("image/png"),
    });
  }
  if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
    return Some(match jpeg_size(b) {
      Some((width, height)) => ImageInfo::sized("image/jpeg", width, height),
      None => ImageInfo::unsized_("image/jpeg"),
    });
  }
  if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
    return Some(match (le16(b, 6), le16(b, 8)) {
      (Some(width), Some(height)) => ImageInfo::sized("image/gif", width, height),
      _ => ImageInfo::unsized_("image/gif"),
    });
  }
  if b.starts_with(b"BM") && b.len() >= 26 {
    // The height is negative for a top-down bitmap.
    let height = le32(b, 22).map(|h| h.cast_signed().unsigned_abs());
    return Some(match (le32(b, 18), height) {
      (Some(width), Some(height)) => ImageInfo::sized("image/bmp", width, height),
      _ => ImageInfo::unsized_("image/bmp"),
    });
  }
  if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
    return Some(match webp_size(b) {
      Some((width, height)) => ImageInfo::sized("image/webp", width, height),
      None => ImageInfo::unsized_("image/webp"),
    });
  }
  if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") {
    return Some(ImageInfo::unsized_("image/tiff"));
  }
  // The vector formats Word has always embedded.
  if b.starts_with(&[1, 0, 0, 0]) && b.get(40..44) == Some(b" EMF") {
    return Some(ImageInfo::unsized_("image/emf"));
  }
  if b.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
    return Some(ImageInfo::unsized_("image/wmf"));
  }
  let head = &b[..b.len().min(1024)];
  if head.windows(4).any(|w| w == b"<svg") {
    return Some(ImageInfo::unsized_("image/svg+xml"));
  }
  None
}

#[cfg(test)]
mod tests;
