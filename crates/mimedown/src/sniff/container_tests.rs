use super::*;
use crate::source::Piece;
use crate::testing::{
  LengthIn, Sizes, boxed, compound, content_types, ebml, ftyp, office_package, riff, zip,
};

const UNKNOWN: Sniffed = Sniffed::Unknown { needs: None };

type Brand = [u8; 4];
/// An entry of a zip: its name and what it holds.
type Entry = (&'static str, &'static [u8]);
/// What a compound file's root holds: a name, and the names under it when it is a storage.
type Child = (&'static str, &'static [&'static str]);

fn contained(media_type: MediaType) -> Sniffed {
  Sniffed::Named {
    media_type,
    evidence: Evidence::Container,
    needs: None,
    retry: None,
  }
}

fn by_signature(media_type: MediaType, needs: Option<ByteRange>) -> Sniffed {
  Sniffed::Named {
    media_type,
    evidence: Evidence::Signature,
    needs,
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

/// A file that opens with these brands and goes on with a box of media.
fn branded(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
  [ftyp(major, compatible), boxed(b"mdat", &[0; 16])].concat()
}

fn zipped(entries: &[(&str, &[u8])]) -> Vec<u8> {
  zip(entries, Sizes::InHeader, b"")
}

// ── ISO base media ───────────────────────────────────────────────

#[test]
fn an_mp4_and_a_heic_differ_only_in_their_brand() {
  let film = branded(b"mp42", &[b"mp42", b"isom"]);
  let photo = branded(b"heic", &[b"mp42", b"isom"]);
  let differing = film.iter().zip(&photo).filter(|(a, b)| a != b).count();
  assert_eq!((film.len(), differing), (photo.len(), 4));
  assert_eq!(sniff(&film), contained(MediaType::Mp4));
  assert_eq!(sniff(&photo), contained(MediaType::Heic));
}

#[test]
fn iso_base_media_is_named_by_its_brands() {
  let cases: [(&Brand, &[&Brand], MediaType); 17] = [
    (
      b"isom",
      &[b"isom", b"iso2", b"avc1", b"mp41"],
      MediaType::Mp4,
    ),
    (b"mp42", &[b"mp42", b"isom"], MediaType::Mp4),
    (b"M4V ", &[b"M4V ", b"mp42"], MediaType::Mp4),
    (b"dash", &[b"iso6", b"mp41"], MediaType::Mp4),
    (b"qt  ", &[b"qt  "], MediaType::Mov),
    (b"M4A ", &[b"M4A ", b"mp42", b"isom"], MediaType::M4a),
    (b"M4B ", &[b"M4B ", b"mp42"], MediaType::M4a),
    (b"3gp4", &[b"3gp4", b"isom"], MediaType::ThreeGp),
    (b"3g2a", &[b"3g2a"], MediaType::ThreeG2),
    (b"heic", &[b"mif1", b"heic"], MediaType::Heic),
    (b"heix", &[b"mif1", b"heix"], MediaType::Heic),
    // The general brand first, and the codec among what it is compatible with.
    (b"mif1", &[b"mif1", b"heic"], MediaType::Heic),
    (b"avif", &[b"avif", b"mif1", b"miaf"], MediaType::Avif),
    (b"mif1", &[b"mif1", b"avif"], MediaType::Avif),
    (b"avis", &[b"avis", b"msf1"], MediaType::Avif),
    // The general brand and no codec named: a picture of this family, and no more is known.
    (b"mif1", &[b"mif1"], MediaType::Heif),
    (b"avci", &[b"avci", b"mif1"], MediaType::Avci),
  ];
  for (major, compatible, media_type) in cases {
    let label = String::from_utf8_lossy(major).into_owned();
    assert_eq!(
      sniff(&branded(major, compatible)),
      contained(media_type),
      "{label}"
    );
  }
}

#[test]
fn a_film_is_not_a_picture_for_being_compatible_with_one() {
  // What a file is, its major brand says; the rest only says what else can read it.
  assert_eq!(
    sniff(&branded(b"mp42", &[b"mp42", b"mif1", b"heic"])),
    contained(MediaType::Mp4)
  );
}

#[test]
fn a_movie_without_brands_is_quicktime_and_a_fragment_is_mp4() {
  let classic = [boxed(b"wide", &[]), boxed(b"mdat", &[0; 16])].concat();
  assert_eq!(sniff(&classic), contained(MediaType::Mov));
  let movie_first = [boxed(b"moov", &[0; 24]), boxed(b"mdat", &[0; 16])].concat();
  assert_eq!(sniff(&movie_first), contained(MediaType::Mov));
  let segment = [boxed(b"styp", b"msdh\0\0\0\0msdh"), boxed(b"moof", &[0; 8])].concat();
  assert_eq!(sniff(&segment), contained(MediaType::Mp4));
  let fragment = [boxed(b"moof", &[0; 8]), boxed(b"mdat", &[0; 8])].concat();
  assert_eq!(sniff(&fragment), contained(MediaType::Mp4));
}

#[test]
fn a_first_box_whose_length_reads_as_another_signature_is_still_a_box() {
  // 442 bytes: the length's four bytes are the ones a program stream opens with.
  let movie = [boxed(b"moov", &[0; 434]), boxed(b"mdat", &[0; 16])].concat();
  assert_eq!(movie[..4], [0, 0, 1, 0xBA]);
  assert_eq!(sniff(&movie), contained(MediaType::Mov));
  // 256 bytes: the four an icon opens with.
  let padded = [boxed(b"free", &[0; 248]), boxed(b"mdat", &[0; 16])].concat();
  assert_eq!(padded[..4], [0, 0, 1, 0]);
  assert_eq!(sniff(&padded), contained(MediaType::Mov));
}

// ── RIFF and EBML ────────────────────────────────────────────────

#[test]
fn riff_is_named_by_its_form() {
  let cases: [(&Brand, &Brand, MediaType); 5] = [
    (b"RIFF", b"WEBP", MediaType::Webp),
    (b"RIFF", b"WAVE", MediaType::Wav),
    (b"RF64", b"WAVE", MediaType::Wav),
    (b"RIFF", b"AVI ", MediaType::Avi),
    (b"RIFF", b"CDDA", MediaType::Cda),
  ];
  for (container, form, media_type) in cases {
    assert_eq!(sniff(&riff(container, form)), contained(media_type));
  }
  // An animated cursor: RIFF, and a form with no type here.
  assert_eq!(sniff(&riff(b"RIFF", b"ACON")), UNKNOWN);
  // The four letters and no form behind them: words.
  assert_eq!(
    sniff(b"RIFF is how the container spells its name.").media_type(),
    MediaType::Txt
  );
}

#[test]
fn ebml_is_named_by_its_document_type() {
  let webm = ebml(Some(b"webm"), LengthIn::OneByte);
  assert_eq!(sniff(&webm), contained(MediaType::Webm));
  let matroska = ebml(Some(b"matroska"), LengthIn::EightBytes);
  assert_eq!(sniff(&matroska), contained(MediaType::Mkv));
  // A header that does not say: Matroska is the general case and WebM the particular one.
  assert_eq!(
    sniff(&ebml(None, LengthIn::OneByte)),
    contained(MediaType::Mkv)
  );
}

// ── Zip ──────────────────────────────────────────────────────────

#[test]
fn a_zip_is_named_by_what_it_lists() {
  let parts = |main: &'static str| -> Vec<Entry> {
    vec![
      ("[Content_Types].xml", b"<Types/>"),
      ("_rels/.rels", b"<Relationships/>"),
      (main, b"<part/>"),
    ]
  };
  let cases: Vec<(Vec<Entry>, MediaType)> = vec![
    (parts("word/document.xml"), MediaType::Docx),
    (parts("xl/workbook.xml"), MediaType::Xlsx),
    (parts("ppt/presentation.xml"), MediaType::Pptx),
    (parts("3D/3dmodel.model"), MediaType::ThreeMf),
    (
      vec![
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", b"<container/>"),
      ],
      MediaType::Epub,
    ),
    (
      vec![
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", b"<office:document-content/>"),
      ],
      MediaType::Odt,
    ),
    (
      vec![
        (
          "mimetype",
          b"application/vnd.oasis.opendocument.spreadsheet",
        ),
        ("content.xml", b"<office:document-content/>"),
      ],
      MediaType::Ods,
    ),
    (
      vec![
        (
          "mimetype",
          b"application/vnd.oasis.opendocument.presentation",
        ),
        ("content.xml", b"<office:document-content/>"),
      ],
      MediaType::Odp,
    ),
    (
      vec![
        ("icon.png", b"\x89PNG"),
        ("pass.json", b"{}"),
        ("manifest.json", b"{}"),
        ("signature", b"\x30\x80"),
      ],
      MediaType::Pkpass,
    ),
    (
      vec![
        ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n"),
        ("com/example/Main.class", b"\xCA\xFE\xBA\xBE"),
      ],
      MediaType::Jar,
    ),
    (
      vec![
        ("chair.usdc", b"PXR-USDC"),
        ("textures/wood.png", b"\x89PNG"),
      ],
      MediaType::Usdz,
    ),
  ];
  for (entries, media_type) in cases {
    assert_eq!(
      sniff(&zipped(&entries)),
      contained(media_type),
      "{media_type}"
    );
  }
}

#[test]
fn a_zip_that_lists_nothing_telling_is_a_zip() {
  let plain = by_signature(MediaType::Zip, None);
  assert_eq!(sniff(&zipped(&[("notes.txt", b"hello")])), plain);
  assert_eq!(sniff(&zipped(&[])), plain);
  // Half of what makes a Wallet pass.
  assert_eq!(sniff(&zipped(&[("pass.json", b"{}")])), plain);
  // A type declared the way an EPUB declares its own, and not one kept in a zip.
  let odd: [(&str, &[u8]); 2] = [("mimetype", b"image/png"), ("picture.png", b"\x89PNG")];
  assert_eq!(sniff(&zipped(&odd)), plain);
  assert_eq!(
    sniff(b"PK is short for Phil Katz, who wrote the format.").media_type(),
    MediaType::Txt
  );
}

#[test]
fn the_head_of_an_office_file_is_named_once_it_reaches_the_main_part() {
  let entries: [(&str, &[u8]); 4] = [
    ("[Content_Types].xml", b"<Types/>"),
    ("_rels/.rels", b"<Relationships/>"),
    ("word/document.xml", b"<w:document/>"),
    ("word/media/image1.png", &[0; 9000]),
  ];
  let file = zipped(&entries);
  assert_eq!(sniff_head(&file[..4096]), contained(MediaType::Docx));
}

#[test]
fn a_head_that_reaches_no_telling_name_is_a_zip_that_asks_for_its_end() {
  // The parts in the order a writer that streams might leave them, the main part last.
  let entries: [(&str, &[u8]); 3] = [
    ("xl/media/image1.png", &[0; 90_000]),
    ("[Content_Types].xml", b"<Types/>"),
    ("xl/workbook.xml", b"<workbook/>"),
  ];
  let file = zipped(&entries);
  assert_eq!(
    sniff_head(&file[..4096]),
    by_signature(MediaType::Zip, Some(ByteRange::last(65_557)))
  );

  // With the size, the end is asked for by its offset; given it, the file is a workbook.
  let size = u64::try_from(file.len()).unwrap();
  let pieces = [Piece {
    offset: 0,
    bytes: &file[..4096],
  }];
  let first = sniff_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  let needs = first.needs().unwrap();
  assert_eq!(needs, ByteRange::forward(size - 65_557, 65_557));
  let pieces = [
    Piece {
      offset: 0,
      bytes: &file[..4096],
    },
    Piece {
      offset: needs.offset,
      bytes: &file[usize::try_from(needs.offset).unwrap()..],
    },
  ];
  let second = sniff_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(second, contained(MediaType::Xlsx));
}

// ── Compound files ───────────────────────────────────────────────

const MSI: [u8; 16] = [
  0x84, 0x10, 0x0C, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46,
];

#[test]
fn a_compound_file_is_named_by_what_its_root_holds() {
  let cases: [(&[Child], MediaType); 6] = [
    (&[("1Table", &[]), ("WordDocument", &[])], MediaType::Doc),
    (&[("Workbook", &[])], MediaType::Xls),
    // What Excel called the stream before 1997.
    (&[("Book", &[])], MediaType::Xls),
    (
      &[("Current User", &[]), ("PowerPoint Document", &[])],
      MediaType::Ppt,
    ),
    (&[("VisioDocument", &[])], MediaType::Vsd),
    // A Word document with a workbook embedded in it is a Word document.
    (
      &[("WordDocument", &[]), ("ObjectPool", &["Workbook"])],
      MediaType::Doc,
    ),
  ];
  for (children, media_type) in cases {
    let file = compound(children, [0; 16]);
    assert_eq!(sniff(&file), contained(media_type), "{media_type}");
  }
  assert_eq!(
    sniff(&compound(&[("\u{5}SummaryInformation", &[])], MSI)),
    contained(MediaType::Msi)
  );
}

#[test]
fn a_compound_file_that_holds_none_of_those_is_a_compound_file() {
  let plain = by_signature(MediaType::Cfb, None);
  assert_eq!(sniff(&compound(&[("Contents", &[])], [0; 16])), plain);
  // A document saved with a password: a compound file around what cannot be read.
  let locked: [(&str, &[&str]); 2] = [("EncryptionInfo", &[]), ("EncryptedPackage", &[])];
  assert_eq!(sniff(&compound(&locked, [0; 16])), plain);
}

#[test]
fn a_compound_file_whose_directory_the_head_does_not_reach_asks_for_it() {
  let file = compound(&[("Workbook", &[])], [0; 16]);
  assert_eq!(
    sniff_head(&file[..512]),
    by_signature(MediaType::Cfb, Some(ByteRange::forward(1024, 512)))
  );
  assert_eq!(sniff_head(&file), contained(MediaType::Xls));
}

// ── Far reads ────────────────────────────────────────────────────

#[test]
fn media_that_comes_first_asks_for_the_box_after_it_and_no_more() {
  // A megabyte of media in front of the movie's own description.
  let mut head = b"\0\x0F\x42\x40mdat".to_vec();
  head.resize(4096, 0);
  assert_eq!(
    sniff_head(&head),
    Sniffed::Unknown {
      needs: Some(ByteRange::forward(1_000_000, 8))
    }
  );
  let next = boxed(b"moov", &[]);
  let pieces = [
    Piece {
      offset: 0,
      bytes: &head,
    },
    Piece {
      offset: 1_000_000,
      bytes: &next,
    },
  ];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(sniff_source(&source), contained(MediaType::Mov));
}

const MACRO_DOCUMENT: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";
const MACRO_WORKBOOK: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";

/// What a fetcher ends with: the first `head` bytes, then each range the answer asks for.
/// With it, how many times it asked.
fn settled(file: &[u8], head: usize) -> (Sniffed, usize) {
  let size = u64::try_from(file.len()).unwrap();
  let mut given = vec![(0usize, head.min(file.len()))];
  for asked in 0..8 {
    let pieces: Vec<Piece<'_>> = given
      .iter()
      .map(|&(start, end)| Piece {
        offset: u64::try_from(start).unwrap(),
        bytes: &file[start..end],
      })
      .collect();
    let found = sniff_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
    let Some(needs) = found.needs() else {
      return (found, asked);
    };
    assert!(!needs.from_end, "the size is known");
    let start = usize::try_from(needs.offset).unwrap();
    let end = (start + usize::try_from(needs.length).unwrap()).min(file.len());
    given.push((start, end));
  }
  panic!("still asking after eight rounds");
}

#[test]
fn a_macro_enabled_document_is_told_from_a_plain_one_by_its_start_alone() {
  // Office writes the part that says so first, so the start of the file holds it.
  let file = office_package("word/document.xml", MACRO_DOCUMENT);
  assert_eq!(sniff(&file), contained(MediaType::Docm));
  // All of it but the end of its directory: every entry is there, and the file is not whole.
  let most = file.len() - 100;
  assert_eq!(settled(&file, most), (contained(MediaType::Docm), 0));
}

#[test]
fn a_package_that_keeps_its_content_types_elsewhere_asks_for_them() {
  let types = content_types("xl/workbook.xml", MACRO_WORKBOOK);
  let filler = vec![b'x'; 40_000];
  for sizes in [Sizes::InHeader, Sizes::AfterData] {
    let file = zip(
      &[
        ("xl/workbook.xml", b"<workbook/>"),
        ("xl/media/picture.bin", &filler),
        ("[Content_Types].xml", types.as_bytes()),
      ],
      sizes,
      b"",
    );
    assert_eq!(sniff(&file), contained(MediaType::Xlsm), "{sizes:?}");
    // From its start it is a workbook, and the answer says that it is not settled.
    let pieces = [Piece {
      offset: 0,
      bytes: &file[..2048],
    }];
    let size = u64::try_from(file.len()).ok();
    let head = sniff_source(&Source::from_pieces(&pieces, size).unwrap());
    assert_eq!(head.media_type(), MediaType::Xlsx, "{sizes:?}");
    assert!(head.needs().is_some(), "{sizes:?}");
    // Given what it asks for, it is the macro-enabled one, in three asks at the most.
    let (found, asked) = settled(&file, 2048);
    assert_eq!(found, contained(MediaType::Xlsm), "{sizes:?}");
    assert!(asked <= 3, "{sizes:?} took {asked} asks");
  }
}
