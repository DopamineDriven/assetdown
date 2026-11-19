//! Files put together in memory for the tests: boxes, a RIFF header, an EBML header, a zip and a
//! compound file. Each is the least that the format allows, written out field by field.

/// One ISO base media box: its length, its type, what it holds.
pub(crate) fn boxed(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
  let length = u32::try_from(8 + payload.len()).unwrap();
  [&length.to_be_bytes()[..], kind, payload].concat()
}

/// A file type box: a major brand, a version, and the brands the file is compatible with.
pub(crate) fn ftyp(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
  let mut payload = major.to_vec();
  payload.extend_from_slice(&[0, 0, 2, 0]);
  for brand in compatible {
    payload.extend_from_slice(*brand);
  }
  boxed(b"ftyp", &payload)
}

/// The start of a RIFF file: the container's letters, a length, the form, and a first chunk.
pub(crate) fn riff(container: &[u8; 4], form: &[u8; 4]) -> Vec<u8> {
  [
    &container[..],
    &36u32.to_le_bytes(),
    form,
    b"fmt \x10\0\0\0\x01\0\x02\0",
  ]
  .concat()
}

/// How an EBML header states its own length.
#[derive(Clone, Copy)]
pub(crate) enum LengthIn {
  OneByte,
  EightBytes,
}

/// An EBML header that declares `doc_type`, and the start of the segment that follows it.
pub(crate) fn ebml(doc_type: Option<&[u8]>, length_in: LengthIn) -> Vec<u8> {
  // EBMLVersion 1, EBMLReadVersion 1.
  let mut body = vec![0x42, 0x86, 0x81, 0x01, 0x42, 0xF7, 0x81, 0x01];
  if let Some(doc_type) = doc_type {
    body.extend_from_slice(&[0x42, 0x82, 0x80 | u8::try_from(doc_type.len()).unwrap()]);
    body.extend_from_slice(doc_type);
  }
  // DocTypeVersion 4.
  body.extend_from_slice(&[0x42, 0x87, 0x81, 0x04]);

  let mut file = vec![0x1A, 0x45, 0xDF, 0xA3];
  let length = u8::try_from(body.len()).unwrap();
  match length_in {
    LengthIn::OneByte => file.push(0x80 | length),
    LengthIn::EightBytes => file.extend_from_slice(&[0x01, 0, 0, 0, 0, 0, 0, length]),
  }
  file.extend_from_slice(&body);
  // A segment of unknown length.
  file.extend_from_slice(&[
    0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
  ]);
  file
}

/// How a zip's writer recorded each entry's size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Sizes {
  /// In the entry's own header, in front of its data.
  InHeader,
  /// After the data, as a writer that streams leaves them. The header in front holds zeros.
  AfterData,
}

fn le16(value: usize) -> [u8; 2] {
  u16::try_from(value).unwrap().to_le_bytes()
}

fn le32(value: usize) -> [u8; 4] {
  u32::try_from(value).unwrap().to_le_bytes()
}

/// The checksum a zip keeps of an entry's bytes.
fn crc32(bytes: &[u8]) -> u32 {
  let mut crc = !0u32;
  for byte in bytes {
    crc ^= u32::from(*byte);
    for _ in 0..8 {
      crc = if crc & 1 == 1 {
        (crc >> 1) ^ 0xEDB8_8320
      } else {
        crc >> 1
      };
    }
  }
  !crc
}

/// A zip of `entries`, each a name and its data, stored without compression. `comment` is what
/// the archive's closing record carries after it.
pub(crate) fn zip(entries: &[(&str, &[u8])], sizes: Sizes, comment: &[u8]) -> Vec<u8> {
  let flags: [u8; 2] = match sizes {
    Sizes::InHeader => [0, 0],
    Sizes::AfterData => [0x08, 0],
  };
  let mut file = Vec::new();
  let mut directory = Vec::new();
  for (name, data) in entries {
    let offset = file.len();
    let stated = match sizes {
      Sizes::InHeader => data.len(),
      Sizes::AfterData => 0,
    };
    // The entry's own header: signature, version, flags, method, time, date, checksum, the two
    // sizes, the lengths of the name and of the extra field.
    let checksum = crc32(data).to_le_bytes();
    let stated_checksum = match sizes {
      Sizes::InHeader => checksum,
      Sizes::AfterData => [0; 4],
    };
    file.extend_from_slice(b"PK\x03\x04");
    file.extend_from_slice(&[20, 0]);
    file.extend_from_slice(&flags);
    file.extend_from_slice(&[0; 6]);
    file.extend_from_slice(&stated_checksum);
    file.extend_from_slice(&le32(stated));
    file.extend_from_slice(&le32(stated));
    file.extend_from_slice(&le16(name.len()));
    file.extend_from_slice(&[0, 0]);
    file.extend_from_slice(name.as_bytes());
    file.extend_from_slice(data);
    if sizes == Sizes::AfterData {
      file.extend_from_slice(b"PK\x07\x08");
      file.extend_from_slice(&checksum);
      file.extend_from_slice(&le32(data.len()));
      file.extend_from_slice(&le32(data.len()));
    }

    // The same entry in the directory, which always has the sizes, and where the entry starts.
    directory.extend_from_slice(b"PK\x01\x02");
    directory.extend_from_slice(&[20, 0, 20, 0]);
    directory.extend_from_slice(&flags);
    directory.extend_from_slice(&[0; 6]);
    directory.extend_from_slice(&checksum);
    directory.extend_from_slice(&le32(data.len()));
    directory.extend_from_slice(&le32(data.len()));
    directory.extend_from_slice(&le16(name.len()));
    directory.extend_from_slice(&[0; 12]);
    directory.extend_from_slice(&le32(offset));
    directory.extend_from_slice(name.as_bytes());
  }
  let directory_at = file.len();
  file.extend_from_slice(&directory);
  // The record that closes the archive: how many entries, the directory's size and offset.
  file.extend_from_slice(b"PK\x05\x06");
  file.extend_from_slice(&[0; 4]);
  file.extend_from_slice(&le16(entries.len()));
  file.extend_from_slice(&le16(entries.len()));
  file.extend_from_slice(&le32(directory.len()));
  file.extend_from_slice(&le32(directory_at));
  file.extend_from_slice(&le16(comment.len()));
  file.extend_from_slice(comment);
  file
}

/// The first eight bytes of every compound file.
pub(crate) const COMPOUND_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const NOTHING: u32 = 0xFFFF_FFFF;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const TABLE_SECTOR: u32 = 0xFFFF_FFFD;

struct Node {
  name: String,
  kind: u8,
  right: u32,
  child: u32,
}

fn node_bytes(node: &Node, class_id: [u8; 16]) -> Vec<u8> {
  let mut entry = vec![0u8; 128];
  let units: Vec<u16> = node.name.encode_utf16().collect();
  for (at, unit) in units.iter().enumerate() {
    entry[2 * at..2 * at + 2].copy_from_slice(&unit.to_le_bytes());
  }
  entry[64..66].copy_from_slice(&le16(2 * (units.len() + 1)));
  entry[66] = node.kind;
  entry[67] = 1;
  entry[68..72].copy_from_slice(&NOTHING.to_le_bytes());
  entry[72..76].copy_from_slice(&node.right.to_le_bytes());
  entry[76..80].copy_from_slice(&node.child.to_le_bytes());
  entry[80..96].copy_from_slice(&class_id);
  entry[116..120].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
  entry
}

/// A compound file of 512-byte sectors. The root holds `children`; a child listed with names of
/// its own is a storage holding those, and one listed with none is a stream. `class_id` is the
/// root's. Sector 0 is the allocation table and the directory starts in sector 1.
pub(crate) fn compound(children: &[(&str, &[&str])], class_id: [u8; 16]) -> Vec<u8> {
  let id = |at: usize| u32::try_from(at).unwrap();
  let mut nodes = vec![Node {
    name: "Root Entry".to_string(),
    kind: 5,
    right: NOTHING,
    child: if children.is_empty() { NOTHING } else { 1 },
  }];
  // The root's children are entries 1 onward, each pointing at the next; theirs follow.
  let mut next_free = 1 + children.len();
  for (at, (name, held)) in children.iter().enumerate() {
    let last = at + 1 == children.len();
    nodes.push(Node {
      name: (*name).to_string(),
      kind: if held.is_empty() { 2 } else { 1 },
      right: if last { NOTHING } else { id(at + 2) },
      child: if held.is_empty() {
        NOTHING
      } else {
        id(next_free)
      },
    });
    next_free += held.len();
  }
  for (_, held) in children {
    for (at, name) in held.iter().enumerate() {
      let last = at + 1 == held.len();
      let own = nodes.len();
      nodes.push(Node {
        name: (*name).to_string(),
        kind: 2,
        right: if last { NOTHING } else { id(own + 1) },
        child: NOTHING,
      });
    }
  }
  let directory_sectors = nodes.len().div_ceil(4);

  let mut header = vec![0u8; 512];
  header[..8].copy_from_slice(&COMPOUND_MAGIC);
  header[24..26].copy_from_slice(&0x3Eu16.to_le_bytes());
  header[26..28].copy_from_slice(&3u16.to_le_bytes());
  header[28..30].copy_from_slice(&[0xFE, 0xFF]);
  header[30..32].copy_from_slice(&9u16.to_le_bytes());
  header[32..34].copy_from_slice(&6u16.to_le_bytes());
  header[44..48].copy_from_slice(&1u32.to_le_bytes());
  header[48..52].copy_from_slice(&1u32.to_le_bytes());
  header[56..60].copy_from_slice(&4096u32.to_le_bytes());
  header[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
  header[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
  // The sectors that hold the allocation table: sector 0, and no other.
  header[76..80].copy_from_slice(&0u32.to_le_bytes());
  let (unlisted, _) = header[80..].as_chunks_mut::<4>();
  for slot in unlisted {
    *slot = NOTHING.to_le_bytes();
  }

  // The table: what sector follows each. Sector 0 is the table itself; the directory's sectors
  // follow one another from sector 1.
  let mut table = vec![NOTHING; 128];
  table[0] = TABLE_SECTOR;
  let directory = table.iter_mut().enumerate().skip(1).take(directory_sectors);
  for (sector, next) in directory {
    *next = if sector == directory_sectors {
      END_OF_CHAIN
    } else {
      id(sector + 1)
    };
  }

  let mut file = header;
  for entry in table {
    file.extend_from_slice(&entry.to_le_bytes());
  }
  for (at, node) in nodes.iter().enumerate() {
    let class_id = if at == 0 { class_id } else { [0; 16] };
    file.extend_from_slice(&node_bytes(node, class_id));
  }
  // Entries that are not in use fill out the last sector of the directory.
  let unused = Node {
    name: String::new(),
    kind: 0,
    right: NOTHING,
    child: NOTHING,
  };
  while !file.len().is_multiple_of(512) {
    file.extend_from_slice(&node_bytes(&unused, [0; 16]));
  }
  file
}

/// One PNG chunk: its length, its type, its bytes, and the checksum of the last two.
pub(crate) fn png_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
  let length = u32::try_from(payload.len()).unwrap();
  let checksum = crc32(&[&kind[..], payload].concat());
  [
    &length.to_be_bytes()[..],
    kind,
    payload,
    &checksum.to_be_bytes(),
  ]
  .concat()
}

/// A PNG's header chunk.
pub(crate) fn png_header(width: u32, height: u32, bit_depth: u8, color_type: u8) -> Vec<u8> {
  let payload = [
    &width.to_be_bytes()[..],
    &height.to_be_bytes(),
    &[bit_depth, color_type, 0, 0, 0],
  ]
  .concat();
  png_chunk(b"IHDR", &payload)
}

/// A PNG of these chunks, in this order, after the signature.
pub(crate) fn png_of(chunks: &[Vec<u8>]) -> Vec<u8> {
  [b"\x89PNG\r\n\x1A\n".to_vec(), chunks.concat()].concat()
}

/// The image facts of an answer, which the test expects it to have.
pub(crate) fn image_facts(specs: crate::Specs) -> crate::ImageFacts {
  match specs.facts {
    crate::Facts::Image(facts) => *facts,
    other => panic!(
      "no image facts were read for {}: {other:?} {:?}",
      specs.media_type, specs.warnings
    ),
  }
}

/// A color profile that calls itself `description`, in the plain text of version 2 or the
/// UTF-16 of version 4, for color of the kind `data_space` names.
pub(crate) fn icc_profile(
  description: Option<&str>,
  version_four: bool,
  data_space: &[u8; 4],
) -> Vec<u8> {
  let mut profile = vec![0u8; 128];
  profile[4..8].copy_from_slice(b"appl");
  profile[8..10].copy_from_slice(if version_four { &[4, 0x30] } else { &[2, 0x10] });
  profile[12..16].copy_from_slice(b"mntr");
  profile[16..20].copy_from_slice(data_space);
  profile[20..24].copy_from_slice(b"XYZ ");
  for (field, value) in [2026u16, 11, 17, 23, 36, 27].into_iter().enumerate() {
    profile[24 + 2 * field..26 + 2 * field].copy_from_slice(&value.to_be_bytes());
  }
  profile[36..40].copy_from_slice(b"acsp");
  profile[48..52].copy_from_slice(b"APPL");
  let Some(description) = description else {
    profile.extend_from_slice(&0u32.to_be_bytes());
    return profile;
  };
  let tag = if version_four {
    let text: Vec<u8> = description
      .encode_utf16()
      .flat_map(u16::to_be_bytes)
      .collect();
    [
      &b"mluc\0\0\0\0"[..],
      &1u32.to_be_bytes(),
      &12u32.to_be_bytes(),
      b"enUS",
      &u32::try_from(text.len()).unwrap().to_be_bytes(),
      &28u32.to_be_bytes(),
      &text,
    ]
    .concat()
  } else {
    let length = u32::try_from(description.len() + 1).unwrap();
    [
      &b"desc\0\0\0\0"[..],
      &length.to_be_bytes(),
      description.as_bytes(),
      &[0],
    ]
    .concat()
  };
  // One tag: its name, where it is, how long it is.
  profile.extend_from_slice(&1u32.to_be_bytes());
  profile.extend_from_slice(b"desc");
  profile.extend_from_slice(&144u32.to_be_bytes());
  profile.extend_from_slice(&u32::try_from(tag.len()).unwrap().to_be_bytes());
  profile.extend_from_slice(&tag);
  profile
}

/// A value of a TIFF entry.
pub(crate) enum TiffValue {
  Short(Vec<u16>),
  Long(Vec<u32>),
  Text(&'static str),
  /// Numerators over denominators.
  Fractions(Vec<(u32, u32)>),
  /// Bytes of no stated meaning, which is how a profile and an XMP packet are kept.
  Bytes(Vec<u8>),
  /// A directory of its own, which the entry points at.
  Directory(Vec<(u16, TiffValue)>),
  /// Several directories, which the entry lists the offsets of.
  Directories(Vec<Vec<(u16, TiffValue)>>),
}

fn ordered<const N: usize>(little: bool, le: [u8; N], be: [u8; N]) -> [u8; N] {
  if little { le } else { be }
}

fn tiff16(little: bool, value: u16) -> [u8; 2] {
  ordered(little, value.to_le_bytes(), value.to_be_bytes())
}

fn tiff32(little: bool, value: u32) -> [u8; 4] {
  ordered(little, value.to_le_bytes(), value.to_be_bytes())
}

/// A directory placed at `at`, with the values that do not fit its entries after it, and the
/// directories it points at after those. The offset of the next directory is left zero.
fn tiff_directory(little: bool, entries: &[(u16, TiffValue)], at: usize) -> Vec<u8> {
  let table = 2 + 12 * entries.len() + 4;
  let mut directory = tiff16(little, u16::try_from(entries.len()).unwrap()).to_vec();
  let mut after: Vec<u8> = Vec::new();
  for (tag, value) in entries {
    let here = at + table + after.len();
    let (kind, count, bytes): (u16, usize, Vec<u8>) = match value {
      TiffValue::Short(values) => (
        3,
        values.len(),
        values.iter().flat_map(|v| tiff16(little, *v)).collect(),
      ),
      TiffValue::Long(values) => (
        4,
        values.len(),
        values.iter().flat_map(|v| tiff32(little, *v)).collect(),
      ),
      TiffValue::Text(text) => (2, text.len() + 1, [text.as_bytes(), &[0]].concat()),
      TiffValue::Fractions(values) => (
        5,
        values.len(),
        values
          .iter()
          .flat_map(|(over, under)| [tiff32(little, *over), tiff32(little, *under)].concat())
          .collect(),
      ),
      TiffValue::Bytes(bytes) => (7, bytes.len(), bytes.clone()),
      TiffValue::Directory(inner) => {
        after.extend(tiff_directory(little, inner, here));
        (4, 1, tiff32(little, u32::try_from(here).unwrap()).to_vec())
      }
      TiffValue::Directories(list) => {
        // The directories come first, and the list of where they are after them.
        let mut offsets = Vec::new();
        for inner in list {
          let at = at + table + after.len();
          offsets.extend(tiff32(little, u32::try_from(at).unwrap()));
          after.extend(tiff_directory(little, inner, at));
        }
        (4, list.len(), offsets)
      }
    };
    directory.extend(tiff16(little, *tag));
    directory.extend(tiff16(little, kind));
    directory.extend(tiff32(little, u32::try_from(count).unwrap()));
    if bytes.len() <= 4 {
      directory.extend(&bytes);
      directory.resize(directory.len() + 4 - bytes.len(), 0);
    } else {
      let here = at + table + after.len();
      directory.extend(tiff32(little, u32::try_from(here).unwrap()));
      after.extend(&bytes);
    }
  }
  directory.extend([0; 4]);
  directory.extend(after);
  directory
}

/// A TIFF of these pages, whose first directory is at `first_at`. What lies between the
/// header and that directory is zeros, as a strip of pixels would be anything.
pub(crate) fn tiff_at(little: bool, first_at: usize, pages: &[Vec<(u16, TiffValue)>]) -> Vec<u8> {
  let mut file = [
    if little { &b"II"[..] } else { &b"MM"[..] },
    &tiff16(little, 42),
    &tiff32(little, u32::try_from(first_at).unwrap()),
  ]
  .concat();
  file.resize(first_at, 0);
  for (index, page) in pages.iter().enumerate() {
    let at = file.len();
    let mut directory = tiff_directory(little, page, at);
    if index + 1 < pages.len() {
      let next = tiff32(little, u32::try_from(at + directory.len()).unwrap());
      let pointer = 2 + 12 * page.len();
      directory[pointer..pointer + 4].copy_from_slice(&next);
    }
    file.extend(directory);
  }
  file
}

/// A TIFF of these pages, with its first directory right after the header.
pub(crate) fn tiff(little: bool, pages: &[Vec<(u16, TiffValue)>]) -> Vec<u8> {
  tiff_at(little, 8, pages)
}

/// A BigTIFF of one directory, whose values each fit their entry.
pub(crate) fn big_tiff(little: bool, entries: &[(u16, u16, u64)]) -> Vec<u8> {
  let wide = |value: u64| ordered(little, value.to_le_bytes(), value.to_be_bytes());
  let mut file = [
    if little { &b"II"[..] } else { &b"MM"[..] },
    &tiff16(little, 43),
    &tiff16(little, 8),
    &[0, 0],
    &wide(16),
  ]
  .concat();
  file.extend(wide(u64::try_from(entries.len()).unwrap()));
  for (tag, kind, value) in entries {
    file.extend(tiff16(little, *tag));
    file.extend(tiff16(little, *kind));
    file.extend(wide(1));
    // A value shorter than eight bytes stands at the start of the field.
    let mut field = match kind {
      3 => tiff16(little, u16::try_from(*value).unwrap()).to_vec(),
      4 => tiff32(little, u32::try_from(*value).unwrap()).to_vec(),
      _ => wide(*value).to_vec(),
    };
    field.resize(8, 0);
    file.extend(field);
  }
  file.extend(wide(0));
  file
}

/// A JPEG of these segments, each a marker and what it holds, then the start of the picture.
pub(crate) fn jpeg(segments: &[(u8, Vec<u8>)]) -> Vec<u8> {
  let mut file = vec![0xFF, 0xD8];
  for (marker, payload) in segments {
    file.extend([0xFF, *marker]);
    file.extend(u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    file.extend(payload);
  }
  file.extend([0xFF, 0xDA, 0, 2, 0xFF, 0xD9]);
  file
}

/// A JPEG frame header: eight bits to the sample, the height, the width, the channels.
pub(crate) fn jpeg_frame(width: u16, height: u16, components: u8) -> Vec<u8> {
  let mut frame = [
    &[8][..],
    &height.to_be_bytes(),
    &width.to_be_bytes(),
    &[components],
  ]
  .concat();
  for id in 1..=components {
    frame.extend([id, 0x11, 0]);
  }
  frame
}

/// An EXIF block: a camera's name, a turn, when and through what lens the picture was taken,
/// and where, south and west of the origin.
pub(crate) fn exif_block(little: bool) -> Vec<u8> {
  tiff(
    little,
    &[vec![
      (0x010F, TiffValue::Text("Leica Camera AG")),
      (0x0110, TiffValue::Text("M11")),
      (0x0112, TiffValue::Short(vec![6])),
      (
        0x8769,
        TiffValue::Directory(vec![
          (0x9003, TiffValue::Text("2026:11:17 23:36:27")),
          (0x9011, TiffValue::Text("-05:00")),
          (0xA001, TiffValue::Short(vec![1])),
          (0xA434, TiffValue::Text("Summilux-M 1:1.4/35")),
        ]),
      ),
      (
        0x8825,
        TiffValue::Directory(vec![
          (1, TiffValue::Text("S")),
          (2, TiffValue::Fractions(vec![(33, 1), (51, 1), (2484, 100)])),
          (3, TiffValue::Text("W")),
          (4, TiffValue::Fractions(vec![(70, 1), (39, 1), (36, 1)])),
        ]),
      ),
    ]],
  )
}

/// An XMP packet with a property written each of the ways a property can be.
pub(crate) const XMP_PACKET: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 9.1-c002">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:xmpMM="http://ns.adobe.com/xap/1.0/mm/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
    xmlns:tiff="http://ns.adobe.com/tiff/1.0/"
    tiff:Orientation="5"
    xmp:CreatorTool="Adobe Photoshop 26.0 (Windows)"
    xmp:CreateDate="2026-11-17T23:36:27-05:00"
    xmpMM:DocumentID="xmp.did:1234"
    photoshop:ColorMode="3"
    photoshop:ICCProfile="Display P3">
   <xmp:ModifyDate>2026-11-18T08:00:00-05:00</xmp:ModifyDate>
   <dc:format>image/jpeg</dc:format>
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">A harbor at dusk</rdf:li></rdf:Alt></dc:title>
   <dc:creator><rdf:Seq><rdf:li>First Maker</rdf:li><rdf:li>Second Maker</rdf:li></rdf:Seq></dc:creator>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

/// A PDF of these pages, each one line of text set in Helvetica on a US Letter page, with
/// `info` in its information dictionary. The cross-reference table is written out in full.
pub(crate) fn pdf(pages: &[&str], info: &[(&str, &str)]) -> Vec<u8> {
  pdf_with(pages, info, "")
}

/// The same, with `in_catalog` written into its catalog: the language of its text, say.
pub(crate) fn pdf_with(pages: &[&str], info: &[(&str, &str)], in_catalog: &str) -> Vec<u8> {
  let page_ids: Vec<usize> = (0..pages.len()).map(|page| 5 + 2 * page).collect();
  let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
  let entries: Vec<String> = info
    .iter()
    .map(|(key, value)| format!("/{key} ({value})"))
    .collect();
  let mut objects = vec![
    format!("<< /Type /Catalog /Pages 2 0 R {in_catalog} >>"),
    format!(
      "<< /Type /Pages /Kids [{}] /Count {} >>",
      kids.join(" "),
      pages.len()
    ),
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    format!("<< {} >>", entries.join(" ")),
  ];
  for (page, text) in pages.iter().enumerate() {
    let content = if text.is_empty() {
      String::new()
    } else {
      format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET")
    };
    objects.push(format!(
      "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
      page_ids[page] + 1
    ));
    objects.push(format!(
      "<< /Length {} >>\nstream\n{content}\nendstream",
      content.len()
    ));
  }
  let mut file = b"%PDF-1.7\n".to_vec();
  let mut offsets = Vec::new();
  for (index, object) in objects.iter().enumerate() {
    offsets.push(file.len());
    file.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).bytes());
  }
  let table_at = file.len();
  file.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
  for offset in offsets {
    file.extend(format!("{offset:010} 00000 n \n").bytes());
  }
  file.extend(
    format!(
      "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{table_at}\n%%EOF\n",
      objects.len() + 1
    )
    .bytes(),
  );
  file
}

/// One item of a HEIF file: its id, its type, the properties it has, and its bytes.
pub(crate) struct HeifItem {
  pub id: u16,
  pub kind: [u8; 4],
  /// Whole property boxes. A property two items share is listed once and pointed at twice.
  pub properties: Vec<Vec<u8>>,
  /// What an item of type `mime` holds.
  pub content_type: &'static str,
  pub data: Vec<u8>,
}

impl HeifItem {
  pub(crate) fn picture(id: u16, kind: &[u8; 4], properties: Vec<Vec<u8>>) -> Self {
    HeifItem {
      id,
      kind: *kind,
      properties,
      content_type: "",
      data: Vec::new(),
    }
  }
}

/// A box that opens with a version and three bytes of flags.
fn full_box(kind: &[u8; 4], version: u8, payload: &[u8]) -> Vec<u8> {
  boxed(kind, &[&[version, 0, 0, 0][..], payload].concat())
}

/// The property that gives a picture's size.
pub(crate) fn heif_size(width: u32, height: u32) -> Vec<u8> {
  full_box(
    b"ispe",
    0,
    &[width.to_be_bytes(), height.to_be_bytes()].concat(),
  )
}

/// The property that gives the depth of each of a picture's channels.
pub(crate) fn heif_depths(depths: &[u8]) -> Vec<u8> {
  full_box(
    b"pixi",
    0,
    &[&[u8::try_from(depths.len()).unwrap()][..], depths].concat(),
  )
}

/// A color property that names the space by coded values.
pub(crate) fn heif_coded_color(primaries: u16, transfer: u16, matrix: u16) -> Vec<u8> {
  boxed(
    b"colr",
    &[
      &b"nclx"[..],
      &primaries.to_be_bytes(),
      &transfer.to_be_bytes(),
      &matrix.to_be_bytes(),
      &[0x80],
    ]
    .concat(),
  )
}

/// A color property that carries a profile.
pub(crate) fn heif_profile(profile: &[u8]) -> Vec<u8> {
  boxed(b"colr", &[&b"prof"[..], profile].concat())
}

/// The property that marks an item as another's alpha.
pub(crate) fn heif_alpha() -> Vec<u8> {
  full_box(b"auxC", 0, b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0")
}

/// The `meta` box of these items, whose bytes begin at `data_at` in the file.
fn heif_meta(
  primary: u16,
  items: &[HeifItem],
  references: &[(&[u8; 4], u16, &[u16])],
  data_at: usize,
) -> Vec<u8> {
  let mut properties: Vec<&Vec<u8>> = Vec::new();
  let mut associations = Vec::new();
  let mut infos = Vec::new();
  let mut locations = Vec::new();
  let mut data_at = data_at;
  let mut located = 0u16;
  for item in items {
    // Which of the properties are this item's, by their place in the list, counted from 1.
    let mut own = vec![u8::try_from(item.properties.len()).unwrap()];
    for property in &item.properties {
      let place = match properties.iter().position(|listed| *listed == property) {
        Some(place) => place,
        None => {
          properties.push(property);
          properties.len() - 1
        }
      };
      own.push(u8::try_from(place + 1).unwrap());
    }
    associations.extend([&item.id.to_be_bytes()[..], &own].concat());
    let mut info = [&item.id.to_be_bytes()[..], &[0, 0], &item.kind, &[0]].concat();
    if !item.content_type.is_empty() {
      info.extend([item.content_type.as_bytes(), &[0]].concat());
    }
    infos.extend(full_box(b"infe", 2, &info));
    if !item.data.is_empty() {
      // The item, which file its bytes are in, how many runs, where the run is and how long.
      locations.extend(
        [
          &item.id.to_be_bytes()[..],
          &[0, 0],
          &[0, 1],
          &u32::try_from(data_at).unwrap().to_be_bytes(),
          &u32::try_from(item.data.len()).unwrap().to_be_bytes(),
        ]
        .concat(),
      );
      data_at += item.data.len();
      located += 1;
    }
  }
  let count = u16::try_from(items.len()).unwrap();
  let listed: Vec<u8> = properties
    .iter()
    .flat_map(|property| property.iter().copied())
    .collect();
  let pointed: Vec<u8> = references
    .iter()
    .flat_map(|(kind, from, to)| {
      let ids: Vec<u8> = to.iter().flat_map(|id| id.to_be_bytes()).collect();
      boxed(
        kind,
        &[
          &from.to_be_bytes()[..],
          &u16::try_from(to.len()).unwrap().to_be_bytes(),
          &ids,
        ]
        .concat(),
      )
    })
    .collect();
  let children = [
    full_box(b"hdlr", 0, b"\0\0\0\0pict\0\0\0\0\0\0\0\0\0\0\0\0\0"),
    full_box(b"pitm", 0, &primary.to_be_bytes()),
    full_box(b"iinf", 0, &[&count.to_be_bytes()[..], &infos].concat()),
    full_box(b"iref", 0, &pointed),
    boxed(
      b"iprp",
      &[
        boxed(b"ipco", &listed),
        full_box(
          b"ipma",
          0,
          &[&u32::from(count).to_be_bytes()[..], &associations].concat(),
        ),
      ]
      .concat(),
    ),
    // Offsets and lengths of four bytes, and no base offset.
    full_box(
      b"iloc",
      0,
      &[&[0x44, 0x00][..], &located.to_be_bytes(), &locations].concat(),
    ),
  ]
  .concat();
  full_box(b"meta", 0, &children)
}

/// A HEIF file of these brands and items, with the items' bytes in a box of media at the end.
pub(crate) fn heif(
  major: &[u8; 4],
  compatible: &[&[u8; 4]],
  primary: u16,
  items: &[HeifItem],
  references: &[(&[u8; 4], u16, &[u16])],
) -> Vec<u8> {
  let opening = ftyp(major, compatible);
  // The list of items says where their bytes are, and its own length says where that is.
  let length = heif_meta(primary, items, references, 0).len();
  let meta = heif_meta(primary, items, references, opening.len() + length + 8);
  let data: Vec<u8> = items
    .iter()
    .flat_map(|item| item.data.iter().copied())
    .collect();
  [opening, meta, boxed(b"mdat", &data)].concat()
}

/// A `moov` box with one track of pictures that holds `frames` of them.
pub(crate) fn heif_sequence(frames: u32) -> Vec<u8> {
  let sizes = full_box(
    b"stsz",
    0,
    &[0u32.to_be_bytes(), frames.to_be_bytes()].concat(),
  );
  let media = [
    full_box(b"hdlr", 0, b"\0\0\0\0pict\0\0\0\0\0\0\0\0\0\0\0\0\0"),
    boxed(b"minf", &boxed(b"stbl", &sizes)),
  ]
  .concat();
  boxed(b"moov", &boxed(b"trak", &boxed(b"mdia", &media)))
}

/// An Office package whose main part is at `main` and is of the content type `main_type`,
/// with properties that name it and the application that wrote it.
/// The part of an Office package that lists what each of its parts is. A part that is not the
/// main one is listed in front of it, as a real file's are.
pub(crate) fn content_types(main: &str, main_type: &str) -> String {
  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/{main}" ContentType="{main_type}"/></Types>"#
  )
}

pub(crate) fn office_package(main: &str, main_type: &str) -> Vec<u8> {
  let types = content_types(main, main_type);
  let relationships = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{main}"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#
  );
  let core = r#"<?xml version="1.0" encoding="UTF-8"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>A Harbor at Dusk</dc:title><dc:subject>Light on water</dc:subject><dc:creator>First Maker</dc:creator><cp:keywords>harbor; dusk, water</cp:keywords><dc:language>en-US</dc:language><dcterms:created xsi:type="dcterms:W3CDTF">2026-11-17T23:36:27Z</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">2026-11-18T08:00:00Z</dcterms:modified></cp:coreProperties>"#;
  let app = r#"<?xml version="1.0" encoding="UTF-8"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Made By Hand</Application><Pages>3</Pages><Words>250</Words></Properties>"#;
  zip(
    &[
      ("[Content_Types].xml", types.as_bytes()),
      ("_rels/.rels", relationships.as_bytes()),
      ("docProps/core.xml", core.as_bytes()),
      ("docProps/app.xml", app.as_bytes()),
      (main, b"<?xml version=\"1.0\"?><root/>"),
    ],
    Sizes::InHeader,
    b"",
  )
}
