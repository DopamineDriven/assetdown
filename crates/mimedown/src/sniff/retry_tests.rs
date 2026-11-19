use super::*;
use crate::source::Piece;
use crate::testing::{
  LengthIn, Sizes, compound, ebml, office_package, pdf, png_header, png_of, zip,
};
use crate::{DocumentDetail, Facts, specs_with};

const RETRY: Options<'static> = Options {
  file_name: None,
  retry_on_unknown: true,
};

fn first_pass(bytes: &[u8]) -> Sniffed {
  sniff(bytes)
}

fn second_look(bytes: &[u8]) -> Sniffed {
  sniff_with(&Source::whole(bytes), RETRY)
}

/// Bytes that are nothing, to stand in front of a file: zeros, so that they are not text.
fn behind(leading: usize, file: &[u8]) -> Vec<u8> {
  [vec![0; leading], file.to_vec()].concat()
}

fn retried(found: Sniffed) -> (MediaType, Option<Retry>) {
  match found {
    Sniffed::Named {
      media_type, retry, ..
    } => (media_type, retry),
    Sniffed::Unknown { .. } => (MediaType::OctetStream, None),
  }
}

#[test]
fn a_second_look_is_taken_only_when_asked_for_and_only_when_the_first_named_nothing() {
  let picture = png_of(&[png_header(8, 8, 8, 6)]);
  let hidden = behind(40, &picture);
  assert_eq!(first_pass(&hidden), Sniffed::Unknown { needs: None });
  assert_eq!(retried(second_look(&hidden)).0, MediaType::Png);
  // What the first pass names is what it is, with or without the option.
  assert_eq!(second_look(&picture), first_pass(&picture));
  assert_eq!(retried(second_look(&picture)).1, None);
}

#[test]
fn a_file_behind_leading_bytes_is_found_and_says_how_many_there_were() {
  let picture = png_of(&[png_header(640, 480, 8, 6)]);
  assert_eq!(
    retried(second_look(&behind(100, &picture))),
    (MediaType::Png, Some(Retry::LeadingBytes { skipped: 100 }))
  );
  assert_eq!(
    retried(second_look(&behind(3, &pdf(&["a page"], &[])))),
    (MediaType::Pdf, Some(Retry::LeadingBytes { skipped: 3 }))
  );
  // Its facts are read from where it starts.
  let found = specs_with(&Source::whole(&behind(100, &picture)), RETRY);
  assert_eq!(found.retry, Some(Retry::LeadingBytes { skipped: 100 }));
  let Facts::Image(facts) = &found.facts else {
    panic!("no image facts: {found:?}");
  };
  assert_eq!((facts.width, facts.height), (640, 480));
  assert_eq!(
    Retry::LeadingBytes { skipped: 100 }.as_str(),
    "leadingBytes"
  );
}

#[test]
fn leading_bytes_are_looked_past_only_so_far() {
  let picture = png_of(&[png_header(8, 8, 8, 6)]);
  assert_eq!(
    second_look(&behind(5000, &picture)),
    Sniffed::Unknown { needs: None }
  );
}

#[test]
fn a_zip_behind_a_program_is_found_by_the_directory_at_its_end() {
  // A self-extracting archive: a program, and the zip it unpacks after it.
  let package = office_package(
    "word/document.xml",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
  );
  let archive = behind(5000, &package);
  assert_eq!(first_pass(&archive), Sniffed::Unknown { needs: None });
  assert_eq!(
    retried(second_look(&archive)),
    (MediaType::Docx, Some(Retry::EndOfFile))
  );
  let plain = behind(5000, &zip(&[("notes.txt", b"hello")], Sizes::InHeader, b""));
  assert_eq!(
    retried(second_look(&plain)),
    (MediaType::Zip, Some(Retry::EndOfFile))
  );
  // The package's own facts are read from where the package starts.
  let found = specs_with(&Source::whole(&archive), RETRY);
  let Facts::Document(facts) = &found.facts else {
    panic!("no document facts: {found:?}");
  };
  assert_eq!(facts.title.as_deref(), Some("A Harbor at Dusk"));
}

#[test]
fn a_pdf_and_an_mp3_are_known_by_how_they_end() {
  let document = behind(3000, &pdf(&["a page"], &[]));
  assert_eq!(
    retried(second_look(&document)),
    (MediaType::Pdf, Some(Retry::EndOfFile))
  );
  // Sound with nothing to name it at its start, and a tag after its last frame.
  let mut song = vec![0u8; 4000];
  song.extend([&b"TAG"[..], &[b' '; 125]].concat());
  assert_eq!(
    retried(second_look(&song)),
    (MediaType::Mp3, Some(Retry::EndOfFile))
  );
  assert_eq!(Retry::EndOfFile.as_str(), "endOfFile");
}

#[test]
fn text_is_read_with_more_allowance() {
  // UTF-16 with no mark in front to say so.
  let unmarked: Vec<u8> = "A letter of some length, written out plainly.\n"
    .encode_utf16()
    .flat_map(u16::to_le_bytes)
    .collect();
  assert_eq!(first_pass(&unmarked), Sniffed::Unknown { needs: None });
  assert_eq!(
    retried(second_look(&unmarked)),
    (MediaType::Txt, Some(Retry::LenientText))
  );
  let found = specs_with(&Source::whole(&unmarked), RETRY);
  let Facts::Document(facts) = &found.facts else {
    panic!("no document facts: {found:?}");
  };
  assert_eq!(facts.word_count, Some(8));
  let DocumentDetail::Text(detail) = &facts.detail else {
    panic!("not a text's detail");
  };
  assert_eq!(detail.encoding, "utf-16le");

  // A long text with one character in it that text does not hold.
  let mut stray = "words and more words, line after line\n"
    .repeat(60)
    .into_bytes();
  stray[700] = 0x01;
  assert_eq!(first_pass(&stray), Sniffed::Unknown { needs: None });
  assert_eq!(
    retried(second_look(&stray)),
    (MediaType::Txt, Some(Retry::LenientText))
  );
  assert_eq!(Retry::LenientText.as_str(), "lenientText");
}

#[test]
fn bytes_that_are_nothing_are_still_nothing() {
  let noise: Vec<u8> = (0..4000u32)
    .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
    .collect();
  assert_eq!(second_look(&noise), Sniffed::Unknown { needs: None });
  assert_eq!(second_look(&[0; 600]), Sniffed::Unknown { needs: None });
  assert_eq!(second_look(b""), Sniffed::Unknown { needs: None });
}

#[test]
fn from_part_of_a_file_the_second_look_asks_for_the_end() {
  let archive = behind(5000, &zip(&[("notes.txt", b"hello")], Sizes::InHeader, b""));
  let size = u64::try_from(archive.len()).unwrap();
  let pieces = [Piece {
    offset: 0,
    bytes: &archive[..4096],
  }];
  let head = sniff_with(&Source::from_pieces(&pieces, Some(size)).unwrap(), RETRY);
  let needs = head.needs().unwrap();
  assert_eq!(needs.offset + needs.length, size);
  // Given that, the directory names it.
  let tail = usize::try_from(needs.offset).unwrap();
  let pieces = [
    Piece {
      offset: 0,
      bytes: &archive[..4096],
    },
    Piece {
      offset: needs.offset,
      bytes: &archive[tail..],
    },
  ];
  let both = sniff_with(&Source::from_pieces(&pieces, Some(size)).unwrap(), RETRY);
  assert_eq!(retried(both), (MediaType::Zip, Some(Retry::EndOfFile)));
  // With no size to go by, the end is asked for counted from the end.
  let pieces = [Piece {
    offset: 0,
    bytes: &archive[..4096],
  }];
  let head = sniff_with(&Source::from_pieces(&pieces, None).unwrap(), RETRY);
  assert!(head.needs().is_some_and(|range| range.from_end));
}

#[test]
fn a_few_bytes_that_happen_to_spell_a_signature_are_not_a_file_behind_leading_bytes() {
  // Each of these opens a file of some type, and each is short enough to turn up by chance
  // in the middle of anything.
  let short: [&[u8]; 6] = [
    b"\0\0\x01\xB3",
    b"\0\0\x01\xBA",
    b"\xFF\xD8\xFF",
    b"\x1F\x8B\x08",
    b"II*\0",
    b"\0asm",
  ];
  for signature in short {
    let file = [vec![0; 100], signature.to_vec(), vec![0; 400]].concat();
    assert_eq!(
      second_look(&file),
      Sniffed::Unknown { needs: None },
      "{signature:?}"
    );
  }
  // The four bytes an EBML file opens with, and no header behind them.
  let file = [vec![0; 50], vec![0x1A, 0x45, 0xDF, 0xA3], vec![0; 400]].concat();
  assert_eq!(second_look(&file), Sniffed::Unknown { needs: None });
}

/// What a compiled program looks like from here: bytes that are not text, with one constant
/// among them.
fn program_holding(constant: &[u8]) -> Vec<u8> {
  [
    &[0xE3; 300][..],
    constant,
    &[0x5A, 0x29, 0x01, 0xDA, 0x04].repeat(200),
  ]
  .concat()
}

#[test]
fn a_signature_held_as_a_constant_is_not_a_file_behind_leading_bytes() {
  // Every program that reads a format holds its signature, and is not a file of that format.
  let constants: [&[u8]; 13] = [
    b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
    b"\x89PNG\r\n\x1A\n",
    b"\x89PNG\r\n\x1A\n\0\0\0\x0DIHDR",
    b"GIF89a",
    b"GIF87a",
    b"%PDF-1.7",
    b"{\\rtf1",
    b"7z\xBC\xAF\x27\x1C",
    b"Rar!\x1A\x07\0",
    b"OggS\0",
    b"ID3\x03\0\0\0\0\0\0",
    b"PK\x03\x04",
    b"RIFF\0\0\0\0WAVE",
  ];
  for constant in constants {
    assert_eq!(
      second_look(&program_holding(constant)),
      Sniffed::Unknown { needs: None },
      "{constant:?}"
    );
  }
}

#[test]
fn a_png_behind_leading_bytes_is_one_whose_header_s_checksum_holds() {
  let mut picture = png_of(&[png_header(640, 480, 8, 6)]);
  assert_eq!(
    retried(second_look(&behind(100, &picture))).0,
    MediaType::Png
  );
  // The same bytes with a width its checksum was not made for.
  picture[19] ^= 0x01;
  assert_eq!(
    second_look(&behind(100, &picture)),
    Sniffed::Unknown { needs: None }
  );
}

#[test]
fn containers_that_parse_are_found_behind_leading_bytes() {
  let sheet = behind(64, &compound(&[("Workbook", &[])], [0; 16]));
  assert_eq!(
    retried(second_look(&sheet)),
    (MediaType::Xls, Some(Retry::LeadingBytes { skipped: 64 }))
  );
  let film = behind(
    7,
    &[ebml(Some(b"matroska"), LengthIn::EightBytes), vec![0; 64]].concat(),
  );
  assert_eq!(
    retried(second_look(&film)),
    (MediaType::Mkv, Some(Retry::LeadingBytes { skipped: 7 }))
  );
}

#[test]
fn the_two_words_a_pdf_ends_with_are_not_a_pdf_s_ending_in_any_order() {
  let constants = [&[0u8; 3000][..], b"%%EOF", &[0; 12], b"startxref", &[0; 30]].concat();
  assert_eq!(second_look(&constants), Sniffed::Unknown { needs: None });
  let no_offset = [&[0u8; 3000][..], b"startxref\n%%EOF\n"].concat();
  assert_eq!(second_look(&no_offset), Sniffed::Unknown { needs: None });
}

/// A zip behind a launcher, with its directory counting from the start of the file: how an
/// archive written straight after a script's first line comes out.
fn counted_from_the_start(launcher: &[u8], archive: &[u8]) -> Vec<u8> {
  let leading = u32::try_from(launcher.len()).unwrap();
  let mut file = [launcher, archive].concat();
  let read16 = |file: &[u8], at: usize| usize::from(u16::from_le_bytes([file[at], file[at + 1]]));
  let moved = |file: &mut [u8], at: usize| {
    let was = u32::from_le_bytes(file[at..at + 4].try_into().unwrap());
    file[at..at + 4].copy_from_slice(&(was + leading).to_le_bytes());
  };
  // The closing record is the last 22 bytes; each of the directory's records says where its
  // entry starts.
  let closing = file.len() - 22;
  moved(&mut file, closing + 16);
  let mut record = usize::try_from(u32::from_le_bytes(
    file[closing + 16..closing + 20].try_into().unwrap(),
  ))
  .unwrap();
  for _ in 0..read16(&file, closing + 10) {
    moved(&mut file, record + 42);
    record +=
      46 + read16(&file, record + 28) + read16(&file, record + 30) + read16(&file, record + 32);
  }
  file
}

#[test]
fn a_zip_whose_directory_counts_from_the_start_of_the_file_is_found() {
  let launcher = b"#!/usr/bin/env python3\n";
  let plain = counted_from_the_start(
    launcher,
    &zip(&[("__main__.py", b"print(1)")], Sizes::InHeader, b""),
  );
  assert_eq!(first_pass(&plain), Sniffed::Unknown { needs: None });
  assert_eq!(
    retried(second_look(&plain)),
    (MediaType::Zip, Some(Retry::EndOfFile))
  );
  let package = office_package(
    "word/document.xml",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
  );
  let document = counted_from_the_start(&[0; 5000], &package);
  assert_eq!(
    retried(second_look(&document)),
    (MediaType::Docx, Some(Retry::EndOfFile))
  );
}

#[cfg(feature = "pdf")]
#[test]
fn a_pdf_behind_leading_bytes_is_read_from_where_it_starts() {
  let found = specs_with(
    &Source::whole(&behind(3, &pdf(&["a page", "another"], &[]))),
    RETRY,
  );
  assert_eq!(found.retry, Some(Retry::LeadingBytes { skipped: 3 }));
  let Facts::Document(facts) = &found.facts else {
    panic!("no document facts: {found:?}");
  };
  assert_eq!(facts.page_count, Some(2));
}

/// A PDF of some size behind eight leading bytes: its header, a body that is not text, and
/// the words it ends with.
fn long_pdf_behind_leading_bytes() -> Vec<u8> {
  let mut file = behind(8, b"%PDF-1.7\n");
  file.extend(std::iter::repeat_n(0x80, 200_000));
  file.extend(b"\nstartxref\n199000\n%%EOF\n");
  file
}

#[test]
fn a_pdf_s_ending_is_read_as_soon_as_it_is_given() {
  // The last kilobyte holds how a PDF ends. The stretch in which a zip's directory could
  // close is sixty times as long, and is not waited for.
  let file = long_pdf_behind_leading_bytes();
  let size = u64::try_from(file.len()).unwrap();
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..4096],
    },
    Piece {
      offset: size - 1024,
      bytes: &file[file.len() - 1024..],
    },
  ];
  let found = sniff_with(&Source::from_pieces(&pieces, Some(size)).unwrap(), RETRY);
  assert_eq!(
    retried(found),
    (MediaType::Pdf, Some(Retry::LeadingBytes { skipped: 8 }))
  );
}

#[test]
fn a_start_read_with_allowance_still_asks_for_the_end_of_a_file_of_known_size() {
  // Words with a stray control character among them, and a PDF's ending far behind them:
  // the end of the file is looked at before the start is read with allowance, so from the
  // start alone the answer names what it can and asks for the end.
  let mut file = b"A page of words that goes on for a good while. ".repeat(100);
  file[40] = 0x01;
  let head = file.len();
  file.extend(std::iter::repeat_n(0x80, 200_000));
  file.extend(b"\nstartxref\n199000\n%%EOF\n");
  let size = u64::try_from(file.len()).unwrap();
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..head],
  }];
  let found = sniff_with(&Source::from_pieces(&pieces, Some(size)).unwrap(), RETRY);
  assert_eq!(retried(found), (MediaType::Txt, Some(Retry::LenientText)));
  assert!(found.needs().is_some());
  // With no size the end cannot be placed, and nothing is asked for that could not be given.
  let found = sniff_with(&Source::from_pieces(&pieces, None).unwrap(), RETRY);
  assert_eq!(found.needs(), None);
}
