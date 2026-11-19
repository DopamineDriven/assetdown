//! AVIF and HEIC: pictures kept as items in an ISO base media file. A `meta` box lists the
//! items, says which one is the picture, and ties each to its properties: its size, its color,
//! how it is turned. A picture stored as a grid of tiles has a size for the tiles as well, so
//! the size is the primary item's own and not the first one found.

use super::{ColorModel, ColorSpace, ImageDetail, ImageFacts, Reading};
use crate::container::isobmff::boxes;
use crate::container::{exif, icc, xmp};
use crate::source::{Missing, Source};

/// What only a picture of this family says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeifDetail {
  /// The codec the picture is kept in, as its item's type: `av01`, `hvc1`.
  pub codec: Option<String>,
  /// Bits in each sample.
  pub bit_depth: Option<u8>,
  /// The picture is stored as a grid of smaller pictures.
  pub tiled: bool,
}

/// No more boxes than this are walked at the top of the file, and no `meta` or `moov` box
/// longer than this is read.
const MOST_BOXES: usize = 10_000;
const LONGEST_BOX: u64 = 16 << 20;

/// The brands of a file that holds a sequence of pictures and not one.
const SEQUENCE_BRANDS: [&[u8; 4]; 6] = [b"avis", b"msf1", b"hevc", b"hevx", b"hevm", b"hevs"];

fn child<'a>(bytes: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
  boxes(bytes)
    .find(|(found, _)| found == kind)
    .map(|(_, payload)| payload)
}

/// A whole number of `width` bytes at `at`, most significant byte first. A width of zero is a
/// field that is not there, and reads as zero.
fn number(bytes: &[u8], at: usize, width: usize) -> Option<u64> {
  let field = bytes.get(at..at.checked_add(width)?)?;
  Some(
    field
      .iter()
      .fold(0, |value, byte| (value << 8) | u64::from(*byte)),
  )
}

/// Text up to the zero that ends it, and what follows the zero.
fn text(bytes: &[u8]) -> (String, &[u8]) {
  let end = bytes
    .iter()
    .position(|byte| *byte == 0)
    .unwrap_or(bytes.len());
  let rest = bytes.get(end + 1..).unwrap_or_default();
  (String::from_utf8_lossy(&bytes[..end]).into_owned(), rest)
}

struct Item {
  id: u64,
  kind: [u8; 4],
  /// What an item of type `mime` holds.
  content_type: String,
}

/// Where an item's bytes are: in the file, or in the `idat` box beside the list of items.
struct Location {
  id: u64,
  in_idat: bool,
  offset: u64,
  length: u64,
}

/// What a `meta` box says.
#[derive(Default)]
struct Meta<'a> {
  primary: Option<u64>,
  items: Vec<Item>,
  /// A reference's type, the item it is from, and the items it points at.
  references: Vec<([u8; 4], u64, Vec<u64>)>,
  /// Every property, in the order the file lists them. An item names its own by position.
  properties: Vec<([u8; 4], &'a [u8])>,
  /// An item, and the positions of its properties, counted from 1.
  associations: Vec<(u64, Vec<usize>)>,
  locations: Vec<Location>,
  idat: &'a [u8],
}

impl<'a> Meta<'a> {
  /// What the box holds. Every box inside it opens with a version and three bytes of flags.
  fn parse(payload: &'a [u8]) -> Option<Self> {
    let mut meta = Meta::default();
    for (kind, body) in boxes(payload.get(4..)?) {
      let version = body.first().copied().unwrap_or(0);
      // An id is two bytes in the older versions of a box and four in the later.
      let wide = |later: u8| if version >= later { 4 } else { 2 };
      match &kind {
        b"pitm" => meta.primary = number(body, 4, wide(1)),
        b"idat" => meta.idat = body,
        b"iinf" => {
          let entries = body.get(4 + wide(1)..)?;
          for (_, entry) in boxes(entries).filter(|(kind, _)| kind == b"infe") {
            let id_width = if entry.first() == Some(&3) { 4 } else { 2 };
            let Some(id) = number(entry, 4, id_width) else {
              continue;
            };
            let Some(kind) = entry
              .get(6 + id_width..)
              .and_then(|rest| rest.first_chunk::<4>())
            else {
              continue;
            };
            let (_, after_name) = text(entry.get(10 + id_width..).unwrap_or_default());
            let (content_type, _) = text(after_name);
            meta.items.push(Item {
              id,
              kind: *kind,
              content_type,
            });
          }
        }
        b"iref" => {
          let width = wide(1);
          for (kind, entry) in boxes(body.get(4..)?) {
            let (Some(from), Some(count)) = (number(entry, 0, width), number(entry, width, 2))
            else {
              continue;
            };
            let to = (0..usize::try_from(count).unwrap_or(0))
              .filter_map(|index| number(entry, width + 2 + index * width, width))
              .collect();
            meta.references.push((kind, from, to));
          }
        }
        b"iprp" => {
          // This one box has no version of its own; the two inside it do.
          if let Some(container) = child(body, b"ipco") {
            meta.properties = boxes(container).collect();
          }
          if let Some(list) = child(body, b"ipma") {
            meta.associate(list);
          }
        }
        // A list that is cut short gives what it held up to there.
        b"iloc" => {
          meta.locate(body);
        }
        _ => {}
      }
    }
    Some(meta)
  }

  /// Which properties each item has.
  fn associate(&mut self, list: &[u8]) -> Option<()> {
    let version = *list.first()?;
    let long_index = list.get(3)? & 1 != 0;
    let id_width = if version >= 1 { 4 } else { 2 };
    let index_width = if long_index { 2 } else { 1 };
    let entries = number(list, 4, 4)?;
    let mut at = 8;
    for _ in 0..entries {
      let id = number(list, at, id_width)?;
      let count = usize::try_from(number(list, at + id_width, 1)?).ok()?;
      at += id_width + 1;
      let mut indices = Vec::with_capacity(count);
      for _ in 0..count {
        // The top bit says whether a reader must understand the property; the rest is its
        // position.
        let field = number(list, at, index_width)?;
        let mask = if long_index { 0x7FFF } else { 0x7F };
        indices.push(usize::try_from(field & mask).ok()?);
        at += index_width;
      }
      self.associations.push((id, indices));
    }
    Some(())
  }

  /// Where each item's bytes are.
  fn locate(&mut self, body: &[u8]) -> Option<()> {
    let version = *body.first()?;
    let sizes = *body.get(4)?;
    let (offset_width, length_width) = (usize::from(sizes >> 4), usize::from(sizes & 0x0F));
    let more = *body.get(5)?;
    let base_width = usize::from(more >> 4);
    let index_width = if version >= 1 {
      usize::from(more & 0x0F)
    } else {
      0
    };
    let id_width = if version >= 2 { 4 } else { 2 };
    let items = number(body, 6, id_width)?;
    let mut at = 6 + id_width;
    for _ in 0..items {
      let id = number(body, at, id_width)?;
      at += id_width;
      let method = if version >= 1 {
        at += 2;
        number(body, at - 2, 2)? & 0x0F
      } else {
        0
      };
      // Which file the bytes are in, which is always this one here.
      at += 2;
      let base = number(body, at, base_width)?;
      at += base_width;
      let extents = number(body, at, 2)?;
      at += 2;
      for extent in 0..extents {
        at += index_width;
        let offset = number(body, at, offset_width)?;
        let length = number(body, at + offset_width, length_width)?;
        at += offset_width + length_width;
        // An item in more than one run of bytes is read for its first.
        if extent == 0 {
          self.locations.push(Location {
            id,
            in_idat: method == 1,
            offset: base.saturating_add(offset),
            length,
          });
        }
      }
    }
    Some(())
  }

  fn item(&self, id: u64) -> Option<&Item> {
    self.items.iter().find(|item| item.id == id)
  }

  /// An item's properties, in the order the file lists them for it.
  fn properties_of(&self, id: u64) -> impl Iterator<Item = &([u8; 4], &'a [u8])> {
    self
      .associations
      .iter()
      .filter(move |(item, _)| *item == id)
      .flat_map(|(_, indices)| indices)
      .filter_map(|index| self.properties.get(index.checked_sub(1)?))
  }

  /// An item's bytes: from the file, or from the box of bytes kept beside the items.
  fn bytes_of(&self, id: u64, source: &Source<'_>) -> Result<Option<Vec<u8>>, Missing> {
    let Some(location) = self.locations.iter().find(|location| location.id == id) else {
      return Ok(None);
    };
    let (Ok(offset), Ok(length)) = (
      usize::try_from(location.offset),
      usize::try_from(location.length),
    ) else {
      return Ok(None);
    };
    if u64::try_from(length).unwrap_or(u64::MAX) > LONGEST_BOX {
      return Ok(None);
    }
    if location.in_idat {
      return Ok(
        self
          .idat
          .get(offset..offset.saturating_add(length))
          .map(<[u8]>::to_vec),
      );
    }
    Ok(Some(source.read(location.offset, length)?.to_vec()))
  }
}

/// How a picture is turned and mirrored for display, as a matrix on its coordinates, with y
/// running down the page.
#[derive(Clone, Copy, PartialEq)]
struct Turn([i8; 4]);

impl Turn {
  const NONE: Turn = Turn([1, 0, 0, 1]);
  const QUARTER_ANTICLOCKWISE: Turn = Turn([0, 1, -1, 0]);
  /// Left and right change places.
  const LEFT_TO_RIGHT: Turn = Turn([-1, 0, 0, 1]);
  /// Top and bottom change places.
  const TOP_TO_BOTTOM: Turn = Turn([1, 0, 0, -1]);

  /// This turn, and then `next`.
  fn then(self, next: Turn) -> Turn {
    let (Turn([a, b, c, d]), Turn([e, f, g, h])) = (next, self);
    Turn([a * e + b * g, a * f + b * h, c * e + d * g, c * f + d * h])
  }

  /// The number EXIF gives this turn, from 1 to 8.
  fn orientation(self) -> Option<u8> {
    let numbered: [(u8, [i8; 4]); 8] = [
      (1, [1, 0, 0, 1]),
      (2, [-1, 0, 0, 1]),
      (3, [-1, 0, 0, -1]),
      (4, [1, 0, 0, -1]),
      (5, [0, 1, 1, 0]),
      (6, [0, -1, 1, 0]),
      (7, [0, -1, -1, 0]),
      (8, [0, 1, -1, 0]),
    ];
    numbered
      .iter()
      .find(|(_, matrix)| *matrix == self.0)
      .map(|(number, _)| *number)
  }
}

/// The `meta` and `moov` boxes at the top of the file, each read whole.
fn top_level(
  source: &Source<'_>,
  sequence: bool,
  reading: &mut Reading,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
  let (mut meta, mut movie) = (None, None);
  let mut at = 0u64;
  for _ in 0..MOST_BOXES {
    let done = meta.is_some() && (movie.is_some() || !sequence);
    let head = match source.array::<8>(at) {
      Ok(head) => head,
      // A still picture has said everything once its `meta` is read.
      Err(Missing::PastEnd) => break,
      Err(Missing::NotGiven(_)) if done => break,
      Err(missing) => {
        reading.take(Err::<(), _>(missing), "before its list of items");
        break;
      }
    };
    if done {
      break;
    }
    let stated = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
    let kind = [head[4], head[5], head[6], head[7]];
    let (header, length) = match stated {
      0 => (
        8,
        source
          .size()
          .map_or(u64::MAX, |size| size.saturating_sub(at)),
      ),
      1 => match source.array::<8>(at + 8) {
        Ok(long) => (16, u64::from_be_bytes(long)),
        Err(missing) => {
          reading.take(Err::<(), _>(missing), "inside a box's header");
          break;
        }
      },
      stated => (8, u64::from(stated)),
    };
    if length < header {
      reading.warn("a box states a length shorter than its own header");
      break;
    }
    if (&kind == b"meta" || &kind == b"moov") && length - header <= LONGEST_BOX {
      let wanted = usize::try_from(length - header).unwrap_or(0);
      let Some(payload) =
        reading.take(source.read(at + header, wanted), "inside its list of items")
      else {
        break;
      };
      match &kind {
        b"meta" => meta = Some(payload.to_vec()),
        _ => movie = Some(payload.to_vec()),
      }
    }
    at = at.saturating_add(length);
  }
  (meta, movie)
}

/// How many pictures a sequence holds: the samples of its first track of pictures.
fn sequence_frames(movie: &[u8]) -> Option<u32> {
  boxes(movie)
    .filter(|(kind, _)| kind == b"trak")
    .find_map(|(_, track)| {
      let media = child(track, b"mdia")?;
      let handler = child(media, b"hdlr")?.get(8..12)?;
      (handler == b"pict" || handler == b"vide").then_some(())?;
      let sizes = child(child(child(media, b"minf")?, b"stbl")?, b"stsz")?;
      u32::try_from(number(sizes, 8, 4)?).ok()
    })
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<ImageFacts> {
  let major = reading.take(source.array::<4>(8), "inside its first box")?;
  let sequence = SEQUENCE_BRANDS.contains(&&major);
  let (meta_bytes, movie) = top_level(source, sequence, reading);
  let Some(meta_bytes) = meta_bytes else {
    if reading.needs.is_none() && reading.warnings.is_empty() {
      reading.warn("the file has no list of items");
    }
    return None;
  };
  let meta = Meta::parse(&meta_bytes)?;
  let is_picture = |item: &&Item| !matches!(&item.kind, b"Exif" | b"mime" | b"uri ");
  let primary = meta
    .primary
    .and_then(|id| meta.item(id))
    .or_else(|| meta.items.iter().find(is_picture))?;

  let (mut width, mut height) = (0, 0);
  let mut depth = None;
  let mut channels = None;
  let mut coded: Option<(u64, u64, u64)> = None;
  let mut profile = None;
  let mut turn = Turn::NONE;
  let mut turned = false;
  // A grid is made of the items it points at. What it does not say of its own color and
  // depth, its tiles say for it; its size is its own alone.
  let tiled = &primary.kind == b"grid";
  let first_tile = meta
    .references
    .iter()
    .find(|(kind, from, _)| tiled && kind == b"dimg" && *from == primary.id)
    .and_then(|(_, _, to)| meta.item(*to.first()?));
  let says = |kind: &[u8; 4]| meta.properties_of(primary.id).any(|(own, _)| own == kind);
  let own = meta
    .properties_of(primary.id)
    .map(|property| (true, property));
  let tiles = first_tile
    .into_iter()
    .flat_map(|tile| meta.properties_of(tile.id))
    .map(|property| (false, property));
  for (own, (kind, body)) in own.chain(tiles) {
    match kind {
      b"ispe" if !own => {}
      b"pixi" if !own && says(b"pixi") => {}
      b"colr" if !own && says(b"colr") => {}
      b"irot" | b"imir" if !own => {}
      b"ispe" => {
        width = number(body, 4, 4)
          .and_then(|side| u32::try_from(side).ok())
          .unwrap_or(0);
        height = number(body, 8, 4)
          .and_then(|side| u32::try_from(side).ok())
          .unwrap_or(0);
      }
      b"pixi" => {
        channels = body.get(4).copied();
        depth = body.get(5).copied();
      }
      b"colr" => match body.get(..4) {
        Some(b"nclx") => {
          coded = Some((
            number(body, 4, 2)?,
            number(body, 6, 2)?,
            number(body, 8, 2)?,
          ));
        }
        Some(b"prof" | b"rICC") => profile = body.get(4..).and_then(icc::parse),
        _ => {}
      },
      // The properties are applied in the order the item lists them.
      b"irot" => {
        for _ in 0..body.first().copied().unwrap_or(0) & 0x03 {
          turn = turn.then(Turn::QUARTER_ANTICLOCKWISE);
        }
        turned = true;
      }
      // An axis of 0 exchanges the top and the bottom; an axis of 1, the left and the right.
      b"imir" => {
        let left_and_right = body.first().copied().unwrap_or(0) & 0x01 == 1;
        turn = turn.then(if left_and_right {
          Turn::LEFT_TO_RIGHT
        } else {
          Turn::TOP_TO_BOTTOM
        });
        turned = true;
      }
      _ => {}
    }
  }

  // An item that is the picture's alpha points at it and says so in a property of its own.
  let has_alpha = meta.references.iter().any(|(kind, from, to)| {
    kind == b"auxl"
      && to.contains(&primary.id)
      && meta.properties_of(*from).any(|(kind, body)| {
        let (purpose, _) = text(body.get(4..).unwrap_or_default());
        kind == b"auxC" && (purpose.contains("alpha") || purpose.ends_with("auxid:1"))
      })
  });
  // A grid is kept in whatever its tiles are kept in.
  let codec = if tiled {
    first_tile.map(|tile| tile.kind)
  } else {
    Some(primary.kind)
  };

  let gray = channels == Some(1);
  let color_model = match (gray, has_alpha, coded) {
    (true, true, _) => ColorModel::GrayscaleAlpha,
    (true, false, _) => ColorModel::Grayscale,
    (false, true, _) => ColorModel::Rgba,
    // A matrix of zero is red, green and blue kept as they are.
    (false, false, Some((_, _, 0))) => ColorModel::Rgb,
    (false, false, _) => ColorModel::Ycbcr,
  };
  // Coded values name the space outright. A picture that says nothing is taken for sRGB.
  let color_space = match coded {
    _ if gray => ColorSpace::Gray,
    Some((9, _, _)) => ColorSpace::Rec2020,
    Some((12, _, _)) => ColorSpace::DisplayP3,
    Some((1, transfer, _)) if transfer != 13 => ColorSpace::Rec709,
    _ => ColorSpace::Srgb,
  };
  let frames = movie
    .as_deref()
    .and_then(sequence_frames)
    .unwrap_or(1)
    .max(1);
  let detail = ImageDetail::Heif(HeifDetail {
    codec: codec.map(|codec| String::from_utf8_lossy(&codec).trim().to_string()),
    bit_depth: depth,
    tiled,
  });
  let mut facts = ImageFacts {
    frames,
    animated: frames > 1,
    has_alpha: Some(has_alpha),
    color_model,
    color_space,
    ..ImageFacts::sized(width, height, detail)
  };
  if let Some(profile) = profile {
    facts.take_profile(profile);
  }

  // A camera's notes and an XMP packet are items of their own, kept where the list says.
  for item in &meta.items {
    let wanted = match &item.kind {
      b"Exif" => true,
      b"mime" => item.content_type == "application/rdf+xml",
      _ => false,
    };
    if !wanted {
      continue;
    }
    let Some(Some(bytes)) = reading.take(meta.bytes_of(item.id, source), "inside an item") else {
      continue;
    };
    if &item.kind == b"mime" {
      facts.xmp = xmp::parse(&bytes);
      continue;
    }
    // Four bytes say how far in the block starts.
    let skip = number(&bytes, 0, 4)
      .and_then(|skip| usize::try_from(skip).ok())
      .unwrap_or(0);
    let block = bytes.get(4usize.saturating_add(skip)..).unwrap_or_default();
    if let Some(exif) = exif::from_bytes(block.strip_prefix(b"Exif\0\0").unwrap_or(block)) {
      facts.take_exif(exif);
    }
  }
  // How the picture is turned is what its own properties say; a reader of this format does
  // not turn it again by what the camera's notes say.
  facts.orientation = if turned { turn.orientation() } else { None };
  Some(facts)
}

#[cfg(test)]
mod tests;
