pub mod composite;

use crate::types::PageImage;
use image::{DynamicImage, ImageBuffer, ImageFormat};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Cursor;

pub fn extract_images_raw(doc: &Document) -> Vec<PageImage> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut results: Vec<PageImage> = page_entries
    .par_iter()
    .flat_map(|&(page_num, page_id)| collect_page_images_raw(doc, page_id, page_num))
    .collect();
  results.sort_unstable_by_key(|r| (r.page, r.image_index));
  results
}

/// Decode all image XObjects on a page to DynamicImages (no PNG encoding).
/// Used by OCR to avoid the PNG encode→decode roundtrip and by render for empty page detection.
pub fn collect_page_decoded_images(doc: &Document, page_id: ObjectId) -> Vec<DynamicImage> {
  collect_page_decoded_images_named(doc, page_id)
    .into_iter()
    .map(|(_, img)| img)
    .collect()
}

/// Like [`collect_page_decoded_images`], keyed by the XObject resource name so
/// callers can match each image to where the content stream painted it.
pub fn collect_page_decoded_images_named(
  doc: &Document,
  page_id: ObjectId,
) -> Vec<(String, DynamicImage)> {
  page_image_sources(doc, page_id)
    .iter()
    .filter_map(|source| Some((source.name.clone(), source.decode()?)))
    .collect()
}

/// An image XObject a page paints, with what decoding it takes.
struct PageImageSource<'a> {
  /// The XObject's resource name.
  name: String,
  stream: &'a lopdf::Stream,
  width: u32,
  height: u32,
  bpc: u32,
  color_space: String,
  codec: Codec,
}

impl PageImageSource<'_> {
  /// Whether the image is in a coding kept for one-bit images. Those decode to black ink on white
  /// paper, a stencil mask included.
  fn is_one_bit(&self) -> bool {
    matches!(self.codec, Codec::Ccitt(_) | Codec::Jbig2(_))
  }

  fn decode(&self) -> Option<DynamicImage> {
    let content = match self.codec {
      Codec::Raw => {
        let mut s = self.stream.clone();
        if s.decompress().is_ok() {
          s.content
        } else {
          self.stream.content.clone()
        }
      }
      _ => image_codec_payload(self.stream),
    };
    // Skip SMask for OCR — to_rgb8() drops alpha anyway
    decode_xobject_to_dynamic_image(
      &content,
      self.width,
      self.height,
      self.bpc,
      &self.color_space,
      &self.codec,
      None,
    )
  }
}

/// The image XObjects in a page's resources that its content paints, undecoded.
fn page_image_sources(doc: &Document, page_id: ObjectId) -> Vec<PageImageSource<'_>> {
  let mut sources = Vec::new();

  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return sources,
  };

  let referenced_names = get_referenced_xobject_names(doc, page_id);

  for (name, obj_ref) in xobjects.iter() {
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }

    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });
    if subtype != Some(b"Image") {
      continue;
    }

    let width = get_dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let height = get_dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;
    if width == 0 || height == 0 {
      continue;
    }

    let color_space = resolve_color_space(&stream.dict, doc);
    let filter = resolve_filter(&stream.dict);
    let codec = codec_of(doc, &stream.dict, &filter, width, height);
    sources.push(PageImageSource {
      name: String::from_utf8_lossy(name).to_string(),
      stream,
      width,
      height,
      bpc,
      color_space,
      codec,
    });
  }

  sources
}

fn collect_page_images_raw(doc: &Document, page_id: ObjectId, page_num: u32) -> Vec<PageImage> {
  let mut images = Vec::new();

  // Get XObjects from page resources (with parent inheritance)
  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return images,
  };

  // Get the set of XObject names actually referenced by Do operators in the content stream
  let referenced_names = get_referenced_xobject_names(doc, page_id);

  let mut img_index = 0u32;

  for (name, obj_ref) in xobjects.iter() {
    // Only process XObjects actually painted on the page via Do operators
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }

    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    // Only process Image XObjects
    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });

    if subtype != Some(b"Image") {
      continue;
    }

    let width = get_dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let height = get_dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;

    if width == 0 || height == 0 {
      continue;
    }

    let color_space = resolve_color_space(&stream.dict, doc);
    let filter = resolve_filter(&stream.dict);

    let channels: u32 = match color_space.as_str() {
      "DeviceRGB" | "ICCBased3" | "CalRGB" => 3,
      "DeviceGray" | "ICCBased1" | "CalGray" => 1,
      "DeviceCMYK" | "ICCBased4" => 4,
      _ => 3,
    };

    // Step 4: Skip the full stream clone for DCT/JPX — they're already in their
    // target encoded format and don't need lopdf decompression.
    let codec = codec_of(doc, &stream.dict, &filter, width, height);
    let content = match codec {
      Codec::Raw => decompress_stream_content(doc, stream, width, height, channels, bpc),
      _ => image_codec_payload(stream),
    };

    // Check for SMask (alpha channel)
    let smask_data = get_smask_data(doc, &stream.dict);

    let png_data = match encode_to_png(
      &content,
      width,
      height,
      bpc,
      &color_space,
      &codec,
      smask_data.as_deref(),
    ) {
      Some(data) => data,
      None => continue,
    };
    // A stencil mask states no color space: it is painted in whatever the fill color is.
    // Decoded here it is a gray picture, and is reported as one.
    let color_space = if matches!(codec, Codec::Jbig2(_)) && !stream.dict.has(b"ColorSpace") {
      "DeviceGray".to_string()
    } else {
      color_space
    };

    let xobject_name = String::from_utf8_lossy(name).to_string();
    let object_id_str = format!("{} {} obj", obj_id.0, obj_id.1);

    images.push(PageImage {
      page: page_num,
      image_index: img_index,
      width,
      height,
      data: png_data,
      color_space,
      bits_per_component: bpc,
      filter,
      xobject_name,
      object_id: object_id_str,
    });

    img_index += 1;
  }

  images
}

/// Walk the page tree to find /Resources (handles inheritance from /Parent)
fn get_page_xobjects(doc: &Document, page_id: ObjectId) -> Option<lopdf::Dictionary> {
  let resources = get_inherited_resources(doc, page_id)?;
  let xobject_obj = resources.get(b"XObject").ok()?;
  resolve_to_dict(doc, xobject_obj)
}

pub(crate) fn get_inherited_resources(
  doc: &Document,
  page_id: ObjectId,
) -> Option<lopdf::Dictionary> {
  let mut current_id = Some(page_id);
  while let Some(id) = current_id {
    let dict = doc.get_dictionary(id).ok()?;
    if let Ok(resources_obj) = dict.get(b"Resources") {
      return resolve_to_dict(doc, resources_obj);
    }
    // Walk up to /Parent
    current_id = dict.get(b"Parent").ok().and_then(|p| match p {
      Object::Reference(ref_id) => Some(*ref_id),
      _ => None,
    });
  }
  None
}

/// Extract raw content bytes from a page's content stream(s).
fn get_page_content_bytes(doc: &Document, page_id: ObjectId) -> Vec<u8> {
  let page_dict = match doc.get_dictionary(page_id) {
    Ok(d) => d,
    Err(_) => return Vec::new(),
  };

  let contents = match page_dict.get(b"Contents") {
    Ok(c) => c,
    Err(_) => return Vec::new(),
  };

  let stream_ids: Vec<ObjectId> = match contents {
    Object::Reference(id) => vec![*id],
    Object::Array(arr) => arr
      .iter()
      .filter_map(|o| {
        if let Object::Reference(id) = o {
          Some(*id)
        } else {
          None
        }
      })
      .collect(),
    _ => return Vec::new(),
  };

  let mut all_bytes = Vec::new();
  for stream_id in stream_ids {
    if let Ok(Object::Stream(s)) = doc.get_object(stream_id) {
      let mut s = s.clone();
      let _ = s.decompress();
      all_bytes.extend_from_slice(&s.content);
    }
  }
  all_bytes
}

/// Scan content bytes for `Do` operators and collect XObject names.
fn collect_do_names(content_bytes: &[u8], names: &mut HashSet<Vec<u8>>) {
  if let Ok(content) = lopdf::content::Content::decode(content_bytes) {
    for op in &content.operations {
      if op.operator == "Do"
        && let Some(Object::Name(name)) = op.operands.first()
      {
        names.insert(name.clone());
      }
    }
  }
}

/// Parse the page content stream to find XObject names referenced by `Do` operators.
/// This filters out XObjects that are defined in Resources but never actually painted.
/// Also recurses one level into Form XObjects to find nested image references.
fn get_referenced_xobject_names(doc: &Document, page_id: ObjectId) -> HashSet<Vec<u8>> {
  let mut names = HashSet::new();

  let content_bytes = get_page_content_bytes(doc, page_id);
  if content_bytes.is_empty() {
    return names;
  }

  collect_do_names(&content_bytes, &mut names);

  // One-level recursion: check if any referenced XObjects are Form XObjects
  // and scan their content streams for additional Do operators
  if names.is_empty() {
    return names;
  }

  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return names,
  };

  let initial_names: Vec<Vec<u8>> = names.iter().cloned().collect();
  for name in &initial_names {
    let obj_ref = match xobjects.get(name.as_slice()) {
      Ok(r) => r,
      Err(_) => continue,
    };
    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };
    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };
    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });
    if subtype != Some(b"Form") {
      continue;
    }
    // Decompress and scan the Form XObject's content stream
    let mut form_stream = stream.clone();
    let _ = form_stream.decompress();
    collect_do_names(&form_stream.content, &mut names);
  }

  names
}

/// Check if any referenced XObject on the page has `/Subtype /Form`.
pub fn page_has_form_xobjects(doc: &Document, page_id: ObjectId) -> bool {
  page_has_xobject_subtype(doc, page_id, b"Form")
}

/// Check if any referenced XObject on the page has `/Subtype /Image`.
///
/// Used by the OCR cascade: when tier 2 (decode embedded images) produced nothing but the
/// page does paint image XObjects, the images exist in a form we could not decode (an indexed
/// color space, an unhandled filter chain, ...) and rasterizing the page is the only way to read them.
pub fn page_has_image_xobjects(doc: &Document, page_id: ObjectId) -> bool {
  page_has_xobject_subtype(doc, page_id, b"Image")
}

fn page_has_xobject_subtype(doc: &Document, page_id: ObjectId, wanted: &[u8]) -> bool {
  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return false,
  };

  let referenced_names = get_referenced_xobject_names(doc, page_id);

  for (name, obj_ref) in xobjects.iter() {
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }
    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };
    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };
    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });
    if subtype == Some(wanted) {
      return true;
    }
  }
  false
}

/// Check if the page content stream contains vector drawing operators.
/// Number of path construction and painting operators on the page. A few hundred means ruled
/// tables; tens of thousands means text printed as outlines, which only the render tier can read.
pub fn page_path_op_count(doc: &Document, page_id: ObjectId) -> usize {
  let content_bytes = get_page_content_bytes(doc, page_id);
  if content_bytes.is_empty() {
    return 0;
  }

  let content = match lopdf::content::Content::decode(&content_bytes) {
    Ok(c) => c,
    Err(_) => return 0,
  };

  let path_ops: &[&str] = &[
    "m", "l", "c", "v", "y", "h", "re", "S", "s", "f", "F", "f*", "B", "B*", "b", "b*", "n", "W",
    "W*",
  ];

  content
    .operations
    .iter()
    .filter(|op| path_ops.contains(&op.operator.as_str()))
    .count()
}

/// Resolve /DecodeParms from a stream dictionary, following indirect references.
fn resolve_decode_parms(doc: &Document, dict: &lopdf::Dictionary) -> Option<lopdf::Dictionary> {
  let dp = dict.get(b"DecodeParms").ok()?;
  match dp {
    Object::Dictionary(d) => Some(d.clone()),
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(Object::Dictionary(d)) => Some(d.clone()),
      _ => None,
    },
    Object::Array(arr) => {
      // Filter chain: DecodeParms is an array parallel to Filter array.
      // Use the first dictionary entry found.
      for item in arr {
        match item {
          Object::Dictionary(d) => return Some(d.clone()),
          Object::Reference(id) => {
            if let Ok(Object::Dictionary(d)) = doc.get_object(*id) {
              return Some(d.clone());
            }
          }
          _ => {}
        }
      }
      None
    }
    _ => None,
  }
}

/// Apply PNG predictor unfiltering to raw decompressed data.
/// Each row has a 1-byte filter type prefix followed by `row_bytes` of filtered data.
/// `bytes_per_pixel` is the number of bytes per pixel (channels * ceil(bpc/8)).
fn apply_png_predictor(data: &[u8], bytes_per_pixel: usize, row_bytes: usize) -> Option<Vec<u8>> {
  let src_row_len = row_bytes + 1; // +1 for filter type byte
  if !data.len().is_multiple_of(src_row_len) {
    return None;
  }
  let num_rows = data.len() / src_row_len;
  let mut output = Vec::with_capacity(num_rows * row_bytes);
  let mut prev_row = vec![0u8; row_bytes];

  for row_idx in 0..num_rows {
    let row_start = row_idx * src_row_len;
    let filter_byte = data[row_start];
    let mut current_row = data[row_start + 1..row_start + src_row_len].to_vec();

    match filter_byte {
      0 => { /* None */ }
      1 => {
        // Sub
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(current_row[i - bytes_per_pixel]);
        }
      }
      2 => {
        // Up
        for i in 0..row_bytes {
          current_row[i] = current_row[i].wrapping_add(prev_row[i]);
        }
      }
      3 => {
        // Average
        for i in 0..bytes_per_pixel {
          current_row[i] = current_row[i].wrapping_add(prev_row[i] / 2);
        }
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(
            ((current_row[i - bytes_per_pixel] as u16 + prev_row[i] as u16) / 2) as u8,
          );
        }
      }
      4 => {
        // Paeth
        for i in 0..bytes_per_pixel {
          current_row[i] = current_row[i].wrapping_add(paeth_predictor(0, prev_row[i], 0));
        }
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(paeth_predictor(
            current_row[i - bytes_per_pixel],
            prev_row[i],
            prev_row[i - bytes_per_pixel],
          ));
        }
      }
      _ => return None, // Unknown filter type
    }

    output.extend_from_slice(&current_row);
    prev_row = current_row;
  }

  Some(output)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
  let pa = (b as i16 - c as i16).abs();
  let pb = (a as i16 - c as i16).abs();
  let pc = (a as i16 + b as i16 - 2 * c as i16).abs();
  if pa <= pb && pa <= pc {
    a
  } else if pb <= pc {
    b
  } else {
    c
  }
}

/// Decompress a stream's content with correct predictor handling.
///
/// lopdf's built-in `decompress()` attempts PNG predictor unfiltering internally
/// but produces corrupted output for some streams (e.g. xdvipdfmx/pandoc images).
/// We bypass it entirely: raw zlib inflate via `flate2`, then apply our own
/// predictor reversal.
fn decompress_stream_content(
  doc: &Document,
  stream: &lopdf::Stream,
  width: u32,
  height: u32,
  channels: u32,
  bpc: u32,
) -> Vec<u8> {
  let bytes_per_sample = if bpc > 8 { 2u32 } else { 1u32 };
  let row_bytes = (width * channels * bpc / 8) as usize;
  let expected = (width * height * channels * bytes_per_sample) as usize;
  let predicted_len = height as usize * (row_bytes + 1);

  // Check if the stream uses FlateDecode
  let uses_flate = match stream.dict.get(b"Filter") {
    Ok(Object::Name(n)) => n == b"FlateDecode",
    Ok(Object::Array(arr)) => arr
      .iter()
      .any(|o| matches!(o, Object::Name(n) if n == b"FlateDecode")),
    _ => false,
  };

  // Step 1: Raw inflate — bypass lopdf's decompress to avoid its buggy predictor handling
  let content = if uses_flate {
    raw_inflate(&stream.content).unwrap_or_else(|| {
      // Fallback: let lopdf try (handles edge cases like chained filters)
      let mut s = stream.clone();
      if s.decompress().is_ok() {
        s.content
      } else {
        stream.content.clone()
      }
    })
  } else {
    stream.content.clone()
  };

  // Step 2: Apply predictor reversal if DecodeParms specifies one
  if let Some(dp) = resolve_decode_parms(doc, &stream.dict) {
    let predictor = get_dict_int(&dp, b"Predictor").unwrap_or(1);

    // TIFF Predictor 2: horizontal differencing (same size as raw pixels)
    if predictor == 2 && content.len() == expected {
      let bpp = (channels * bpc / 8).max(1) as usize;
      let mut data = content;
      apply_tiff_predictor2(&mut data, bpp, row_bytes);
      return data;
    }

    // PNG Predictors 10-15: each row has a leading filter type byte
    if (10..=15).contains(&predictor) && content.len() == predicted_len {
      let bpp = (channels * bpc / 8).max(1) as usize;
      if let Some(unfiltered) = apply_png_predictor(&content, bpp, row_bytes) {
        return unfiltered;
      }
    }
  }

  content
}

/// Raw zlib inflate without any predictor handling.
fn raw_inflate(data: &[u8]) -> Option<Vec<u8>> {
  use std::io::Read;
  // Try zlib wrapper first (most common in PDF)
  let mut output = Vec::new();
  if flate2::read::ZlibDecoder::new(data)
    .read_to_end(&mut output)
    .is_ok()
  {
    return Some(output);
  }
  // Fallback to raw deflate (no zlib header)
  output.clear();
  if flate2::read::DeflateDecoder::new(data)
    .read_to_end(&mut output)
    .is_ok()
  {
    return Some(output);
  }
  None
}

/// Reverse TIFF Predictor 2 (horizontal differencing) in-place.
/// Each byte after the first `bpp` bytes in each row is a delta from the previous byte.
fn apply_tiff_predictor2(data: &mut [u8], bpp: usize, row_bytes: usize) {
  if row_bytes == 0 {
    return;
  }
  let num_rows = data.len() / row_bytes;
  for row in 0..num_rows {
    let start = row * row_bytes;
    for i in (start + bpp)..(start + row_bytes) {
      data[i] = data[i].wrapping_add(data[i - bpp]);
    }
  }
}

/// Retrieve and decompress the SMask (soft mask / alpha channel) image data if present
fn get_smask_data(doc: &Document, dict: &lopdf::Dictionary) -> Option<Vec<u8>> {
  let smask_ref = dict.get(b"SMask").ok()?;
  let smask_id = match smask_ref {
    Object::Reference(id) => *id,
    _ => return None,
  };

  let smask_stream = match doc.get_object(smask_id) {
    Ok(Object::Stream(s)) => s,
    _ => return None,
  };

  // Verify it's an Image subtype
  let subtype = smask_stream.dict.get(b"Subtype").ok().and_then(|v| {
    if let Object::Name(n) = v {
      Some(n.as_slice())
    } else {
      None
    }
  });
  if subtype != Some(b"Image") {
    return None;
  }

  // SMask is always DeviceGray with 1 channel
  let smask_width = get_dict_int(&smask_stream.dict, b"Width").unwrap_or(0) as u32;
  let smask_height = get_dict_int(&smask_stream.dict, b"Height").unwrap_or(0) as u32;
  let smask_bpc = get_dict_int(&smask_stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;
  Some(decompress_stream_content(
    doc,
    smask_stream,
    smask_width,
    smask_height,
    1,
    smask_bpc,
  ))
}

fn resolve_to_dict(doc: &Document, obj: &Object) -> Option<lopdf::Dictionary> {
  match obj {
    Object::Dictionary(d) => Some(d.clone()),
    Object::Reference(id) => match doc.get_object(*id).ok()? {
      Object::Dictionary(d) => Some(d.clone()),
      _ => None,
    },
    _ => None,
  }
}

fn get_dict_int(dict: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
  match dict.get(key).ok()? {
    Object::Integer(i) => Some(*i),
    _ => None,
  }
}

fn resolve_color_space(dict: &lopdf::Dictionary, doc: &Document) -> String {
  let cs = match dict.get(b"ColorSpace") {
    Ok(obj) => obj,
    Err(_) => return "DeviceRGB".to_string(),
  };

  match cs {
    Object::Name(name) => String::from_utf8_lossy(name).to_string(),
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
      // ICCBased is typically [/ICCBased <stream ref>]
      Ok(Object::Array(arr)) => parse_color_space_array(arr, doc),
      _ => "DeviceRGB".to_string(),
    },
    Object::Array(arr) => parse_color_space_array(arr, doc),
    _ => "DeviceRGB".to_string(),
  }
}

fn parse_color_space_array(arr: &[Object], doc: &Document) -> String {
  if arr.is_empty() {
    return "DeviceRGB".to_string();
  }

  let cs_name = match &arr[0] {
    Object::Name(n) => String::from_utf8_lossy(n).to_string(),
    _ => return "DeviceRGB".to_string(),
  };

  if cs_name == "ICCBased" && arr.len() > 1 {
    // Get /N from the ICCBased stream to determine channel count
    let stream_id = match &arr[1] {
      Object::Reference(id) => *id,
      _ => return "ICCBased".to_string(),
    };

    if let Ok(Object::Stream(s)) = doc.get_object(stream_id) {
      let n = get_dict_int(&s.dict, b"N").unwrap_or(3);
      return format!("ICCBased{n}");
    }
  }

  cs_name
}

fn resolve_filter(dict: &lopdf::Dictionary) -> String {
  match dict.get(b"Filter") {
    Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
    Ok(Object::Array(arr)) => {
      // Filter chain — return the last (innermost) filter for image type detection
      if let Some(Object::Name(name)) = arr.last() {
        String::from_utf8_lossy(name).to_string()
      } else {
        "None".to_string()
      }
    }
    _ => "None".to_string(),
  }
}

/// Bytes to hand to an image codec (JPEG, JPEG 2000, fax, JBIG2): the stream with every filter
/// before the codec's own undone.
///
/// `/Filter` may be a chain such as `[ /ASCII85Decode /DCTDecode ]` (ReportLab, older
/// Ghostscript, fax and mail gateways). The codec is always the innermost (last) entry;
/// everything before it is a transport encoding that must be undone first, otherwise the
/// codec is fed ASCII text and the image is silently dropped. lopdf applies a filter chain
/// in order, so build a temporary stream whose `/Filter` holds only the outer filters and
/// let it decode those. A bare `/Filter /DCTDecode` returns the content untouched.
fn image_codec_payload(stream: &lopdf::Stream) -> Vec<u8> {
  let outer: Vec<Object> = match stream.dict.get(b"Filter") {
    Ok(Object::Array(arr)) if arr.len() > 1 => arr[..arr.len() - 1].to_vec(),
    _ => return stream.content.clone(),
  };
  let outer_len = outer.len();
  let mut dict = lopdf::Dictionary::new();
  dict.set("Filter", Object::Array(outer));
  // DecodeParms is parallel to Filter when both are arrays; keep only the outer entries.
  if let Ok(Object::Array(parms)) = stream.dict.get(b"DecodeParms") {
    let keep = parms.len().min(outer_len);
    dict.set("DecodeParms", Object::Array(parms[..keep].to_vec()));
  }
  let transport = lopdf::Stream::new(dict, stream.content.clone());
  transport
    .decompressed_content()
    .unwrap_or_else(|_| stream.content.clone())
}

/// How an image's bytes are coded, with what its codec needs beside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Codec {
  Jpeg,
  Jpx,
  Ccitt(CcittParams),
  Jbig2(Jbig2Params),
  /// Samples as the stream's own filters leave them.
  Raw,
}

/// The codec the innermost filter of an image names.
fn codec_of(
  doc: &Document,
  dict: &lopdf::Dictionary,
  filter: &str,
  width: u32,
  height: u32,
) -> Codec {
  match filter {
    "DCTDecode" => Codec::Jpeg,
    "JPXDecode" => Codec::Jpx,
    "CCITTFaxDecode" => {
      ccitt_params(doc, dict, filter, width, height).map_or(Codec::Raw, Codec::Ccitt)
    }
    "JBIG2Decode" => Codec::Jbig2(jbig2_params(doc, dict)),
    _ => Codec::Raw,
  }
}

/// Whether `/Decode [1 0]` on a one-bit image swaps which bit is the ink.
fn decode_is_inverted(dict: &lopdf::Dictionary) -> bool {
  match dict.get(b"Decode") {
    Ok(Object::Array(a)) => {
      matches!(a.first(), Some(Object::Integer(1)))
        || matches!(a.first(), Some(Object::Real(r)) if *r >= 0.5)
    }
    _ => false,
  }
}

/// What a `JBIG2Decode` image needs beside its own bytes (ISO 32000-1 §7.4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Jbig2Params {
  /// The segments several images share (symbol dictionaries, mostly), from the stream that
  /// `/DecodeParms /JBIG2Globals` names, with that stream's own filter undone.
  pub globals: Option<Vec<u8>>,
  /// `/Decode [1 0]` on the image dictionary.
  pub inverted: bool,
}

fn jbig2_params(doc: &Document, dict: &lopdf::Dictionary) -> Jbig2Params {
  let globals = resolve_decode_parms(doc, dict).and_then(|parms| {
    let (_, named) = doc.dereference(parms.get(b"JBIG2Globals").ok()?).ok()?;
    let Object::Stream(stream) = named else {
      return None;
    };
    if stream.dict.has(b"Filter") {
      stream.decompressed_content().ok()
    } else {
      Some(stream.content.clone())
    }
  });
  Jbig2Params {
    globals,
    inverted: decode_is_inverted(dict),
  }
}

/// The largest JBIG2 page that is decoded, in pixels: a gigapixel, which an A0 sheet scanned at
/// 600 dpi does not reach. A stream states its own size, and a hostile one states any.
const MAX_JBIG2_PIXELS: u64 = 1 << 30;

/// Collects decoded JBIG2 rows as 8-bit gray pixels.
struct Jbig2Rows {
  width: usize,
  ink: u8,
  paper: u8,
  /// Where the row being filled starts in `pixels`.
  row: usize,
  pixels: Vec<u8>,
}

impl Jbig2Rows {
  fn push(&mut self, black: bool, count: usize) {
    let value = if black { self.ink } else { self.paper };
    let end = self
      .pixels
      .len()
      .saturating_add(count)
      .min(self.row.saturating_add(self.width));
    self.pixels.resize(end, value);
  }
}

impl hayro_jbig2::Decoder for Jbig2Rows {
  fn push_pixel(&mut self, black: bool) {
    self.push(black, 1);
  }

  fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
    let pixels = usize::try_from(chunk_count)
      .unwrap_or(usize::MAX)
      .saturating_mul(8);
    self.push(black, pixels);
  }

  fn next_line(&mut self) {
    // A row the decoder left short is paper to its end.
    self.row = self.row.saturating_add(self.width);
    self.pixels.resize(self.row, self.paper);
  }
}

/// Decode a JBIG2 image (the bi-level coding scanners keep text in) into an 8-bit gray image:
/// its ink black on white paper, unless `/Decode` swaps the two. A stencil mask comes out the
/// same way, as what it paints.
fn decode_jbig2(content: &[u8], width: u32, height: u32, p: &Jbig2Params) -> Option<DynamicImage> {
  use hayro_jbig2::{DecodeSettings, Globals, Page};

  let settings = DecodeSettings::default();
  let globals = match &p.globals {
    Some(bytes) => Some(Globals::new(bytes, &settings).ok()?),
    None => None,
  };
  let page = Page::new_embedded(content, globals.as_ref(), &settings).ok()?;
  let (w, h) = (page.width(), page.height());
  let area = u64::from(w).checked_mul(u64::from(h))?;
  if area == 0 || area > MAX_JBIG2_PIXELS {
    return None;
  }
  // The filter's output has a black pixel as 0, which a gray image shows black and a stencil
  // mask paints.
  let (ink, paper) = if p.inverted {
    (255u8, 0u8)
  } else {
    (0u8, 255u8)
  };
  let mut rows = Jbig2Rows {
    width: usize::try_from(w).ok()?,
    ink,
    paper,
    row: 0,
    pixels: Vec::new(),
  };
  page.decode(&mut rows).ok()?;
  rows.pixels.resize(usize::try_from(area).ok()?, paper);
  let img = DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, rows.pixels)?);
  // The stream's own size is authoritative for its bits; resample only if the dictionary
  // disagrees, so that the picture is the size a caller is told.
  Some(if width > 0 && height > 0 && (width, height) != (w, h) {
    img.resize_exact(width, height, image::imageops::FilterType::Nearest)
  } else {
    img
  })
}

/// `/DecodeParms` of a `CCITTFaxDecode` image plus the `/Decode` inversion
/// (ISO 32000-1 §7.4.6, table 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CcittParams {
  /// `/K`: < 0 pure Group 4, 0 pure Group 3 1-D, > 0 mixed Group 3 2-D.
  pub k: i64,
  pub columns: u32,
  pub rows: u32,
  pub black_is_1: bool,
  pub encoded_byte_align: bool,
  pub end_of_line: bool,
  pub end_of_block: bool,
  /// `/Decode [1 0]` on the image dictionary.
  pub inverted: bool,
}

fn ccitt_params(
  doc: &Document,
  dict: &lopdf::Dictionary,
  filter: &str,
  width: u32,
  height: u32,
) -> Option<CcittParams> {
  if filter != "CCITTFaxDecode" {
    return None;
  }
  let parms = resolve_decode_parms(doc, dict).unwrap_or_default();
  let flag = |key: &[u8], default: bool| match parms.get(key) {
    Ok(Object::Boolean(b)) => *b,
    _ => default,
  };
  let inverted = decode_is_inverted(dict);
  Some(CcittParams {
    k: get_dict_int(&parms, b"K").unwrap_or(0),
    columns: get_dict_int(&parms, b"Columns")
      .filter(|c| *c > 0)
      .map(|c| c as u32)
      .unwrap_or(if width > 0 { width } else { 1728 }),
    rows: get_dict_int(&parms, b"Rows")
      .filter(|r| *r > 0)
      .map(|r| r as u32)
      .unwrap_or(height),
    black_is_1: flag(b"BlackIs1", false),
    encoded_byte_align: flag(b"EncodedByteAlign", false),
    end_of_line: flag(b"EndOfLine", false),
    end_of_block: flag(b"EndOfBlock", true),
    inverted,
  })
}

/// Collects decoded CCITT rows as 8-bit gray pixels.
struct CcittRows {
  width: usize,
  black: u8,
  white: u8,
  pixels: Vec<u8>,
}

impl hayro_ccitt::Decoder for CcittRows {
  fn push_row(&mut self, changes: &[u32]) {
    let start = self.pixels.len();
    self.pixels.resize(start + self.width, self.white);
    let row = &mut self.pixels[start..];
    for pair in changes.chunks(2) {
      let a = (pair[0] as usize).min(self.width);
      let b = pair
        .get(1)
        .map_or(self.width, |c| (*c as usize).min(self.width));
      row[a..b].fill(self.black);
    }
  }
}

/// Decode a Group 3 / Group 4 fax image (the format copiers and fax gateways
/// scan to) into an 8-bit gray image. A truncated stream yields the rows that
/// did decode, padded with white to the declared height.
fn decode_ccitt(content: &[u8], width: u32, height: u32, p: CcittParams) -> Option<DynamicImage> {
  use hayro_ccitt::{DecodeSettings, DecoderContext, EncodingMode};

  let encoding = if p.k < 0 {
    EncodingMode::Group4
  } else if p.k == 0 {
    EncodingMode::Group3_1D
  } else {
    EncodingMode::Group3_2D
  };
  let settings = DecodeSettings {
    columns: p.columns,
    rows: p.rows,
    end_of_block: p.end_of_block,
    end_of_line: p.end_of_line,
    rows_are_byte_aligned: p.encoded_byte_align,
    encoding,
    strict: false,
  };
  // The filter emits black as 1 only with /BlackIs1; DeviceGray then shows a 1
  // bit as white unless /Decode [1 0] flips it back.
  let black_bit_is_white = p.black_is_1 != p.inverted;
  let (black, white) = if black_bit_is_white {
    (255u8, 0u8)
  } else {
    (0u8, 255u8)
  };
  let mut sink = CcittRows {
    width: p.columns as usize,
    black,
    white,
    pixels: Vec::with_capacity(p.columns as usize * height as usize),
  };
  let mut ctx = DecoderContext::new(settings);
  // Errors mean a malformed tail; keep whatever rows came through.
  let _ = hayro_ccitt::decode(content, &mut sink, &mut ctx);
  let rows_decoded = sink.pixels.len() / sink.width.max(1);
  if rows_decoded == 0 {
    return None;
  }
  let (w, h) = (p.columns, height.max(1));
  sink.pixels.resize(w as usize * h as usize, white);
  let gray = image::GrayImage::from_raw(w, h, sink.pixels)?;
  let img = DynamicImage::ImageLuma8(gray);
  // /Columns is authoritative for the bitstream; resample only if /Width disagrees.
  Some(if width > 0 && width != w {
    img.resize_exact(width, h, image::imageops::FilterType::Nearest)
  } else {
    img
  })
}

/// Decode an XObject stream into a DynamicImage (shared by PNG export and OCR).
fn decode_xobject_to_dynamic_image(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  codec: &Codec,
  smask: Option<&[u8]>,
) -> Option<DynamicImage> {
  let dynamic_img = match codec {
    Codec::Jpeg => image::load_from_memory_with_format(content, ImageFormat::Jpeg).ok()?,
    Codec::Jpx => decode_jpx(content)?,
    Codec::Ccitt(p) => decode_ccitt(content, width, height, *p)?,
    Codec::Jbig2(p) => decode_jbig2(content, width, height, p)?,
    Codec::Raw => decode_raw_pixels(content, width, height, bpc, color_space)?,
  };

  Some(if let Some(mask_data) = smask {
    apply_smask(dynamic_img, mask_data, width, height)
  } else {
    dynamic_img
  })
}

fn encode_to_png(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  codec: &Codec,
  smask: Option<&[u8]>,
) -> Option<Vec<u8>> {
  let final_img =
    decode_xobject_to_dynamic_image(content, width, height, bpc, color_space, codec, smask)?;
  let mut png_buf = Cursor::new(Vec::new());
  final_img.write_to(&mut png_buf, ImageFormat::Png).ok()?;
  Some(png_buf.into_inner())
}

/// Decode a JPEG 2000 (JPXDecode) stream using hayro-jpeg2000 (pure Rust)
fn decode_jpx(content: &[u8]) -> Option<DynamicImage> {
  let jp2_img =
    hayro_jpeg2000::Image::new(content, &hayro_jpeg2000::DecodeSettings::default()).ok()?;
  DynamicImage::from_decoder(jp2_img).ok()
}

/// Decode raw pixel data (FlateDecode / uncompressed) into a DynamicImage
fn decode_raw_pixels(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
) -> Option<DynamicImage> {
  let channels: u32 = match color_space {
    "DeviceRGB" | "ICCBased3" | "CalRGB" => 3,
    "DeviceGray" | "ICCBased1" | "CalGray" => 1,
    "DeviceCMYK" | "ICCBased4" => 4,
    _ => 3,
  };
  let bytes_per_sample = if bpc > 8 { 2u32 } else { 1u32 };
  let expected = (width * height * channels * bytes_per_sample) as usize;

  // Validate buffer size before constructing image
  if content.len() < expected {
    return None;
  }
  // Use exactly the expected number of bytes
  let pixel_data = &content[..expected];

  // Downscale 16-bit to 8-bit if needed
  let pixel_data_8bit: Vec<u8> = if bytes_per_sample == 2 {
    let (pairs, _) = pixel_data.as_chunks::<2>();
    pairs
      .iter()
      .map(|pair| (u16::from_be_bytes(*pair) >> 8) as u8)
      .collect()
  } else {
    pixel_data.to_vec()
  };

  match color_space {
    "DeviceRGB" | "ICCBased3" | "CalRGB" => {
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    "DeviceGray" | "ICCBased1" | "CalGray" => {
      let img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageLuma8(img))
    }
    "DeviceCMYK" | "ICCBased4" => {
      let rgb_bytes = cmyk_to_rgb(&pixel_data_8bit);
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb_bytes)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    _ => {
      let expected_rgb = (width * height * 3) as usize;
      if pixel_data_8bit.len() == expected_rgb {
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
          ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
        Some(DynamicImage::ImageRgb8(img))
      } else {
        None
      }
    }
  }
}

/// Combine a base RGB image with a grayscale SMask to produce an RGBA PNG
fn apply_smask(base: DynamicImage, mask_data: &[u8], width: u32, height: u32) -> DynamicImage {
  let rgb = base.to_rgb8();
  let expected_mask_len = (width * height) as usize;

  if mask_data.len() < expected_mask_len {
    return DynamicImage::ImageRgb8(rgb);
  }

  let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
  for (rgb_pixel, &alpha) in rgb.pixels().zip(mask_data.iter()) {
    rgba_pixels.push(rgb_pixel[0]);
    rgba_pixels.push(rgb_pixel[1]);
    rgba_pixels.push(rgb_pixel[2]);
    rgba_pixels.push(alpha);
  }

  match ImageBuffer::from_raw(width, height, rgba_pixels) {
    Some(img) => DynamicImage::ImageRgba8(img),
    None => DynamicImage::ImageRgb8(rgb),
  }
}

fn cmyk_to_rgb(cmyk: &[u8]) -> Vec<u8> {
  let pixel_count = cmyk.len() / 4;
  let mut rgb = Vec::with_capacity(pixel_count * 3);

  for i in 0..pixel_count {
    let c = cmyk[i * 4] as f32 / 255.0;
    let m = cmyk[i * 4 + 1] as f32 / 255.0;
    let y = cmyk[i * 4 + 2] as f32 / 255.0;
    let k = cmyk[i * 4 + 3] as f32 / 255.0;

    let r = 255.0 * (1.0 - c) * (1.0 - k);
    let g = 255.0 * (1.0 - m) * (1.0 - k);
    let b = 255.0 * (1.0 - y) * (1.0 - k);

    rgb.push(r as u8);
    rgb.push(g as u8);
    rgb.push(b as u8);
  }

  rgb
}

#[cfg(test)]
mod ccitt_tests;

#[cfg(test)]
mod jbig2_tests;
