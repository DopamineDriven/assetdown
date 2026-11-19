use super::*;
use crate::source::Piece;

const UNKNOWN: Sniffed = Sniffed::Unknown { needs: None };

fn named(media_type: MediaType) -> Sniffed {
  Sniffed::Named {
    media_type,
    evidence: Evidence::Signature,
    needs: None,
    retry: None,
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

fn join(parts: &[&[u8]]) -> Vec<u8> {
  parts.concat()
}

fn padded(start: &[u8], length: usize) -> Vec<u8> {
  let mut bytes = start.to_vec();
  bytes.resize(length, 0);
  bytes
}

// ── Headers, built by hand ───────────────────────────────────────

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\x0DIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

/// A 1 by 1 bitmap with the header Windows has written since 3.1.
fn bmp() -> Vec<u8> {
  join(&[
    b"BM",
    &58u32.to_le_bytes(), // the file's size
    &[0; 4],              // reserved
    &54u32.to_le_bytes(), // where the pixels start
    &40u32.to_le_bytes(), // the size of the header that follows
    &1i32.to_le_bytes(),
    &1i32.to_le_bytes(),
    &1u16.to_le_bytes(),
    &24u16.to_le_bytes(),
    &[0; 24],
    &[0, 0, 0xFF, 0],
  ])
}

/// An icon (`kind` 1) or a cursor (`kind` 2) holding one picture.
fn ico(kind: u8) -> Vec<u8> {
  join(&[
    &[0, 0, kind, 0, 1, 0],
    &[16, 16, 0, 0, 1, 0, 32, 0],
    &40u32.to_le_bytes(), // the picture's size
    &22u32.to_le_bytes(), // where it starts
    &[0; 40],
  ])
}

/// The first 512 bytes of a tar: one entry's header.
fn tar(magic: &[u8]) -> Vec<u8> {
  let mut header = padded(b"notes.txt", 512);
  header[257..257 + magic.len()].copy_from_slice(magic);
  header
}

/// An sfnt font's start: its version, how many tables it has, and the first table's tag.
fn sfnt(version: &[u8], first_tag: &[u8]) -> Vec<u8> {
  join(&[
    version,
    &[0, 10, 0, 0x80, 0, 3, 0, 0x20],
    first_tag,
    &[0; 12],
  ])
}

/// The first page of an Ogg stream, carrying one packet.
fn ogg(packet: &[u8]) -> Vec<u8> {
  join(&[
    b"OggS",
    &[0, 2],       // version; the first page of a stream
    &[0; 8],       // granule position
    &[1, 0, 0, 0], // serial number
    &[0; 8],       // page number, checksum
    &[1, u8::try_from(packet.len()).unwrap()],
    packet,
  ])
}

/// An MPEG-1 Layer III frame at 128 kbit/s and 44.1 kHz: 417 bytes.
fn mp3_frame() -> Vec<u8> {
  padded(&[0xFF, 0xFB, 0x90, 0x00], 417)
}

/// An MPEG-2 Layer III frame at 64 kbit/s and 22.05 kHz: 208 bytes.
fn mpeg2_frame() -> Vec<u8> {
  padded(&[0xFF, 0xF3, 0x80, 0x00], 208)
}

/// An MPEG-1 Layer I frame at 128 kbit/s and 32 kHz: 192 bytes.
fn layer1_frame() -> Vec<u8> {
  padded(&[0xFF, 0xFF, 0x48, 0x00], 192)
}

/// An AAC frame in an ADTS header that states its length as 64 bytes.
fn adts_frame() -> Vec<u8> {
  padded(&[0xFF, 0xF1, 0x50, 0x80, 0x08, 0x1F, 0xFC], 64)
}

/// An ID3v2.4 tag whose body is `body` bytes long.
fn id3(body: u8) -> Vec<u8> {
  padded(
    &[b'I', b'D', b'3', 4, 0, 0, 0, 0, 0, body],
    10 + usize::from(body),
  )
}

const FLAC: &[u8] = b"fLaC\0\0\0\x22";

/// The start of an embedded OpenType font: its two lengths, its version, and `LP` 34 bytes in.
fn eot(version: u32) -> Vec<u8> {
  let mut header = join(&[
    &900u32.to_le_bytes(), // the file's length
    &700u32.to_le_bytes(), // the font's
    &version.to_le_bytes(),
  ]);
  header.resize(34, 0);
  header.extend_from_slice(b"LP");
  header.resize(900, 0);
  header
}

/// A binary STL of `triangles` triangles: 80 bytes of anything, the count, 50 bytes for each.
fn stl(triangles: u32) -> Vec<u8> {
  let mut file = padded(b"made by hand", 80);
  file.extend_from_slice(&triangles.to_le_bytes());
  file.resize(84 + 50 * usize::try_from(triangles).unwrap(), 0);
  file
}

/// The first line of an IGES file: 72 columns of text, the section's letter, the line's number.
fn iges(number: &str) -> Vec<u8> {
  format!(
    "{:<72}S{number}\n{:<72}G      1\n",
    "A part drawn by hand", "1H,,1H;"
  )
  .into_bytes()
}

// ── What is named ────────────────────────────────────────────────

#[test]
fn every_fixed_signature_names_its_type() {
  let two_mp3_frames = join(&[&mp3_frame(), &mp3_frame()]);
  let two_mpeg2_frames = join(&[&mpeg2_frame(), &mpeg2_frame()]);
  let two_layer1_frames = join(&[&layer1_frame(), &layer1_frame()]);
  let two_adts_frames = join(&[&adts_frame(), &adts_frame()]);
  let cases: Vec<(&str, Vec<u8>, MediaType)> = vec![
    ("png", PNG.to_vec(), MediaType::Png),
    (
      "jpeg",
      b"\xFF\xD8\xFF\xE0\0\x10JFIF\0".to_vec(),
      MediaType::Jpeg,
    ),
    (
      "gif 87a",
      b"GIF87a\x01\0\x01\0\0\0\0".to_vec(),
      MediaType::Gif,
    ),
    (
      "gif 89a",
      b"GIF89a\x01\0\x01\0\0\0\0".to_vec(),
      MediaType::Gif,
    ),
    ("bmp", bmp(), MediaType::Bmp),
    ("ico", ico(1), MediaType::Ico),
    ("cur", ico(2), MediaType::Ico),
    (
      "tiff, little-endian",
      b"II*\0\x08\0\0\0".to_vec(),
      MediaType::Tiff,
    ),
    (
      "tiff, big-endian",
      b"MM\0*\0\0\0\x08".to_vec(),
      MediaType::Tiff,
    ),
    (
      "bigtiff, little-endian",
      b"II+\0\x08\0\0\0\x10\0\0\0\0\0\0\0".to_vec(),
      MediaType::Tiff,
    ),
    (
      "bigtiff, big-endian",
      b"MM\0+\0\x08\0\0\0\0\0\0\0\0\0\x10".to_vec(),
      MediaType::Tiff,
    ),
    (
      "radiance",
      b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 1\n".to_vec(),
      MediaType::Hdr,
    ),
    (
      "radiance, written as rgbe",
      b"#?RGBE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 1\n".to_vec(),
      MediaType::Hdr,
    ),
    (
      "ktx",
      b"\xABKTX 11\xBB\r\n\x1A\n\x01\x02\x03\x04".to_vec(),
      MediaType::Ktx,
    ),
    (
      "ktx2",
      b"\xABKTX 20\xBB\r\n\x1A\n\0\0\0\0".to_vec(),
      MediaType::Ktx2,
    ),
    (
      "dpx, big-endian",
      b"SDPX\0\0\x08\0V2.0\0\0\0\0".to_vec(),
      MediaType::Dpx,
    ),
    (
      "dpx, little-endian",
      b"XPDS\0\x08\0\0V1.0\0\0\0\0".to_vec(),
      MediaType::Dpx,
    ),
    (
      "emf",
      join(&[
        &[1, 0, 0, 0, 108, 0, 0, 0],
        &[0; 32],
        b" EMF",
        &[0, 0, 1, 0],
      ]),
      MediaType::Emf,
    ),
    (
      "pdf",
      b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec(),
      MediaType::Pdf,
    ),
    (
      "rtf",
      b"{\\rtf1\\ansi\\deff0 {\\fonttbl}}".to_vec(),
      MediaType::Rtf,
    ),
    (
      "gzip",
      b"\x1F\x8B\x08\0\0\0\0\0\0\x03".to_vec(),
      MediaType::Gzip,
    ),
    (
      "7z",
      b"7z\xBC\xAF\x27\x1C\0\x04".to_vec(),
      MediaType::SevenZip,
    ),
    (
      "rar, to version 4",
      b"Rar!\x1A\x07\0\xCF\x90s".to_vec(),
      MediaType::Rar,
    ),
    (
      "rar, version 5",
      b"Rar!\x1A\x07\x01\0\x33\x92".to_vec(),
      MediaType::Rar,
    ),
    ("tar, posix", tar(b"ustar\x0000"), MediaType::Tar),
    ("tar, gnu", tar(b"ustar  \0"), MediaType::Tar),
    ("bzip2", b"BZh91AY&SY\0\0\0\0".to_vec(), MediaType::Bzip2),
    (
      "bzip2, of nothing",
      b"BZh9\x17\x72\x45\x38\x50\x90\0\0\0\0".to_vec(),
      MediaType::Bzip2,
    ),
    ("flac", padded(FLAC, 42), MediaType::Flac),
    (
      "flac, whose first block is its last",
      padded(b"fLaC\x80\0\0\x22", 42),
      MediaType::Flac,
    ),
    (
      "midi",
      b"MThd\0\0\0\x06\0\x01\0\x01\0\x60".to_vec(),
      MediaType::Midi,
    ),
    ("mp3", two_mp3_frames, MediaType::Mp3),
    ("mp3, mpeg-2", two_mpeg2_frames, MediaType::Mp3),
    ("mpeg audio, layer I", two_layer1_frames, MediaType::Mp3),
    ("aac", two_adts_frames, MediaType::Aac),
    (
      "mpeg, a program stream",
      b"\0\0\x01\xBA\x44\0\x04\0".to_vec(),
      MediaType::Mpeg,
    ),
    (
      "mpeg, video alone",
      b"\0\0\x01\xB3\x16\0\xF0\xC4".to_vec(),
      MediaType::Mpeg,
    ),
    ("wasm", b"\0asm\x01\0\0\0".to_vec(), MediaType::Wasm),
    (
      "woff",
      b"wOFF\0\x01\0\0\0\0\x10\0".to_vec(),
      MediaType::Woff,
    ),
    (
      "woff of an opentype font",
      b"wOFFOTTO\0\0\x10\0".to_vec(),
      MediaType::Woff,
    ),
    (
      "woff2",
      b"wOF2\0\x01\0\0\0\0\x10\0".to_vec(),
      MediaType::Woff2,
    ),
    ("otf", sfnt(b"OTTO", b"CFF "), MediaType::Otf),
    ("ttf", sfnt(&[0, 1, 0, 0], b"DSIG"), MediaType::Ttf),
    ("ttf, apple's", sfnt(b"true", b"OS/2"), MediaType::Ttf),
    ("ttc", b"ttcf\0\x02\0\0\0\0\0\x02".to_vec(), MediaType::Ttc),
    ("glb", b"glTF\x02\0\0\0\x40\0\0\0".to_vec(), MediaType::Glb),
    ("eot", eot(0x0002_0001), MediaType::Eot),
    ("eot, version 2.2", eot(0x0002_0002), MediaType::Eot),
    ("eot, version 1", eot(0x0001_0000), MediaType::Eot),
    (
      "usd, binary",
      b"PXR-USDC\0\x08\0\0\0\0\0\0".to_vec(),
      MediaType::Usd,
    ),
    (
      "usd, as text",
      b"#usda 1.0\n(\n    defaultPrim = \"Chair\"\n)\n".to_vec(),
      MediaType::Usda,
    ),
    ("iges", iges("      1"), MediaType::Iges),
    (
      "iges, numbered with zeros",
      iges("0000001"),
      MediaType::Iges,
    ),
    (
      "vrml 1",
      b"#VRML V1.0 ascii\nSeparator {}\n".to_vec(),
      MediaType::Vrml,
    ),
    (
      "vrml 2",
      b"#VRML V2.0 utf8\nShape {}\n".to_vec(),
      MediaType::Vrml,
    ),
    (
      "step",
      b"ISO-10303-21;\nHEADER;\nENDSEC;\n".to_vec(),
      MediaType::Step,
    ),
    ("stl, binary", stl(2), MediaType::Stl),
  ];
  for (label, bytes, media_type) in cases {
    assert_eq!(sniff(&bytes), named(media_type), "{label}");
  }
}

#[test]
fn an_ogg_is_named_for_what_its_first_page_carries() {
  let cases: [(&[u8], MediaType); 9] = [
    (b"\x01vorbis\0\0\0\0\x02", MediaType::OggAudio),
    (b"OpusHead\x01\x02\0\0", MediaType::OggAudio),
    (b"\x7FFLAC\x01\0\0\x01fLaC", MediaType::OggAudio),
    (b"Speex   1.2.1\0\0\0", MediaType::OggAudio),
    (b"\x80theora\x03\x02\x01\0", MediaType::OggVideo),
    (b"OVP80\x01\x01\0\0\0\0\0", MediaType::OggVideo),
    (b"BBCD\0\0\0\0\x01\0\0\0", MediaType::OggVideo),
    (b"\x01video\0\0\0XVID", MediaType::OggVideo),
    // A stream of notes about the other streams: it says nothing of what they are.
    (b"fishead\0\x04\0\0\0", MediaType::Ogg),
  ];
  for (packet, media_type) in cases {
    assert_eq!(sniff(&ogg(packet)), named(media_type), "{packet:?}");
  }
  // A page that ends before its packet is still an Ogg.
  assert_eq!(sniff(&ogg(b"")[..27]), named(MediaType::Ogg));
}

#[test]
fn an_id3_tag_is_an_mp3_unless_what_follows_it_says_otherwise() {
  assert_eq!(sniff(&id3(20)), named(MediaType::Mp3));
  let then_mp3 = join(&[&id3(20), &mp3_frame()]);
  assert_eq!(sniff(&then_mp3), named(MediaType::Mp3));
  let then_flac = join(&[&id3(20), &padded(FLAC, 42)]);
  assert_eq!(sniff(&then_flac), named(MediaType::Flac));
  let then_aac = join(&[&id3(20), &adts_frame()]);
  assert_eq!(sniff(&then_aac), named(MediaType::Aac));
}

// ── What is not ──────────────────────────────────────────────────

#[test]
fn bytes_that_name_nothing_are_an_answer() {
  let cases: [&[u8]; 4] = [
    b"",
    b"a zero\0in the middle of words",
    &[0; 64],
    &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04],
  ];
  for bytes in cases {
    assert_eq!(sniff(bytes), UNKNOWN, "{bytes:?}");
  }
  assert_eq!(UNKNOWN.media_type(), MediaType::OctetStream);
  assert_eq!(UNKNOWN.needs(), None);
}

/// Words: text of no kind the bytes can name.
fn words() -> Sniffed {
  Sniffed::Named {
    media_type: MediaType::Txt,
    evidence: Evidence::Content,
    needs: None,
    retry: None,
  }
}

#[test]
fn a_signature_cut_short_names_nothing() {
  assert_eq!(sniff(&PNG[..4]), UNKNOWN);
  assert_eq!(sniff(&bmp()[..12]), UNKNOWN);
  assert_eq!(sniff(&tar(b"ustar\x0000")[..259]), UNKNOWN);
}

#[test]
fn letters_that_could_begin_a_text_are_not_believed_alone() {
  let cases: [&[u8]; 10] = [
    b"BMW builds its cars in Munich and in a few other places.",
    b"true\n",
    b"true or false: the header behind the letters has to hold together",
    b"OTTO VON BISMARCK was born in 1815 at Schoenhausen.",
    b"ID3 tags are kept at the start of the file.",
    b"wOFF is how the format spells its own name.",
    b"fLaC is how the format spells its own name.",
    b"MThd is the first chunk of the file.",
    b"SDPX is what the first four bytes read.",
    b"glTF is the name of the format.",
  ];
  for bytes in cases {
    assert_eq!(
      sniff(bytes),
      words(),
      "{:?}",
      String::from_utf8_lossy(bytes)
    );
  }
  // Four bytes that read as an icon's, with no picture behind them.
  assert_eq!(sniff(&[0, 0, 1, 0, 0, 0, 0, 0]), UNKNOWN);
  assert_eq!(sniff(&padded(&[0, 0, 1, 0, 1, 0], 22)), UNKNOWN);
  // A bitmap's two letters and no header.
  assert_eq!(sniff(&padded(b"BM", 54)), UNKNOWN);
  // The version of a TrueType font, with no table behind it.
  assert_eq!(sniff(&padded(&[0, 1, 0, 0], 32)), UNKNOWN);
}

#[test]
fn a_text_in_utf16_is_not_an_mp3() {
  // Its byte-order mark and first letter read as the header of a Layer I frame of 192 bytes.
  let mut text = vec![0xFF, 0xFE];
  for unit in "Hello, this is a letter and not a song. "
    .repeat(8)
    .encode_utf16()
  {
    text.extend_from_slice(&unit.to_le_bytes());
  }
  assert!(text.len() > 400);
  assert_eq!(sniff(&text), words());
}

#[test]
fn one_frame_header_is_not_enough_to_call_a_file_audio() {
  assert_eq!(sniff(&mp3_frame()), UNKNOWN);
  assert_eq!(sniff(&adts_frame()), UNKNOWN);
  // A header, and where the next frame should start, something else.
  let then_noise = join(&[&mp3_frame(), b"and then words"]);
  assert_eq!(sniff(&then_noise), UNKNOWN);
  // Two frames that do not agree on the sample rate are not one stream.
  let mixed = join(&[&mp3_frame(), &mpeg2_frame()]);
  assert_eq!(sniff(&mixed), UNKNOWN);
}

#[test]
fn a_binary_stl_is_known_by_its_length_and_by_nothing_else() {
  assert_eq!(sniff(&stl(12)), named(MediaType::Stl));
  // One byte more than its count of triangles accounts for.
  let mut longer = stl(12);
  longer.push(0);
  assert_eq!(sniff(&longer), UNKNOWN);
  // A count of none: 84 bytes that could be anything.
  assert_eq!(sniff(&stl(0)), UNKNOWN);
  // Without the file's length there is nothing to go by.
  let file = stl(12);
  let pieces = [Piece {
    offset: 0,
    bytes: &file,
  }];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert!(matches!(sniff_source(&source), Sniffed::Unknown { .. }));
  let source = Source::from_pieces(&pieces, Some(684)).unwrap();
  assert_eq!(sniff_source(&source), named(MediaType::Stl));
}

#[test]
fn an_embedded_font_has_to_hold_together() {
  // A version there never was.
  assert_eq!(sniff(&eot(0x0003_0000)), UNKNOWN);
  // A font longer than the file that holds it.
  let mut inside_out = eot(0x0002_0001);
  inside_out[4..8].copy_from_slice(&5000u32.to_le_bytes());
  assert_eq!(sniff(&inside_out), UNKNOWN);
}

// ── Part of a file ───────────────────────────────────────────────

#[test]
fn a_head_that_stops_inside_a_signature_asks_for_more() {
  assert_eq!(
    sniff_head(&PNG[..4]),
    Sniffed::Unknown {
      needs: Some(ByteRange::forward(4, 4092))
    }
  );
  // When the file is known to end there, there is nothing to ask for.
  let pieces = [Piece {
    offset: 0,
    bytes: &PNG[..4],
  }];
  let source = Source::from_pieces(&pieces, Some(4)).unwrap();
  assert_eq!(sniff_source(&source), UNKNOWN);
}

#[test]
fn a_head_that_holds_the_signature_asks_for_nothing() {
  assert_eq!(sniff_head(&bmp()[..18]), named(MediaType::Bmp));
  assert_eq!(sniff_head(b"%PDF-1.7"), named(MediaType::Pdf));
}

#[test]
fn a_png_is_named_from_its_signature_and_asks_whether_it_is_animated() {
  // The signature and no chunk after it: a PNG, and the chunks that follow would say which
  // kind. One chunk is followed by the next, so a block of them is asked for.
  assert_eq!(
    sniff_head(&PNG[..8]),
    Sniffed::Named {
      media_type: MediaType::Png,
      evidence: Evidence::Signature,
      needs: Some(ByteRange::forward(8, 4096)),
      retry: None,
    }
  );
  // A whole file that ends before its picture is a PNG and asks for nothing.
  assert_eq!(sniff(PNG), named(MediaType::Png));
}

#[test]
fn a_tar_is_asked_for_when_the_head_stops_before_its_signature() {
  let file = tar(b"ustar\x0000");
  assert_eq!(
    sniff_head(&file[..100]),
    Sniffed::Unknown {
      needs: Some(ByteRange::forward(100, 3996))
    }
  );
  // No more is asked for than the file holds.
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..100],
  }];
  let source = Source::from_pieces(&pieces, Some(512)).unwrap();
  let first = sniff_source(&source);
  assert_eq!(
    first,
    Sniffed::Unknown {
      needs: Some(ByteRange::forward(100, 412))
    }
  );

  // Given what was asked for, the answer is settled.
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..100],
    },
    Piece {
      offset: 100,
      bytes: &file[100..],
    },
  ];
  let source = Source::from_pieces(&pieces, Some(512)).unwrap();
  assert_eq!(sniff_source(&source), named(MediaType::Tar));
}

#[test]
fn a_piece_from_the_middle_of_a_file_asks_for_its_head() {
  let middle = [7; 100];
  let pieces = [Piece {
    offset: 1000,
    bytes: &middle,
  }];
  let source = Source::from_pieces(&pieces, Some(50_000)).unwrap();
  assert_eq!(
    sniff_source(&source),
    Sniffed::Unknown {
      needs: Some(ByteRange::forward(0, 4096))
    }
  );
  let source = Source::from_pieces(&pieces, Some(1100)).unwrap();
  assert_eq!(
    sniff_source(&source),
    Sniffed::Unknown {
      needs: Some(ByteRange::forward(0, 1100))
    }
  );
}

#[test]
fn a_tag_longer_than_the_head_is_an_mp3_that_asks_for_what_follows_the_tag() {
  // The tag says it is 110 bytes long; only its first 40 are here.
  let tag = id3(100);
  assert_eq!(
    sniff_head(&tag[..40]),
    Sniffed::Named {
      media_type: MediaType::Mp3,
      evidence: Evidence::Signature,
      needs: Some(ByteRange::forward(110, 4)),
      retry: None,
    }
  );
  let then_flac = join(&[&tag, &padded(FLAC, 42)]);
  assert_eq!(sniff_head(&then_flac), named(MediaType::Flac));
}

#[test]
fn an_ogg_whose_first_packet_is_not_here_asks_for_it() {
  // The page's header is its first 27 bytes; one byte of segment lengths and the packet follow.
  let page = ogg(b"OpusHead\x01\x02\0\0");
  assert_eq!(
    sniff_head(&page[..27]),
    Sniffed::Named {
      media_type: MediaType::Ogg,
      evidence: Evidence::Signature,
      needs: Some(ByteRange::forward(28, 8)),
      retry: None,
    }
  );
  // Given that range and no more, the packet says which kind of Ogg it is.
  let pieces = [
    Piece {
      offset: 0,
      bytes: &page[..27],
    },
    Piece {
      offset: 28,
      bytes: &page[28..36],
    },
  ];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(sniff_source(&source), named(MediaType::OggAudio));
}

#[test]
fn evidence_is_spelled_as_the_answer_spells_it() {
  assert_eq!(Evidence::Signature.as_str(), "signature");
  assert_eq!(Evidence::FileName.as_str(), "fileName");
}

/// A PHP file with no closing tag, longer than the sample that is read for text.
fn long_php() -> Vec<u8> {
  let file = format!("<?php\n{}", "echo 'a line of it';\n".repeat(4000)).into_bytes();
  assert!(file.len() > TEXT_SAMPLE);
  file
}

#[test]
fn a_whole_text_file_asks_for_nothing_however_it_opens() {
  let conflict = format!("<<<<<<< HEAD\n{}", "ours and theirs\n".repeat(5000)).into_bytes();
  assert!(conflict.len() > TEXT_SAMPLE);
  for file in [long_php(), conflict] {
    let found = sniff(&file);
    assert!(matches!(found, Sniffed::Named { .. }));
    assert_eq!(found.needs(), None);
  }
}

#[test]
fn text_that_is_not_markup_is_not_asked_for_again() {
  let file = long_php();
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..8192],
  }];
  let size = u64::try_from(file.len()).ok();
  let found = sniff_source(&Source::from_pieces(&pieces, size).unwrap());
  assert_eq!(found.needs(), None);
}

#[test]
fn markup_whose_opening_is_cut_short_is_asked_for_as_far_as_text_is_read_and_no_further() {
  let file = format!(
    "<?xml version=\"1.0\"?>\n<!-- {} -->\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n{}",
    "a long comment ".repeat(600),
    "<!-- after it -->\n".repeat(5000)
  )
  .into_bytes();
  assert!(file.len() > TEXT_SAMPLE);
  let size = u64::try_from(file.len()).ok();
  let sample = u64::try_from(TEXT_SAMPLE).unwrap();

  let head = [Piece {
    offset: 0,
    bytes: &file[..4096],
  }];
  let found = sniff_source(&Source::from_pieces(&head, size).unwrap());
  assert_eq!(found.needs(), Some(ByteRange::forward(4096, sample - 4096)));

  let more = [Piece {
    offset: 0,
    bytes: &file[..TEXT_SAMPLE],
  }];
  let found = sniff_source(&Source::from_pieces(&more, size).unwrap());
  assert_eq!((found.media_type(), found.needs()), (MediaType::Svg, None));

  // A file that ends before the sample would: only what there is of it is asked for.
  let short = &file[..20_000];
  let head = [Piece {
    offset: 0,
    bytes: &short[..4096],
  }];
  let found = sniff_source(&Source::from_pieces(&head, Some(20_000)).unwrap());
  assert_eq!(found.needs(), Some(ByteRange::forward(4096, 20_000 - 4096)));
}
