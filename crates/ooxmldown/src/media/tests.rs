use super::*;

fn info(mime: &'static str, width: u32, height: u32) -> Option<ImageInfo> {
  Some(ImageInfo::sized(mime, width, height))
}

#[test]
fn a_png_states_its_size_in_its_first_chunk() {
  let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
  png.extend_from_slice(&[0, 0, 0, 13]);
  png.extend_from_slice(b"IHDR");
  png.extend_from_slice(&640u32.to_be_bytes());
  png.extend_from_slice(&480u32.to_be_bytes());
  assert_eq!(image_info(&png), info("image/png", 640, 480));
  // A signature with nothing after it is still a PNG.
  assert_eq!(
    image_info(&png[..8]),
    Some(ImageInfo::unsized_("image/png"))
  );
}

#[test]
fn a_jpeg_states_its_size_in_a_frame_header_after_other_segments() {
  let mut jpeg = vec![0xFF, 0xD8];
  // An application segment of 16 bytes, then a padded marker, then the frame header.
  jpeg.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
  jpeg.extend_from_slice(&[0; 14]);
  jpeg.extend_from_slice(&[0xFF, 0xFF, 0xC2, 0x00, 0x11, 0x08]);
  jpeg.extend_from_slice(&300u16.to_be_bytes());
  jpeg.extend_from_slice(&400u16.to_be_bytes());
  assert_eq!(image_info(&jpeg), info("image/jpeg", 400, 300));
  assert_eq!(
    image_info(&[0xFF, 0xD8, 0xFF, 0xE0]),
    Some(ImageInfo::unsized_("image/jpeg"))
  );
}

#[test]
fn gif_bmp_and_webp_state_their_sizes() {
  let mut gif = b"GIF89a".to_vec();
  gif.extend_from_slice(&[0x40, 0x01, 0xF0, 0x00]);
  assert_eq!(image_info(&gif), info("image/gif", 320, 240));

  let mut bmp = b"BM".to_vec();
  bmp.extend_from_slice(&[0; 16]);
  bmp.extend_from_slice(&100u32.to_le_bytes());
  // Top-down: the height is stored negative.
  bmp.extend_from_slice(&(-50i32).to_le_bytes());
  assert_eq!(image_info(&bmp), info("image/bmp", 100, 50));

  let mut webp = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
  webp.extend_from_slice(&[0; 8]);
  webp.extend_from_slice(&[0xFF, 0x03, 0x00, 0xFF, 0x01, 0x00]);
  assert_eq!(image_info(&webp), info("image/webp", 1024, 512));
}

#[test]
fn vector_formats_are_named_without_a_size() {
  let mut emf = vec![1, 0, 0, 0];
  emf.extend_from_slice(&[0; 36]);
  emf.extend_from_slice(b" EMF");
  assert_eq!(image_info(&emf), Some(ImageInfo::unsized_("image/emf")));
  assert_eq!(
    image_info(&[0xD7, 0xCD, 0xC6, 0x9A, 0, 0]),
    Some(ImageInfo::unsized_("image/wmf"))
  );
  assert_eq!(
    image_info(br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#),
    Some(ImageInfo::unsized_("image/svg+xml"))
  );
}

#[test]
fn what_is_not_a_picture_is_none() {
  assert_eq!(image_info(b""), None);
  assert_eq!(image_info(b"not a picture"), None);
  assert_eq!(image_info(&[0xFF, 0xD8]), None);
}
