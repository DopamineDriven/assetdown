//! The rarer images: the JPEG family beyond JPEG itself, and what cameras record.

use super::*;
use crate::source::Piece;
use crate::testing::{TiffValue, ftyp, jpeg, jpeg_frame, tiff, tiff_at};

fn type_of(bytes: &[u8]) -> Option<MediaType> {
  match sniff(bytes) {
    Sniffed::Named { media_type, .. } => Some(media_type),
    Sniffed::Unknown { .. } => None,
  }
}

/// What the head of a file is, when its size is not known.
fn sniff_head(head: &[u8]) -> Sniffed {
  let pieces = [Piece {
    offset: 0,
    bytes: head,
  }];
  sniff_source(&Source::from_pieces(&pieces, None).unwrap())
}

const JP2_SIGNATURE: &[u8] = b"\0\0\0\x0CjP  \r\n\x87\n";
const JXL_SIGNATURE: &[u8] = b"\0\0\0\x0CJXL \r\n\x87\n";

#[test]
fn a_jpeg_xl_is_named_bare_or_in_its_container() {
  let bare = [&[0xFF, 0x0A][..], &[0x7A, 0x16, 0x10, 0x90, 0x01, 0x00]].concat();
  assert_eq!(type_of(&bare), Some(MediaType::Jxl));
  let boxed = [JXL_SIGNATURE, &ftyp(b"jxl ", &[b"jxl "])].concat();
  assert_eq!(type_of(&boxed), Some(MediaType::Jxl));
}

#[test]
fn the_two_bytes_a_jpeg_xl_opens_with_are_not_enough_in_front_of_text() {
  // In an old encoding of one byte to a character, these are a letter and a line ending.
  let text = [
    &[0xFF, 0x0A][..],
    "a line of words, and then another line of words\n"
      .repeat(4)
      .as_bytes(),
  ]
  .concat();
  assert_ne!(type_of(&text), Some(MediaType::Jxl));
}

#[test]
fn the_jpeg_2000_family_is_told_apart_by_its_brand() {
  let file =
    |major: &[u8; 4], compatible: &[&[u8; 4]]| [JP2_SIGNATURE, &ftyp(major, compatible)].concat();
  assert_eq!(type_of(&file(b"jp2 ", &[b"jp2 "])), Some(MediaType::Jp2));
  assert_eq!(
    type_of(&file(b"jpx ", &[b"jpx ", b"jp2 "])),
    Some(MediaType::Jpx)
  );
  assert_eq!(
    type_of(&file(b"jpm ", &[b"jpm ", b"jp2 "])),
    Some(MediaType::Jpm)
  );
  assert_eq!(type_of(&file(b"jph ", &[b"jph "])), Some(MediaType::Jph));
  // Motion JPEG 2000 is video, and no type here.
  assert_eq!(type_of(&file(b"mjp2", &[b"mjp2"])), None);
  // The signature alone says the family, and the brand is asked for.
  let head = sniff_head(JP2_SIGNATURE);
  assert!(
    matches!(
      head,
      Sniffed::Named {
        media_type: MediaType::Jp2,
        needs: Some(_),
        ..
      }
    ),
    "{head:?}"
  );
}

#[test]
fn a_jpeg_2000_codestream_with_no_boxes_around_it_is_named() {
  let codestream = [&b"\xFF\x4F\xFF\x51\0\x2F\0\0"[..], &[0; 40]].concat();
  assert_eq!(type_of(&codestream), Some(MediaType::J2c));
}

#[test]
fn a_jpeg_xr_is_named_in_either_version() {
  assert_eq!(
    type_of(b"II\xBC\x01\x20\0\0\0\0\0\0\0"),
    Some(MediaType::Jxr)
  );
  assert_eq!(type_of(b"II\xBC\0\x20\0\0\0\0\0\0\0"), Some(MediaType::Jxr));
}

#[test]
fn a_jpeg_ls_is_told_from_a_jpeg_by_its_frame_header() {
  let lossless = jpeg(&[(0xF7, jpeg_frame(640, 480, 3))]);
  assert_eq!(type_of(&lossless), Some(MediaType::Jls));
  let after_jfif = jpeg(&[
    (0xE0, b"JFIF\0\x01\x02\0\0\x01\0\x01\0\0".to_vec()),
    (0xF7, jpeg_frame(8, 8, 1)),
  ]);
  assert_eq!(type_of(&after_jfif), Some(MediaType::Jls));
  assert_eq!(
    type_of(&jpeg(&[(0xC0, jpeg_frame(640, 480, 3))])),
    Some(MediaType::Jpeg)
  );
}

#[test]
fn a_jpeg_whose_frame_header_was_not_given_is_a_jpeg_and_asks_for_nothing() {
  let file = jpeg(&[(0xE1, vec![0; 9000]), (0xF7, jpeg_frame(640, 480, 3))]);
  let head = sniff_head(&file[..4096]);
  assert_eq!(
    head,
    Sniffed::Named {
      media_type: MediaType::Jpeg,
      evidence: Evidence::Signature,
      needs: None,
      retry: None,
    }
  );
}

/// One directory of a TIFF: whether it is the full picture or a smaller one of it, its size,
/// and what its samples mean.
fn directory(reduced: bool, width: u32, height: u32, photometric: u16) -> Vec<(u16, TiffValue)> {
  vec![
    (254, TiffValue::Long(vec![u32::from(reduced)])),
    (256, TiffValue::Long(vec![width])),
    (257, TiffValue::Long(vec![height])),
    (262, TiffValue::Short(vec![photometric])),
  ]
}

/// What a color filter over a sensor records: one sample to the pixel, in a mosaic.
const MOSAIC: u16 = 32803;

/// A raw file as most cameras lay it out: a small picture first, with the maker's name, and
/// the sensor's recording in a directory of its own.
fn camera_file(make: &'static str) -> Vec<u8> {
  let mut first = directory(true, 160, 120, 2);
  first.push((271, TiffValue::Text(make)));
  first.push((
    330,
    TiffValue::Directories(vec![
      directory(true, 1620, 1080, 6),
      directory(false, 6032, 4032, MOSAIC),
    ]),
  ));
  tiff(true, &[first])
}

#[test]
fn a_canon_raw_is_told_by_the_letters_after_its_header() {
  let mut file = tiff_at(true, 16, &[directory(false, 5184, 3456, 6)]);
  file[8..12].copy_from_slice(b"CR\x02\0");
  assert_eq!(type_of(&file), Some(MediaType::Cr2));
}

#[test]
fn a_digital_negative_is_told_by_its_version_tag() {
  let mut first = directory(true, 256, 171, 2);
  first.push((0xC612, TiffValue::Bytes(vec![1, 4, 0, 0])));
  assert_eq!(type_of(&tiff(true, &[first])), Some(MediaType::Dng));
}

#[test]
fn a_nikon_and_a_sony_raw_are_told_by_the_maker_of_a_mosaic() {
  assert_eq!(
    type_of(&camera_file("NIKON CORPORATION")),
    Some(MediaType::Nef)
  );
  assert_eq!(type_of(&camera_file("SONY")), Some(MediaType::Arw));
  // A mosaic by a maker that is not told apart is a raw file and no more.
  assert_eq!(type_of(&camera_file("PENTAX")), Some(MediaType::Raw));
  let mut whole = directory(false, 4000, 3000, MOSAIC);
  whole.push((271, TiffValue::Text("SAMSUNG")));
  assert_eq!(type_of(&tiff(false, &[whole])), Some(MediaType::Raw));
}

#[test]
fn a_tiff_with_no_mosaic_in_it_is_a_tiff_whoever_made_it() {
  let mut scan = directory(false, 4000, 3000, 2);
  scan.push((271, TiffValue::Text("NIKON CORPORATION")));
  assert_eq!(type_of(&tiff(true, &[scan])), Some(MediaType::Tiff));
  assert_eq!(
    type_of(&tiff(false, &[directory(false, 640, 480, 1)])),
    Some(MediaType::Tiff)
  );
}

#[test]
fn a_tiff_whose_directory_was_not_given_is_a_tiff_and_asks_for_it() {
  let file = tiff_at(true, 50_000, &[directory(false, 640, 480, 2)]);
  let head = sniff_head(&file[..4096]);
  let Sniffed::Named {
    media_type: MediaType::Tiff,
    needs: Some(needs),
    ..
  } = head
  else {
    panic!("{head:?}");
  };
  assert_eq!(needs.offset, 50_000);
}

#[test]
fn camera_raw_with_a_signature_of_its_own_is_named() {
  assert_eq!(
    type_of(b"FUJIFILMCCD-RAW 0201FF129502X-T2\0\0\0\0"),
    Some(MediaType::Raf)
  );
  assert_eq!(type_of(b"IIRO\x08\0\0\0\x11\0\0\0"), Some(MediaType::Orf));
  assert_eq!(type_of(b"IIRS\x08\0\0\0\x11\0\0\0"), Some(MediaType::Orf));
  assert_eq!(type_of(b"MMOR\0\0\0\x08\0\x11\0\0"), Some(MediaType::Orf));
  assert_eq!(
    type_of(b"IIU\0\x18\0\0\0\x88\xE7\x74\xD8"),
    Some(MediaType::Rw2)
  );
  assert_eq!(
    type_of(&ftyp(b"crx ", &[b"crx ", b"isom"])),
    Some(MediaType::Cr3)
  );
}

#[test]
fn the_strings_the_rarer_images_also_go_by_are_accepted() {
  assert_eq!(
    MediaType::from_mime("image/vnd.ms-photo"),
    Some(MediaType::Jxr)
  );
  assert_eq!(
    MediaType::from_mime("image/x-jp2-codestream"),
    Some(MediaType::J2c)
  );
  assert_eq!(
    MediaType::from_mime("image/x-panasonic-raw2"),
    Some(MediaType::Rw2)
  );
  assert_eq!(crate::media_types_of("j2k"), [MediaType::J2c]);
  assert_eq!(crate::media_types_of("wdp"), [MediaType::Jxr]);
  assert_eq!(crate::media_types_of("nrw"), [MediaType::Nef]);
}
