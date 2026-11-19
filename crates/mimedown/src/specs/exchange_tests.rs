//! The exchange a fetcher has with `specs`: the head of a file, then whatever the answer asks
//! for, until it asks for nothing. Whatever the head and however little is known of the file's
//! size, the exchange has to end, and end with the answer the whole file gives.

use super::*;
use crate::source::Piece;
use crate::testing::{
  HeifItem, LengthIn, Sizes, TiffValue, XMP_PACKET, boxed, compound, ebml, exif_block, ftyp, heif,
  heif_size, jpeg, jpeg_frame, office_package, pdf, png_chunk, png_header, png_of, tiff, tiff_at,
  zip,
};

/// The most fetches an exchange may take.
const MOST_STEPS: usize = 8;

/// The answer a fetcher ends with, how many fetches after the head it took, and how many bytes
/// it fetched in all.
fn exchange(label: &str, file: &[u8], head: usize, size_known: bool) -> (Specs, usize, u64) {
  let length = u64::try_from(file.len()).unwrap();
  let at = |offset: u64| usize::try_from(offset).unwrap();
  let mut fetched = vec![(0, u64::try_from(head.min(file.len())).unwrap())];
  let mut size = size_known.then_some(length);
  for step in 0..=MOST_STEPS {
    let pieces: Vec<Piece<'_>> = fetched
      .iter()
      .map(|(offset, length)| Piece {
        offset: *offset,
        bytes: &file[at(*offset)..at(*offset + *length)],
      })
      .collect();
    let found = specs_source(&Source::from_pieces(&pieces, size).unwrap());
    let Some(range) = found.needs else {
      let bytes = fetched.iter().map(|(_, length)| length).sum();
      return (found, step, bytes);
    };
    let start = if range.from_end {
      length.saturating_sub(range.offset)
    } else {
      range.offset
    };
    let end = start.saturating_add(range.length);
    // A range counted from the end, or one that runs past it, is how a fetcher learns how
    // long the file is.
    if range.from_end || end >= length {
      size = Some(length);
    }
    let (start, end) = (start.min(length), end.min(length));
    if start < end {
      fetched.push((start, end - start));
    }
  }
  panic!(
    "{label}, a head of {head} bytes, size known: {size_known}: the exchange did not settle in {MOST_STEPS} fetches, having fetched {fetched:?}"
  );
}

/// Every exchange over `file` ends with what the whole file gives.
fn settles(label: &str, file: &[u8]) {
  let whole = specs(file);
  assert_eq!(
    whole.needs, None,
    "{label}: the whole file asks for nothing"
  );
  for head in [16, 300, 4096] {
    for size_known in [true, false] {
      let (found, steps, _) = exchange(label, file, head, size_known);
      let case =
        format!("{label}, a head of {head} bytes, size known: {size_known}, {steps} fetches");
      assert_eq!(found.media_type, whole.media_type, "{case}");
      assert_eq!(found.facts, whole.facts, "{case}");
      assert_eq!(found.warnings, whole.warnings, "{case}");
    }
  }
}

fn picture() -> Vec<u8> {
  png_chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0, 0, 0, 1, 0, 1])
}

#[test]
fn images_settle() {
  let note = png_chunk(b"tEXt", &[&b"Comment\0"[..], &[b'x'; 6000]].concat());
  settles(
    "png",
    &png_of(&[
      png_header(640, 480, 8, 6),
      note.clone(),
      picture(),
      png_chunk(b"IEND", &[]),
    ]),
  );
  let animation = png_chunk(b"acTL", &[0, 0, 0, 3, 0, 0, 0, 0]);
  settles(
    "apng",
    &png_of(&[
      png_header(64, 64, 8, 6),
      note,
      animation,
      picture(),
      png_chunk(b"IEND", &[]),
    ]),
  );

  let frame = [
    &[0x2C][..],
    &[0; 4],
    &[1, 0, 1, 0, 0],
    &[2, 2, 0x4C, 0x01, 0],
  ]
  .concat();
  let gif = [
    &b"GIF89a\x40\0\x40\0\x81\0\0"[..],
    &[0; 12],
    &frame.repeat(400),
    &[0x3B],
  ]
  .concat();
  settles("gif", &gif);

  let notes = (
    0xE1,
    [&b"Exif\0\0"[..], &exif_block(true), &[0; 9000]].concat(),
  );
  settles("jpeg", &jpeg(&[notes, (0xC0, jpeg_frame(640, 480, 3))]));

  let page = |width: u32| {
    vec![
      (256, TiffValue::Long(vec![width])),
      (257, TiffValue::Long(vec![480])),
      (262, TiffValue::Short(vec![2])),
      (277, TiffValue::Short(vec![3])),
    ]
  };
  settles("tiff", &tiff_at(true, 50_000, &[page(640), page(320)]));

  let chunk = |kind: &[u8; 4], payload: &[u8]| {
    [
      &kind[..],
      &u32::try_from(payload.len()).unwrap().to_le_bytes(),
      payload,
      &vec![0; payload.len() % 2],
    ]
    .concat()
  };
  let body = [
    chunk(b"VP8X", &[0x08, 0, 0, 0, 63, 0, 0, 63, 0, 0]),
    chunk(
      b"VP8 ",
      &[
        &[0x30, 0x01, 0x00, 0x9D, 0x01, 0x2A, 64, 0, 64, 0][..],
        &[0; 9000],
      ]
      .concat(),
    ),
    chunk(b"EXIF", &exif_block(true)),
  ]
  .concat();
  settles(
    "webp",
    &[
      &b"RIFF"[..],
      &u32::try_from(body.len() + 4).unwrap().to_le_bytes(),
      b"WEBP",
      &body,
    ]
    .concat(),
  );

  let mut item = HeifItem::picture(1, b"av01", vec![heif_size(1920, 1080)]);
  item.data = vec![0; 20_000];
  settles("avif", &heif(b"avif", &[b"avif"], 1, &[item], &[]));

  let root = format!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" data-note=\"{}\" width=\"640\" height=\"480\">",
    "x".repeat(700)
  );
  settles("svg", format!("{root}<g/></svg>").as_bytes());
}

#[test]
fn documents_settle() {
  settles(
    "text",
    "A line of words, and then another line of words.\n"
      .repeat(200)
      .as_bytes(),
  );
  settles(
    "json",
    format!("{{\"items\": [{}1]}}", "1, ".repeat(3000)).as_bytes(),
  );
  let rtf = [
    &br"{\rtf1\ansi\deflang1033{\info{\title A Harbor at Dusk}}\paperw12240\paperh15840\pard "[..],
    "word ".repeat(2000).as_bytes(),
    br"\par}",
  ]
  .concat();
  settles("rtf", &rtf);
  settles(
    "pdf",
    &pdf(
      &["Hello from the first page", "and from the second"],
      &[("Creator", "made by hand")],
    ),
  );
  settles(
    "docx",
    &office_package(
      "word/document.xml",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    ),
  );
}

#[test]
fn containers_settle() {
  let big = vec![7u8; 90_000];
  settles(
    "zip",
    &zip(
      &[("media/film.bin", &big), ("notes.txt", b"hello")],
      Sizes::AfterData,
      b"",
    ),
  );
  settles(
    "compound file",
    &compound(
      &[("Workbook", &[]), ("\u{5}SummaryInformation", &[])],
      [0; 16],
    ),
  );
  let mut archive = b"notes.txt".to_vec();
  archive.resize(2048, 0);
  archive[257..265].copy_from_slice(b"ustar\x0000");
  settles("tar", &archive);
  settles(
    "matroska",
    &[ebml(Some(b"matroska"), LengthIn::EightBytes), vec![0; 5000]].concat(),
  );
}

/// Media first and the movie's own description after it: the box that settles what the file is
/// lies past the head. With the `video` feature a parser is asked for more than a name, and a
/// description built by hand is not one it reads.
#[cfg(not(feature = "video"))]
#[test]
fn a_film_whose_media_comes_first_settles() {
  settles(
    "quicktime",
    &[boxed(b"mdat", &vec![0; 20_000]), boxed(b"moov", &[0; 64])].concat(),
  );
}

#[test]
fn what_follows_a_png_s_picture_is_read_when_it_is_there_and_never_asked_for() {
  let file = png_of(&[
    png_header(8, 8, 8, 2),
    picture(),
    png_chunk(b"tEXt", b"Comment\0written last"),
    png_chunk(b"IEND", &[]),
  ]);
  let whole = specs(&file);
  let (found, steps, _) = exchange("a png", &file, 60, true);
  // The head reaches the picture, so nothing is fetched; the text after it is the one thing
  // the whole file says that the exchange does not.
  assert_eq!(steps, 0);
  assert_ne!(found.facts, whole.facts);
}

#[test]
fn the_rarer_images_settle() {
  // JPEG XL: bare, in a file of boxes with what a camera wrote down between the pieces of the
  // codestream, and with it after the codestream.
  let codestream = [
    &[0xFF, 0x0A, 0x4F, 0x02][..],
    &[0x00, 0x91, 0x07, 0xE3].repeat(2000),
  ]
  .concat();
  settles("jxl codestream", &codestream);
  let opening = [
    b"\0\0\0\x0CJXL \r\n\x87\n".to_vec(),
    ftyp(b"jxl ", &[b"jxl "]),
  ]
  .concat();
  let notes = boxed(b"Exif", &[&[0; 4][..], &exif_block(true)].concat());
  let packet = boxed(b"xml ", XMP_PACKET.as_bytes());
  let first = boxed(
    b"jxlp",
    &[&0u32.to_be_bytes()[..], &codestream[..8]].concat(),
  );
  let rest = boxed(
    b"jxlp",
    &[&0x8000_0001u32.to_be_bytes()[..], &codestream[8..]].concat(),
  );
  settles(
    "jxl in pieces",
    &[opening.clone(), first, packet.clone(), notes.clone(), rest].concat(),
  );
  settles(
    "jxl with notes last",
    &[opening, boxed(b"jxlc", &codestream), notes, packet].concat(),
  );

  // JPEG 2000: a file whose XMP packet stands between its header and its picture, and a
  // codestream alone.
  let size = [
    &0u16.to_be_bytes()[..],
    &640u32.to_be_bytes(),
    &480u32.to_be_bytes(),
    &[0; 8],
    &640u32.to_be_bytes(),
    &480u32.to_be_bytes(),
    &[0; 8],
  ]
  .concat();
  let picture = [
    &[0xFF, 0x4F, 0xFF, 0x51, 0, 47][..],
    &size,
    &[0, 3, 7, 1, 1, 7, 1, 1, 7, 1, 1],
    &[0; 9000],
  ]
  .concat();
  settles("j2c", &picture);
  let described = [
    boxed(
      b"ihdr",
      &[
        &480u32.to_be_bytes()[..],
        &640u32.to_be_bytes(),
        &[0, 3, 7, 7, 0, 0],
      ]
      .concat(),
    ),
    boxed(b"colr", &[1, 0, 0, 0, 0, 0, 16]),
  ]
  .concat();
  let marked = [
    0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC,
  ];
  let jp2 = [
    b"\0\0\0\x0CjP  \r\n\x87\n".to_vec(),
    ftyp(b"jp2 ", &[b"jp2 "]),
    boxed(b"jp2h", &described),
    boxed(b"uuid", &[&marked[..], XMP_PACKET.as_bytes()].concat()),
    boxed(b"jp2c", &picture),
  ]
  .concat();
  settles("jp2", &jp2);

  // JPEG XR, with its directory far into the file.
  let mut jxr = tiff_at(
    true,
    50_000,
    &[vec![
      (0xBC80, TiffValue::Long(vec![640])),
      (0xBC81, TiffValue::Long(vec![480])),
    ]],
  );
  jxr[2..4].copy_from_slice(&[0xBC, 0x01]);
  settles("jxr", &jxr);

  // Radiance, with a header longer than a short head.
  let header = format!(
    "#?RADIANCE\n# {}\nFORMAT=32-bit_rle_rgbe\n\n-Y 480 +X 640\n",
    "x".repeat(700)
  );
  settles(
    "hdr",
    &[header.as_bytes(), &[2, 2, 2, 128].repeat(400)].concat(),
  );

  // JPEG-LS, whose frame header a camera's notes stand in front of.
  settles(
    "jls",
    &jpeg(&[(0xE1, vec![0; 9000]), (0xF7, jpeg_frame(640, 480, 3))]),
  );

  // What a camera records: a small copy first, and the sensor's recording in a directory the
  // first one points at.
  let copy = |reduced: u32, width: u32, photometric: u16| {
    vec![
      (254, TiffValue::Long(vec![reduced])),
      (256, TiffValue::Long(vec![width])),
      (257, TiffValue::Long(vec![width * 2 / 3])),
      (262, TiffValue::Short(vec![photometric])),
    ]
  };
  let mut first = copy(1, 160, 2);
  first.push((271, TiffValue::Text("NIKON CORPORATION")));
  first.push((
    330,
    TiffValue::Directories(vec![copy(1, 1620, 6), copy(0, 6032, 32803)]),
  ));
  settles("nef", &tiff(true, &[first]));
}

/// A JPEG as a phone writes one: what the camera noted, a second picture's index, a color
/// profile and a maker's own segment, each in a segment of its own in front of the picture.
fn camera_jpeg() -> Vec<u8> {
  jpeg(&[
    (0xE0, b"JFIF\0\x01\x02\0\0\x01\0\x01\0\0".to_vec()),
    (
      0xE1,
      [&b"Exif\0\0"[..], &exif_block(true), &[0; 11_000]].concat(),
    ),
    (0xE2, [&b"MPF\0"[..], &[0; 82]].concat()),
    (0xE2, [&b"ICC_PROFILE\0\x01\x01"[..], &[0; 540]].concat()),
    (0xEA, vec![0; 564]),
    (0xDB, vec![0; 130]),
    (0xDD, vec![0; 2]),
    (0xC0, jpeg_frame(4032, 3024, 3)),
  ])
}

#[test]
fn a_jpeg_with_a_camera_s_segments_in_front_of_its_picture_settles_in_a_few_fetches() {
  let file = camera_jpeg();
  settles("a camera's jpeg", &file);
  // From the head a fetcher usually has, it takes the camera's notes and then one block.
  let (_, fetches, _) = exchange("a camera's jpeg", &file, 4096, true);
  assert!(fetches <= 3, "{fetches} fetches");
}

/// Sound files `audiodown-node` keeps among its fixtures.
#[cfg(feature = "audio")]
#[test]
fn a_wave_file_settles_without_its_sound_being_fetched() {
  for name in [
    "stereo.wav",
    "pcm24.wav",
    "float.wav",
    "pcm_alaw.wav",
    "rf64.wav",
  ] {
    let path = format!(
      "{}/../audiodown-node/__test__/fixtures/{name}",
      env!("CARGO_MANIFEST_DIR")
    );
    let file = std::fs::read(&path).unwrap();
    settles(name, &file);
    // A file too long for the lengths of the usual header keeps them elsewhere, and is still
    // asked for whole.
    if name != "rf64.wav" && file.len() > 16_384 {
      let (_, _, fetched) = exchange(name, &file, 4096, true);
      assert!(fetched <= 8192, "{name}: {fetched} bytes of {}", file.len());
    }
  }
}
