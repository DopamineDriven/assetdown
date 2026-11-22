//! What a file is, from a signature at a fixed place in it or from what its container says it
//! holds.
//!
//! The tests are tried from the strictest to the loosest, so that no type is offered to a loose
//! test before a stricter one could have claimed it. A signature short enough to begin an
//! ordinary text (`BM`, `true`, `ID3`) is not believed alone: the header behind it has to hold
//! together too.

use crate::container::isobmff::Brands;
use crate::container::{cfb, ebml, isobmff, riff, zip};
use crate::document::office;
use crate::image::{jpeg, png, tiff};
use crate::media_type::MediaType;
use crate::source::{ByteRange, Missing, Source};
use crate::text::{self, Kind};

/// How much of a file's start is asked for when the head given stops short of a signature.
const HEAD_BYTES: u64 = 4096;

/// What in the bytes named the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Evidence {
  /// A signature at a fixed place.
  Signature,
  /// What a container says it holds: a brand, a form type, the names of its entries.
  Container,
  /// How the content reads: a root element, how a text begins.
  Content,
  /// The file's name, for text whose kind the bytes cannot settle.
  FileName,
}

impl Evidence {
  pub fn as_str(self) -> &'static str {
    match self {
      Evidence::Signature => "signature",
      Evidence::Container => "container",
      Evidence::Content => "content",
      Evidence::FileName => "fileName",
    }
  }
}

/// A second look that named a type the first pass could not. It is taken only when asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Retry {
  /// The file's own start stands behind bytes that are not part of it.
  LeadingBytes {
    /// How many bytes stand in front of the file.
    skipped: u64,
  },
  /// The end of the file says what the start does not: a zip's directory, a PDF's trailer,
  /// a tag after the last frame of an MP3.
  EndOfFile,
  /// Text read with more allowance: UTF-16 with no byte-order mark, or a stray control
  /// character or two.
  LenientText,
}

impl Retry {
  pub fn as_str(self) -> &'static str {
    match self {
      Retry::LeadingBytes { .. } => "leadingBytes",
      Retry::EndOfFile => "endOfFile",
      Retry::LenientText => "lenientText",
    }
  }
}

/// What a sniff found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sniffed {
  /// The bytes name a type.
  Named {
    media_type: MediaType,
    evidence: Evidence,
    /// A range that would make the answer surer, when the bytes given stop short of it.
    needs: Option<ByteRange>,
    /// The second look that named it, when the first pass could not.
    retry: Option<Retry>,
  },
  /// Nothing in the bytes given names a type. This is an answer, not an error.
  Unknown {
    /// A range that could still name one, when the bytes given stop short of it.
    needs: Option<ByteRange>,
  },
}

impl Sniffed {
  /// The type, which is `application/octet-stream` for bytes that name none.
  pub fn media_type(&self) -> MediaType {
    match self {
      Sniffed::Named { media_type, .. } => *media_type,
      Sniffed::Unknown { .. } => MediaType::OctetStream,
    }
  }

  /// The range that would settle the answer. Always `None` for a whole file.
  pub fn needs(&self) -> Option<ByteRange> {
    match self {
      Sniffed::Named { needs, .. } | Sniffed::Unknown { needs } => *needs,
    }
  }
}

/// What the whole file in `bytes` is.
pub fn sniff(bytes: &[u8]) -> Sniffed {
  sniff_source(&Source::whole(bytes))
}

/// What a caller can say beside the bytes. It is never the type: finding the type is what the
/// call is for.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options<'a> {
  /// The file's name. It is consulted for text and for nothing else, since text is the one
  /// thing whose kind the bytes cannot always settle: Markdown and a Rust source file are both
  /// words.
  pub file_name: Option<&'a str>,
  /// Look again when the first pass names nothing: behind leading bytes, then at the end of
  /// the file, then at the bytes as text read with more allowance. An answer found this way
  /// says which look found it.
  pub retry_on_unknown: bool,
}

/// How much of the start of a file is read to tell whether it is text and of what kind.
const TEXT_SAMPLE: usize = 64 << 10;

/// What the file is of which `source` holds all or part.
pub fn sniff_source(source: &Source<'_>) -> Sniffed {
  sniff_with(source, Options::default())
}

/// The same, with what the caller can say beside the bytes.
pub fn sniff_with(source: &Source<'_>, options: Options<'_>) -> Sniffed {
  located(source, options).0
}

/// How far behind leading bytes a file's own start is looked for. A PDF's header may stand
/// anywhere in its first 1,024 bytes.
const LEADING_WINDOW: u64 = 1024;

/// How much of the end of a file is read for a PDF's trailer.
const TRAILER_BYTES: u64 = 1024;

/// The tests tried behind leading bytes. A signature is not one of them: every program that
/// reads a format holds its signature as a constant, and three or four bytes turn up by chance
/// in the middle of anything. Behind leading bytes a type is named only by what a constant
/// cannot be: a checksum that holds, or a structure that parses. A PDF and a zip are found from
/// their ends, where each keeps what confirms it.
const STRICT_TESTS: [fn(&mut Probe<'_, '_>) -> Option<Hit>; 3] =
  [checked_png, declared_elements, rooted_streams];

/// A PNG whose header chunk's checksum holds.
fn checked_png(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let holds = probe.found(png::header_holds(probe.source).map(Some))?;
  holds.then(|| MediaType::Png.into())
}

/// EBML whose header names its document type. The four bytes it opens with are not enough to
/// go on behind leading bytes.
fn declared_elements(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let header = probe.found(ebml::header(probe.source))?;
  let media_type = match header.doc_type.as_deref()? {
    "webm" => MediaType::Webm,
    _ => MediaType::Mkv,
  };
  Some(Hit::contained(media_type))
}

/// A range of a file with its first `by` bytes left off, as a range of the file itself.
fn unshifted(range: ByteRange, by: u64) -> ByteRange {
  if range.from_end {
    range
  } else {
    ByteRange::forward(range.offset.saturating_add(by), range.length)
  }
}

/// The answer, and how many bytes stand in front of the file it describes.
pub(crate) fn located(source: &Source<'_>, options: Options<'_>) -> (Sniffed, u64) {
  // A file known to be of no bytes has nothing to look at, and that is its answer.
  if source.size() == Some(0) {
    let empty = Sniffed::Named {
      media_type: MediaType::Empty,
      evidence: Evidence::Content,
      needs: None,
      retry: None,
    };
    return (empty, 0);
  }
  let first = first_pass(source, options);
  if !options.retry_on_unknown || !matches!(first, Sniffed::Unknown { .. }) {
    return (first, 0);
  }
  let named = |hit: Hit, retry: Retry, by: u64| {
    let found = Sniffed::Named {
      media_type: hit.media_type,
      evidence: hit.evidence,
      needs: hit.needs.map(|range| unshifted(range, by)),
      retry: Some(retry),
    };
    (found, by)
  };
  // Behind leading bytes: the same file, a little way in.
  for skipped in 1..source.head_len().min(LEADING_WINDOW) {
    let view = source.shifted(skipped);
    let mut probe = Probe {
      source: &view,
      short: None,
    };
    if let Some(hit) = STRICT_TESTS.iter().find_map(|test| test(&mut probe)) {
      return named(
        told_apart(&view, hit),
        Retry::LeadingBytes { skipped },
        skipped,
      );
    }
  }
  // At the end of the file. When the end was not given, it is what the answer asks for.
  let end = match at_the_end(source) {
    Ok(Some((hit, retry, by))) => return named(hit, retry, by),
    Ok(None) | Err(Missing::PastEnd) => None,
    Err(Missing::NotGiven(range)) => Some(range),
  };
  if let Some(hit) = written(source, options, true) {
    // The end of the file is looked at before the start is read with allowance. When the end
    // was not given, the answer names what the start is and asks for it. A file whose size
    // is not known has no end that could be placed, and is not asked for one.
    let end = end.filter(|_| source.size().is_some());
    let hit = Hit {
      needs: end.or(hit.needs),
      ..hit
    };
    return named(hit, Retry::LenientText, 0);
  }
  let needs = first.needs().or(end);
  (Sniffed::Unknown { needs }, 0)
}

/// What the end of the file says it is: a zip by its directory, a PDF by its trailer, an MP3
/// by the tag after its last frame. With it, which look that makes it and how many bytes stand
/// in front of the file.
fn at_the_end(source: &Source<'_>) -> Result<Option<(Hit, Retry, u64)>, Missing> {
  // A zip says where it starts: its directory counts either from the zip's own first byte or
  // from the file's, and in both cases the records it points at have to be there.
  let zipped = zip::start_from_end(source);
  if let Ok(Some(start)) = zipped {
    let view = source.shifted(start);
    let listing = zip::listing_by_directory(&view).map_err(|missing| match missing {
      Missing::NotGiven(range) => Missing::NotGiven(unshifted(range, start)),
      Missing::PastEnd => Missing::PastEnd,
    })?;
    if let Some(listing) = listing {
      return Ok(Some((listed(&listing, &view), Retry::EndOfFile, start)));
    }
  }
  let Some(size) = source.size() else {
    return zipped.map(|_| None);
  };
  // A PDF's trailer and an MP3's tag stand in the last bytes of the file, which may be here
  // when the much longer stretch a zip's directory could close in is not. They are read with
  // what was given, and only then is what was missing asked for.
  let reach = size.min(TRAILER_BYTES);
  let end = source.read(size - reach, usize::try_from(reach).unwrap_or(0));
  if let Ok(end) = end
    && ends_as_pdf(end)
  {
    // Its header may stand a little way in, and then that is where the file starts.
    return Ok(Some(match pdf_header_at(source) {
      Some(skipped) if skipped > 0 => (
        MediaType::Pdf.into(),
        Retry::LeadingBytes { skipped },
        skipped,
      ),
      _ => (MediaType::Pdf.into(), Retry::EndOfFile, 0),
    }));
  }
  let tag = size.checked_sub(128).map(|at| source.read(at, 3));
  if let Some(Ok(b"TAG")) = tag {
    return Ok(Some((MediaType::Mp3.into(), Retry::EndOfFile, 0)));
  }
  zipped?;
  end?;
  tag.transpose()?;
  Ok(None)
}

/// How a PDF ends: `startxref`, the place of its cross-reference table, and `%%EOF`. The two
/// words alone are constants in any program that writes one.
fn ends_as_pdf(end: &[u8]) -> bool {
  let word = b"startxref";
  let Some(at) = end.windows(word.len()).rposition(|window| window == word) else {
    return false;
  };
  let after = end
    .get(at + word.len()..)
    .unwrap_or_default()
    .trim_ascii_start();
  let digits = after
    .iter()
    .take_while(|byte| byte.is_ascii_digit())
    .count();
  let closes = after
    .get(digits..)
    .is_some_and(|rest| rest.trim_ascii_start().starts_with(b"%%EOF"));
  digits > 0 && closes
}

/// Where a PDF's header stands in the first bytes of the file: `%PDF-` and a version.
fn pdf_header_at(source: &Source<'_>) -> Option<u64> {
  let window = usize::try_from(LEADING_WINDOW).ok()?;
  let head = source.head(window + 8);
  let at = head.windows(8).position(|bytes| {
    matches!(
      bytes,
      [b'%', b'P', b'D', b'F', b'-', major, b'.', minor]
        if major.is_ascii_digit() && minor.is_ascii_digit()
    )
  })?;
  u64::try_from(at).ok()
}

/// What the file is, from one look at it.
fn first_pass(source: &Source<'_>, options: Options<'_>) -> Sniffed {
  let mut probe = Probe {
    source,
    short: None,
  };
  if let Some(hit) = TESTS.iter().find_map(|test| test(&mut probe)) {
    let hit = told_apart(source, hit);
    return Sniffed::Named {
      media_type: hit.media_type,
      evidence: hit.evidence,
      needs: hit.needs,
      retry: None,
    };
  }
  // Text comes last: it is what a file is when it is nothing else. While a test above still
  // lacks bytes near the start of the file, the head is asked for first.
  let settled = probe.short.is_none_or(|range| range.offset >= HEAD_BYTES);
  if settled && let Some(hit) = written(source, options, false) {
    return Sniffed::Named {
      media_type: hit.media_type,
      evidence: hit.evidence,
      needs: hit.needs,
      retry: None,
    };
  }
  // No test passed, but one that ran out of bytes that were never given still could. When what
  // it lacked is near the start, the head of the file is asked for, far enough for every test;
  // when it lies far into the file, as the box after a film's media does, that range alone is.
  let needs = probe.short.map(|range| {
    if range.offset >= HEAD_BYTES {
      return range;
    }
    let from = source.head_len();
    let to = range.offset.saturating_add(range.length).max(HEAD_BYTES);
    let to = source.size().map_or(to, |size| to.min(size));
    ByteRange::forward(from, to.saturating_sub(from))
  });
  Sniffed::Unknown { needs }
}

/// The file as text, when it is text, and the kind of text it is. `leniently` reads it with
/// the allowance a second look gives.
fn written(source: &Source<'_>, options: Options<'_>, leniently: bool) -> Option<Hit> {
  let sample = source.head(TEXT_SAMPLE);
  let cut = source
    .whole_file()
    .is_none_or(|file| file.len() > sample.len());
  let decode = if leniently {
    text::decode_leniently
  } else {
    text::decode
  };
  let (_, text) = decode(sample, cut)?;
  let (media_type, evidence) = match text::kind(&text, cut, options.file_name) {
    Kind::Content(media_type) => (media_type, Evidence::Content),
    Kind::Named(media_type) => (media_type, Evidence::FileName),
  };
  // Markup whose first tag runs past the bytes given cannot say what it is yet. More of the
  // file can change that only as far as text is read at all, so that far is asked for and no
  // further: an answer is not asked for twice, and a whole file asks for nothing.
  let given = source.head_len();
  let sample = u64::try_from(TEXT_SAMPLE).unwrap_or(u64::MAX);
  let read_to = source.size().map_or(sample, |size| size.min(sample));
  let opening_cut =
    given < read_to && text::markup::opening(&text) == text::markup::Opening::Unfinished;
  Some(Hit {
    media_type,
    evidence,
    needs: opening_cut.then(|| ByteRange::forward(given, read_to - given)),
  })
}

/// Some types open with the same signature and are told apart by what follows it. This is
/// looked at once the signature has named the file.
fn told_apart(source: &Source<'_>, hit: Hit) -> Hit {
  match hit.media_type {
    // A PNG is an animated one when a chunk in front of its picture says so.
    MediaType::Png => match png::is_animated(source) {
      Ok(true) => MediaType::Apng.into(),
      Ok(false) | Err(Missing::PastEnd) => hit,
      // The chunks in front of the picture were not all given: a PNG, and this would say
      // which. One chunk is followed by the next, so a block is asked for.
      Err(Missing::NotGiven(range)) => Hit {
        needs: Some(source.block(range)),
        ..hit
      },
    },
    // JPEG-LS opens as a JPEG does and differs in its frame header, which every writer of it
    // puts at the start. It is told when the bytes given reach that header and is never asked
    // for: nearly every file that opens this way is a JPEG.
    MediaType::Jpeg => match jpeg::frame_marker(source) {
      Ok(Some(jpeg::LS_FRAME)) => MediaType::Jls.into(),
      _ => hit,
    },
    // What a camera records is a TIFF whose directories say so. A directory is followed by
    // its values and by the directories it points at, so one that was not given is asked for
    // a block at a time.
    MediaType::Tiff => match tiff::kind(source) {
      Ok(media_type) => Hit { media_type, ..hit },
      Err(Missing::PastEnd) => hit,
      Err(Missing::NotGiven(range)) => Hit {
        needs: Some(source.block(range)),
        ..hit
      },
    },
    _ => hit,
  }
}

/// What a test found: the type, what named it, and a range that would make the answer surer.
struct Hit {
  media_type: MediaType,
  evidence: Evidence,
  needs: Option<ByteRange>,
}

/// A type named by a signature.
impl From<MediaType> for Hit {
  fn from(media_type: MediaType) -> Self {
    Hit {
      media_type,
      evidence: Evidence::Signature,
      needs: None,
    }
  }
}

impl Hit {
  /// A type named by what its container says it holds.
  fn contained(media_type: MediaType) -> Self {
    Hit {
      media_type,
      evidence: Evidence::Container,
      needs: None,
    }
  }
}

/// Reads for the tests, and remembers the read nearest the start of the file that failed
/// because its bytes were not given.
struct Probe<'s, 'a> {
  source: &'s Source<'a>,
  short: Option<ByteRange>,
}

impl<'s> Probe<'s, '_> {
  fn note(&mut self, range: ByteRange) {
    if self.short.is_none_or(|known| range.offset < known.offset) {
      self.short = Some(range);
    }
  }

  /// `length` bytes at `offset`, or nothing when they are not there, whatever the reason.
  fn bytes(&mut self, offset: u64, length: usize) -> Option<&'s [u8]> {
    self.found(self.source.read(offset, length).map(Some))
  }

  /// What a read found, or nothing when the bytes for it are not there, whatever the reason.
  fn found<T>(&mut self, read: Result<Option<T>, Missing>) -> Option<T> {
    match read {
      Ok(found) => found,
      Err(Missing::PastEnd) => None,
      Err(Missing::NotGiven(range)) => {
        self.note(range);
        None
      }
    }
  }

  fn array<const N: usize>(&mut self, offset: u64) -> Option<[u8; N]> {
    self.bytes(offset, N)?.try_into().ok()
  }

  fn is(&mut self, offset: u64, magic: &[u8]) -> bool {
    self.bytes(offset, magic.len()) == Some(magic)
  }

  /// Bytes a test wants once it has already named a type. When they were not given, the range
  /// to ask for comes back in their place; when the file ends before them, nothing does.
  fn further(&self, offset: u64, length: usize) -> Result<&'s [u8], Option<ByteRange>> {
    self
      .source
      .read(offset, length)
      .map_err(|missing| match missing {
        Missing::PastEnd => None,
        Missing::NotGiven(range) => Some(range),
      })
  }
}

/// The tests, strictest first. Boxes come before the signatures because a box opens with its
/// own length, and a length can read as anything: a first box of 442 bytes opens with the four
/// bytes an MPEG program stream does. The box test asks for more than four bytes can fake.
const TESTS: [fn(&mut Probe<'_, '_>) -> Option<Hit>; 21] = [
  boxes,
  jpeg_2000,
  exact,
  jxl_codestream,
  dpx,
  woff,
  sfnt,
  eot,
  bitmap,
  icon,
  bzip2,
  emf,
  chunks,
  elements,
  entries,
  streams,
  stl,
  ogg,
  id3,
  adts,
  mpeg_audio,
];

/// Brands that say a file of boxes is a picture, and in which codec.
const AVIF_BRANDS: [&[u8; 4]; 2] = [b"avif", b"avis"];
const HEIC_BRANDS: [&[u8; 4]; 8] = [
  b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"hevm", b"hevs",
];
const AVCI_BRANDS: [&[u8; 4]; 2] = [b"avci", b"avcs"];
/// A picture or a sequence of them, in a codec the brand does not name.
const HEIF_BRANDS: [&[u8; 4]; 2] = [b"mif1", b"msf1"];

/// ISO base media, named by the brands its first box declares.
fn boxes(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let opening = probe.found(isobmff::opening(probe.source))?;
  let media_type = match &opening.brands {
    Some(brands) => branded(brands),
    // A piece of a fragmented file.
    None if [*b"styp", *b"moof", *b"sidx"].contains(&opening.first) => MediaType::Mp4,
    // A QuickTime movie from before there were brands.
    None => MediaType::Mov,
  };
  Some(Hit::contained(media_type))
}

/// What a file's brands make it. The major brand says what the file is. For a picture it is
/// often the general one, and the codec is then among the brands it is compatible with.
fn branded(brands: &Brands) -> MediaType {
  let major_in = |family: &[&[u8; 4]]| family.contains(&&brands.major);
  let any_in = |family: &[&[u8; 4]]| {
    major_in(family)
      || brands
        .compatible
        .iter()
        .any(|brand| family.contains(&brand))
  };
  let picture = [
    &AVIF_BRANDS[..],
    &HEIC_BRANDS[..],
    &AVCI_BRANDS[..],
    &HEIF_BRANDS[..],
  ];
  if picture.iter().any(|family| major_in(family)) {
    return if any_in(&AVIF_BRANDS) {
      MediaType::Avif
    } else if any_in(&HEIC_BRANDS) {
      MediaType::Heic
    } else if any_in(&AVCI_BRANDS) {
      MediaType::Avci
    } else {
      MediaType::Heif
    };
  }
  match &brands.major {
    // Canon's third raw format, kept in boxes as a movie is.
    b"crx " => MediaType::Cr3,
    b"qt  " => MediaType::Mov,
    b"M4A " | b"M4B " | b"M4P " => MediaType::M4a,
    [b'3', b'g', b'2', _] => MediaType::ThreeG2,
    [b'3', b'g', ..] => MediaType::ThreeGp,
    _ => MediaType::Mp4,
  }
}

/// The box every file of the JPEG 2000 family opens with.
const JPEG_2000: &[u8] = b"\0\0\0\x0CjP  \r\n\x87\n";

/// The JPEG 2000 family: the signature box its files share, and then the brand that says
/// which of them a file is.
fn jpeg_2000(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  if !probe.is(0, JPEG_2000) {
    return None;
  }
  let brands = match isobmff::brands_at(probe.source, 12) {
    Ok(brands) => brands,
    Err(Missing::PastEnd) => None,
    // The family is known, and the brand would say which of it.
    Err(Missing::NotGiven(range)) => {
      return Some(Hit {
        needs: Some(range),
        ..MediaType::Jp2.into()
      });
    }
  };
  let media_type = match brands.as_ref().map(|brands| &brands.major) {
    Some(b"jpx ") => MediaType::Jpx,
    Some(b"jpm ") => MediaType::Jpm,
    Some(b"jph ") => MediaType::Jph,
    // Motion JPEG 2000 is video, and no type here.
    Some(b"mjp2") => return None,
    _ => MediaType::Jp2,
  };
  Some(media_type.into())
}

/// A JPEG XL codestream with no boxes around it opens with two bytes, which in an old encoding
/// of one byte to a character are a letter and a line ending. So what follows them must not
/// be text.
fn jxl_codestream(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  if !probe.is(0, &[0xFF, 0x0A]) {
    return None;
  }
  let after = probe.source.head(258).get(2..).unwrap_or_default();
  text::decode(after, true)
    .is_none()
    .then(|| MediaType::Jxl.into())
}

/// RIFF, named by its form. A form with no type here is not named.
fn chunks(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let media_type = match &probe.found(riff::form(probe.source))? {
    b"WEBP" => MediaType::Webp,
    b"WAVE" => MediaType::Wav,
    b"AVI " => MediaType::Avi,
    b"CDDA" => MediaType::Cda,
    _ => return None,
  };
  Some(Hit::contained(media_type))
}

/// EBML, named by its document type. WebM is Matroska kept to a few codecs, so a header that
/// does not say which is the general one.
fn elements(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let header = probe.found(ebml::header(probe.source))?;
  Some(Hit::contained(match header.doc_type.as_deref() {
    Some("webm") => MediaType::Webm,
    _ => MediaType::Mkv,
  }))
}

/// A zip, named by what it lists. One that lists nothing telling is a zip, and when its names
/// are not all here it says where the rest are.
fn entries(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let listing = probe.found(zip::listing(probe.source))?;
  Some(listed(&listing, probe.source))
}

/// What a zip is, by what it lists.
fn listed(listing: &zip::Listing, source: &Source<'_>) -> Hit {
  let lists = |name: &str| listing.names.iter().any(|listed| listed == name);
  // Only a type that is kept in a zip may be declared by one.
  let declared = listing
    .declared
    .as_deref()
    .and_then(MediaType::from_mime)
    .filter(|declared| {
      matches!(
        declared,
        MediaType::Epub | MediaType::Odt | MediaType::Ods | MediaType::Odp
      )
    });
  // A USDZ's first entry is the scene itself.
  let scene_first = listing.names.first().is_some_and(|first| {
    [".usd", ".usda", ".usdc"]
      .iter()
      .any(|extension| first.ends_with(extension))
  });
  let model_part = listing
    .names
    .iter()
    .any(|listed| listed.eq_ignore_ascii_case("3D/3dmodel.model"));
  // A fixed-page document's root part is its sequence of documents. It may have any name
  // and stand anywhere, and is told by how its name ends.
  let page_sequence = listing
    .names
    .iter()
    .any(|listed| listed.to_ascii_lowercase().ends_with(".fdseq"));

  let named = if declared.is_some() {
    declared
  } else if lists("word/document.xml") {
    Some(MediaType::Docx)
  } else if lists("xl/workbook.xml") {
    Some(MediaType::Xlsx)
  } else if lists("ppt/presentation.xml") {
    Some(MediaType::Pptx)
  } else if lists("pass.json") && lists("manifest.json") {
    Some(MediaType::Pkpass)
  } else if scene_first {
    Some(MediaType::Usdz)
  } else if model_part {
    Some(MediaType::ThreeMf)
  } else if page_sequence {
    Some(MediaType::Xps)
  } else if lists("META-INF/MANIFEST.MF") {
    Some(MediaType::Jar)
  } else {
    None
  };
  match named {
    // The names say Word, Excel or PowerPoint. Whether the file carries macros or is a
    // template is in the content type of its main part, which one small entry lists. When
    // that entry was not given, the answer names what the names say and asks for it.
    Some(office @ (MediaType::Docx | MediaType::Xlsx | MediaType::Pptx)) => {
      let main = match office {
        MediaType::Docx => "/word/document.xml",
        MediaType::Xlsx => "/xl/workbook.xml",
        _ => "/ppt/presentation.xml",
      };
      match zip::part(source, "[Content_Types].xml") {
        Ok(Some(types)) => Hit::contained(office::exact(&types, main).unwrap_or(office)),
        Ok(None) | Err(Missing::PastEnd) => Hit::contained(office),
        Err(Missing::NotGiven(range)) => Hit {
          needs: Some(range),
          ..Hit::contained(office)
        },
      }
    }
    Some(media_type) => Hit::contained(media_type),
    None => Hit {
      needs: listing.needs,
      ..MediaType::Zip.into()
    },
  }
}

/// The class Windows Installer gives the root of a package.
const INSTALLER_CLASS: [u8; 16] = [
  0x84, 0x10, 0x0C, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46,
];

/// A compound file, named by the stream its root holds. One that holds none of these is a
/// compound file and no more.
fn streams(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let root = probe.found(cfb::root(probe.source))?;
  Some(held_by(&root))
}

/// The same, for a compound file whose directory is there and opens with its root. Behind
/// leading bytes the eight bytes such a file begins with are a constant like any other.
fn rooted_streams(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let root = probe.found(cfb::root(probe.source))?;
  root.rooted.then(|| held_by(&root))
}

/// What a compound file is, by the streams under its root.
fn held_by(root: &cfb::Root) -> Hit {
  let holds = |name: &str| root.children.iter().any(|child| child == name);
  let named = if holds("WordDocument") {
    Some(MediaType::Doc)
  } else if holds("Workbook") || holds("Book") {
    Some(MediaType::Xls)
  } else if holds("PowerPoint Document") {
    Some(MediaType::Ppt)
  } else if holds("VisioDocument") {
    Some(MediaType::Vsd)
  } else if holds("EncryptedPackage") && holds("EncryptionInfo") {
    // What Word, Excel, and PowerPoint write for a file of 2007 or later saved with a
    // password: the package, encrypted, beside how it was encrypted.
    Some(MediaType::EncryptedOffice)
  } else if root.class_id == INSTALLER_CLASS {
    Some(MediaType::Msi)
  } else {
    None
  };
  match named {
    Some(media_type) => Hit::contained(media_type),
    None => Hit {
      needs: root.needs,
      ..MediaType::Cfb.into()
    },
  }
}

/// Bytes that stand at one place in every file of a type.
struct Signature {
  offset: u64,
  bytes: &'static [u8],
  media_type: MediaType,
}

const fn at_start(bytes: &'static [u8], media_type: MediaType) -> Signature {
  Signature {
    offset: 0,
    bytes,
    media_type,
  }
}

/// Where a tar's header keeps its signature.
const fn in_tar_header(bytes: &'static [u8]) -> Signature {
  Signature {
    offset: 257,
    bytes,
    media_type: MediaType::Tar,
  }
}

/// An IGES file is lines of 80 columns. Column 73 holds the letter of the line's section and
/// the seven after it the line's number, so the first line ends in `S` and a 1.
const fn in_iges_first_line(bytes: &'static [u8]) -> Signature {
  Signature {
    offset: 72,
    bytes,
    media_type: MediaType::Iges,
  }
}

const SIGNATURES: [Signature; 44] = [
  at_start(b"\x89PNG\r\n\x1A\n", MediaType::Png),
  // JPEG XL in its container, a JPEG 2000 codestream with no boxes around it, and JPEG XR in
  // either of its versions.
  at_start(b"\0\0\0\x0CJXL \r\n\x87\n", MediaType::Jxl),
  at_start(b"\xFF\x4F\xFF\x51", MediaType::J2c),
  at_start(b"II\xBC\x01", MediaType::Jxr),
  at_start(b"II\xBC\0", MediaType::Jxr),
  // What cameras record, where the format has a signature of its own: Fuji, Olympus in its
  // three forms, and Panasonic. Each is the letters and then where its first directory is.
  at_start(b"FUJIFILMCCD-RAW ", MediaType::Raf),
  at_start(b"IIRO\x08\0\0\0", MediaType::Orf),
  at_start(b"IIRS\x08\0\0\0", MediaType::Orf),
  at_start(b"MMOR\0\0\0\x08", MediaType::Orf),
  at_start(b"IIU\0\x18\0\0\0", MediaType::Rw2),
  at_start(b"\xFF\xD8\xFF", MediaType::Jpeg),
  at_start(b"GIF87a", MediaType::Gif),
  at_start(b"GIF89a", MediaType::Gif),
  // TIFF and BigTIFF, in either byte order.
  at_start(b"II*\0", MediaType::Tiff),
  at_start(b"MM\0*", MediaType::Tiff),
  at_start(b"II+\0", MediaType::Tiff),
  at_start(b"MM\0+", MediaType::Tiff),
  at_start(b"#?RADIANCE", MediaType::Hdr),
  at_start(b"#?RGBE", MediaType::Hdr),
  at_start(b"\xABKTX 11\xBB\r\n\x1A\n", MediaType::Ktx),
  at_start(b"\xABKTX 20\xBB\r\n\x1A\n", MediaType::Ktx2),
  at_start(b"%PDF-", MediaType::Pdf),
  // A form's data opens as the form does, with its own name in the place of `PDF`.
  at_start(b"%FDF-", MediaType::Fdf),
  at_start(b"{\\rtf", MediaType::Rtf),
  at_start(b"\x1F\x8B\x08", MediaType::Gzip),
  at_start(b"7z\xBC\xAF\x27\x1C", MediaType::SevenZip),
  // RAR up to version 4, then version 5.
  at_start(b"Rar!\x1A\x07\0", MediaType::Rar),
  at_start(b"Rar!\x1A\x07\x01\0", MediaType::Rar),
  // POSIX, then GNU.
  in_tar_header(b"ustar\x0000"),
  in_tar_header(b"ustar  \0"),
  // `fLaC`, then the header of the block every FLAC begins with: 34 bytes of stream facts.
  at_start(b"fLaC\0\0\0\x22", MediaType::Flac),
  at_start(b"fLaC\x80\0\0\x22", MediaType::Flac),
  // `MThd`, then the length its header always has.
  at_start(b"MThd\0\0\0\x06", MediaType::Midi),
  // A program stream's pack header, and a video stream's sequence header.
  at_start(b"\0\0\x01\xBA", MediaType::Mpeg),
  at_start(b"\0\0\x01\xB3", MediaType::Mpeg),
  at_start(b"\0asm", MediaType::Wasm),
  // A collection of fonts, in either of its two versions.
  at_start(b"ttcf\0\x01\0\0", MediaType::Ttc),
  at_start(b"ttcf\0\x02\0\0", MediaType::Ttc),
  // Universal Scene Description: the binary form, then the text form, which a version follows.
  at_start(b"PXR-USDC", MediaType::Usd),
  at_start(b"#usda ", MediaType::Usda),
  at_start(b"#VRML V", MediaType::Vrml),
  at_start(b"ISO-10303-21;", MediaType::Step),
  // The number is written with spaces in front of it, or with zeros.
  in_iges_first_line(b"S      1"),
  in_iges_first_line(b"S0000001"),
];

/// A binary glTF, in either of its two versions.
const GLB: [&[u8]; 2] = [b"glTF\x01\0\0\0", b"glTF\x02\0\0\0"];

fn exact(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  if GLB.iter().any(|magic| probe.is(0, magic)) {
    return Some(MediaType::Glb.into());
  }
  SIGNATURES
    .iter()
    .find(|signature| probe.is(signature.offset, signature.bytes))
    .map(|signature| signature.media_type.into())
}

/// DPX: its four letters in either byte order, then where the picture starts, then `V1.0` or
/// `V2.0`.
fn dpx(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let named = probe.is(0, b"SDPX") || probe.is(0, b"XPDS");
  let versioned = probe.is(8, b"V1") || probe.is(8, b"V2");
  (named && versioned).then(|| MediaType::Dpx.into())
}

/// The versions an sfnt font opens with: TrueType outlines, Apple's TrueType, and OpenType with
/// CFF outlines.
const SFNT_VERSIONS: [(&[u8; 4], MediaType); 3] = [
  (&[0, 1, 0, 0], MediaType::Ttf),
  (b"true", MediaType::Ttf),
  (b"OTTO", MediaType::Otf),
];

/// WOFF and WOFF2 wrap an sfnt font and say which kind right after their own four letters.
fn woff(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let media_type = if probe.is(0, b"wOFF") {
    MediaType::Woff
  } else if probe.is(0, b"wOF2") {
    MediaType::Woff2
  } else {
    return None;
  };
  let flavor: [u8; 4] = probe.array(4)?;
  SFNT_VERSIONS
    .iter()
    .any(|(version, _)| **version == flavor)
    .then(|| media_type.into())
}

/// An sfnt font: the version, then how many tables it has, then the tables' records, each of
/// which begins with a tag of four printable characters.
fn sfnt(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let version: [u8; 4] = probe.array(0)?;
  let (_, media_type) = SFNT_VERSIONS.iter().find(|(known, _)| **known == version)?;
  let tables = u16::from_be_bytes(probe.array(4)?);
  let tag: [u8; 4] = probe.array(12)?;
  let holds_together =
    (1..=255).contains(&tables) && tag.iter().all(|byte| (0x20..=0x7E).contains(byte));
  holds_together.then(|| (*media_type).into())
}

/// The versions an embedded OpenType font has had.
const EOT_VERSIONS: [u32; 3] = [0x0001_0000, 0x0002_0001, 0x0002_0002];

/// An embedded OpenType font: the file's length, the length of the font inside it, a version,
/// and `LP` thirty-four bytes in.
fn eot(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let length = u32::from_le_bytes(probe.array(0)?);
  let font_length = u32::from_le_bytes(probe.array(4)?);
  let version = u32::from_le_bytes(probe.array(8)?);
  let holds_together = EOT_VERSIONS.contains(&version)
    && probe.is(34, b"LP")
    && font_length > 0
    && font_length < length;
  holds_together.then(|| MediaType::Eot.into())
}

/// The sizes a bitmap's second header can have, from OS/2 1.x to Windows 5.
const BITMAP_HEADER_SIZES: [u32; 8] = [12, 16, 40, 52, 56, 64, 108, 124];

/// A bitmap: `BM`, four reserved bytes that are zero, where the pixels start, and the size of
/// the header that follows, which is one of a few.
fn bitmap(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  if !probe.is(0, b"BM") || !probe.is(6, &[0; 4]) {
    return None;
  }
  let pixels_at = u32::from_le_bytes(probe.array(10)?);
  let header_size = u32::from_le_bytes(probe.array(14)?);
  let holds_together =
    BITMAP_HEADER_SIZES.contains(&header_size) && pixels_at >= header_size.saturating_add(14);
  holds_together.then(|| MediaType::Bmp.into())
}

/// An icon or a cursor: two zero bytes, a 1 or a 2, how many pictures, and a first entry that
/// places its picture after the entries.
fn icon(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  if !probe.is(0, &[0, 0, 1, 0]) && !probe.is(0, &[0, 0, 2, 0]) {
    return None;
  }
  let pictures = u16::from_le_bytes(probe.array(4)?);
  let [reserved]: [u8; 1] = probe.array(9)?;
  let picture_size = u32::from_le_bytes(probe.array(14)?);
  let picture_at = u32::from_le_bytes(probe.array(18)?);
  let entries_end = 6 + 16 * u32::from(pictures);
  let holds_together =
    pictures > 0 && reserved == 0 && picture_size > 0 && picture_at >= entries_end;
  holds_together.then(|| MediaType::Ico.into())
}

/// bzip2: `BZh`, a level from 1 to 9, then the six bytes that open a block or, in a file that
/// compresses nothing, close the stream.
fn bzip2(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let [b'B', b'Z', b'h', b'1'..=b'9']: [u8; 4] = probe.array(0)? else {
    return None;
  };
  let opens = probe.is(4, b"\x31\x41\x59\x26\x53\x59") || probe.is(4, b"\x17\x72\x45\x38\x50\x90");
  opens.then(|| MediaType::Bzip2.into())
}

/// An enhanced metafile: a first record of type 1, and ` EMF` forty bytes in.
fn emf(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  (probe.is(0, &[1, 0, 0, 0]) && probe.is(40, b" EMF")).then(|| MediaType::Emf.into())
}

/// A binary STL has no signature: 80 bytes of anything, a count of triangles, and 50 bytes for
/// each. Only its length gives it away, so the length has to be known and has to fit exactly.
fn stl(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let size = probe.source.size()?;
  let triangles = u64::from(u32::from_le_bytes(probe.array(80)?));
  (triangles > 0 && size == 84 + 50 * triangles).then(|| MediaType::Stl.into())
}

/// An Ogg file is audio or video by the codec of its first stream, which the first packet of
/// the first page names.
fn ogg(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  // `OggS`, and the only version there is.
  if !probe.is(0, b"OggS\0") {
    return None;
  }
  let unnamed = |needs| Hit {
    needs,
    ..MediaType::Ogg.into()
  };
  // The page's header is 27 bytes, the last of which counts the segment lengths that follow.
  let segments = match probe.further(26, 1) {
    Ok(count) => count.first().copied().map_or(0, u64::from),
    Err(needs) => return Some(unnamed(needs)),
  };
  let packet = match probe.further(27 + segments, 8) {
    Ok(packet) => packet,
    Err(needs) => return Some(unnamed(needs)),
  };
  // Vorbis, Opus, FLAC and Speex; then Theora, VP8, Dirac and the video of an OGM file.
  let audio: [&[u8]; 4] = [b"\x01vorbis", b"OpusHead", b"\x7FFLAC", b"Speex   "];
  let video: [&[u8]; 4] = [b"\x80theora", b"OVP80", b"BBCD\0", b"\x01video"];
  let carries = |codecs: &[&[u8]]| codecs.iter().any(|codec| packet.starts_with(codec));
  let media_type = if carries(&audio) {
    MediaType::OggAudio
  } else if carries(&video) {
    MediaType::OggVideo
  } else {
    MediaType::Ogg
  };
  Some(media_type.into())
}

/// An ID3v2 tag stands in front of an MP3, and now and then in front of a FLAC or an AAC
/// stream. The tag states its own length, so what follows it can be looked at.
fn id3(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let [b'I', b'D', b'3', major @ 2..=4, 0, flags, size @ ..]: [u8; 10] = probe.array(0)? else {
    return None;
  };
  // Four bytes of seven bits each.
  if size.iter().any(|byte| *byte >= 0x80) {
    return None;
  }
  let body = size
    .iter()
    .fold(0u64, |sum, byte| (sum << 7) | u64::from(*byte));
  let footer = if major == 4 && flags & 0x10 != 0 {
    10
  } else {
    0
  };
  Some(match probe.further(10 + body + footer, 4) {
    Ok(b"fLaC") => MediaType::Flac.into(),
    Ok([first, second, third, _]) if is_adts(*first, *second, *third) => MediaType::Aac.into(),
    Ok(_) | Err(None) => MediaType::Mp3.into(),
    Err(needs) => Hit {
      needs,
      ..MediaType::Mp3.into()
    },
  })
}

/// The start of an ADTS header: twelve set bits, a layer of zero, and a sample rate that is one
/// of the thirteen there are.
fn is_adts(first: u8, second: u8, third: u8) -> bool {
  first == 0xFF && second & 0xF6 == 0xF0 && (third >> 2) & 0x0F <= 12
}

/// The length an ADTS frame states for itself, and the two bytes every frame of one stream
/// shares.
fn adts_frame(probe: &mut Probe<'_, '_>, offset: u64) -> Option<(u64, [u8; 2])> {
  let header: [u8; 6] = probe.array(offset)?;
  if !is_adts(header[0], header[1], header[2]) {
    return None;
  }
  let length =
    (u64::from(header[3] & 0x03) << 11) | (u64::from(header[4]) << 3) | u64::from(header[5] >> 5);
  // No frame is shorter than its own header.
  (length >= 7).then_some((length, [header[1], header[2] & 0xFC]))
}

/// AAC in ADTS has no signature, only a frame header a few bits wide, so one is not believed:
/// a second has to stand where the first says it ends, and belong to the same stream.
fn adts(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let (length, stream) = adts_frame(probe, 0)?;
  let (_, next) = adts_frame(probe, length)?;
  (stream == next).then(|| MediaType::Aac.into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MpegVersion {
  One,
  Two,
  TwoFive,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MpegLayer {
  One,
  Two,
  Three,
}

/// What every frame of one MPEG audio stream shares, and how long this frame is.
struct MpegFrame {
  stream: (MpegVersion, MpegLayer, usize),
  length: u64,
}

/// Kilobits a second by the header's bitrate index. Index 0 is a frame whose length the header
/// does not give, and 15 is not allowed.
const MPEG1_LAYER1: [u64; 15] = [
  0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
];
const MPEG1_LAYER2: [u64; 15] = [
  0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
];
const MPEG1_LAYER3: [u64; 15] = [
  0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
];
const MPEG2_LAYER1: [u64; 15] = [
  0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
];
const MPEG2_LAYERS2_3: [u64; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

/// Read four bytes as the header of an MPEG audio frame, when they are a valid one.
fn mpeg_frame(header: [u8; 4]) -> Option<MpegFrame> {
  let [0xFF, second, third, _] = header else {
    return None;
  };
  if second & 0xE0 != 0xE0 {
    return None;
  }
  let (version, rates) = match (second >> 3) & 0x03 {
    0 => (MpegVersion::TwoFive, [11_025, 12_000, 8_000]),
    2 => (MpegVersion::Two, [22_050, 24_000, 16_000]),
    3 => (MpegVersion::One, [44_100, 48_000, 32_000]),
    _ => return None,
  };
  let layer = match (second >> 1) & 0x03 {
    1 => MpegLayer::Three,
    2 => MpegLayer::Two,
    3 => MpegLayer::One,
    _ => return None,
  };
  let bitrates = match (version, layer) {
    (MpegVersion::One, MpegLayer::One) => MPEG1_LAYER1,
    (MpegVersion::One, MpegLayer::Two) => MPEG1_LAYER2,
    (MpegVersion::One, MpegLayer::Three) => MPEG1_LAYER3,
    (_, MpegLayer::One) => MPEG2_LAYER1,
    (_, MpegLayer::Two | MpegLayer::Three) => MPEG2_LAYERS2_3,
  };
  let rate_index = usize::from((third >> 2) & 0x03);
  let rate: u64 = *rates.get(rate_index)?;
  let bits = bitrates.get(usize::from(third >> 4)).copied()? * 1000;
  if bits == 0 {
    return None;
  }
  let padding = u64::from((third >> 1) & 0x01);
  let length = match (version, layer) {
    (_, MpegLayer::One) => (12 * bits / rate + padding) * 4,
    (MpegVersion::One, _) | (_, MpegLayer::Two) => 144 * bits / rate + padding,
    (_, MpegLayer::Three) => 72 * bits / rate + padding,
  };
  Some(MpegFrame {
    stream: (version, layer, rate_index),
    length,
  })
}

/// MPEG audio without a tag in front has no signature either, and its frame header is looser
/// than ADTS's: the byte-order mark of a UTF-16 text reads as one. So here too a second frame
/// of the same stream has to stand where the first says it ends.
fn mpeg_audio(probe: &mut Probe<'_, '_>) -> Option<Hit> {
  let first = mpeg_frame(probe.array(0)?)?;
  let second = mpeg_frame(probe.array(first.length)?)?;
  (first.stream == second.stream).then(|| MediaType::Mp3.into())
}

#[cfg(test)]
mod container_tests;
#[cfg(test)]
mod rare_tests;
#[cfg(test)]
mod retry_tests;
#[cfg(test)]
mod tests;
