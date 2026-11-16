use super::*;
use lopdf::{Dictionary, Stream, dictionary};

const W: u8 = 255;
const B: u8 = 0;

/// The picture every test here decodes: 8 pixels wide and 2 tall, a run of three black pixels on
/// the first row and nothing on the second. It is the bitmap the Group 4 test in `ccitt_tests`
/// uses.
const INK_BLACK: [u8; 16] = [W, W, B, B, B, W, W, W, W, W, W, W, W, W, W, W];
const INK_WHITE: [u8; 16] = [B, B, W, W, W, B, B, B, B, B, B, B, B, B, B, B];

/// That picture as an embedded JBIG2 stream, written by hand (ITU-T T.88): a page information
/// segment, then one immediate generic region whose bitmap is coded the way a Group 4 fax is.
fn jbig2_page() -> Vec<u8> {
  let mut data = Vec::new();
  // Segment 0: page information (type 48) for page 1, with 19 bytes of data.
  data.extend_from_slice(&[0, 0, 0, 0, 0x30, 0x00, 0x01, 0, 0, 0, 19]);
  // Width 8, height 2, no resolution stated, no flags, no striping.
  data.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0, 0]);
  // Segment 1: immediate generic region (type 38) on page 1, with 23 bytes of data.
  data.extend_from_slice(&[0, 0, 0, 1, 0x26, 0x00, 0x01, 0, 0, 0, 23]);
  // The region: 8 by 2 at the page's corner, combined by OR.
  data.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0x00]);
  // Coded as MMR, which is fax Group 4.
  data.push(0x01);
  // 001 0111 10 1 | 0001 1 | 000000000001 000000000001
  // Row 1: horizontal mode, white 2, black 3, then vertical 0 to the end of the row.
  // Row 2: pass mode over the row above, then vertical 0. Then the end-of-block code.
  data.extend_from_slice(&[0x2F, 0x46, 0x00, 0x20, 0x02]);
  data
}

/// What a JBIG2 image's dictionary says when it is an ordinary one-bit gray image.
fn gray_image() -> Dictionary {
  dictionary! {
    "Type" => "XObject",
    "Subtype" => "Image",
    "Width" => 8,
    "Height" => 2,
    "BitsPerComponent" => 1,
    "ColorSpace" => "DeviceGray",
    "Filter" => "JBIG2Decode",
  }
}

/// Adds a page that paints `image` to `doc`, and returns the page.
fn paint(doc: &mut Document, image: Stream) -> ObjectId {
  let image_id = doc.add_object(image);
  let content_id = doc.add_object(Stream::new(
    dictionary! {},
    b"q 8 0 0 2 0 0 cm /Im0 Do Q".to_vec(),
  ));
  let pages_id = doc.new_object_id();
  let page_id = doc.add_object(dictionary! {
    "Type" => "Page",
    "Parent" => Object::Reference(pages_id),
    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    "Contents" => Object::Reference(content_id),
    "Resources" => Object::Dictionary(dictionary! {
      "XObject" => Object::Dictionary(dictionary! { "Im0" => Object::Reference(image_id) }),
    }),
  });
  doc.objects.insert(
    pages_id,
    Object::Dictionary(
      dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 },
    ),
  );
  let catalog =
    doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
  doc.trailer.set("Root", Object::Reference(catalog));
  page_id
}

/// A one-page document painting one image with this dictionary and these bytes.
fn document(image: Dictionary, content: Vec<u8>) -> (Document, ObjectId) {
  let mut doc = Document::with_version("1.7");
  let page_id = paint(&mut doc, Stream::new(image, content));
  (doc, page_id)
}

/// The gray pixels of a PNG, row by row.
fn pixels(png: &[u8]) -> Vec<u8> {
  image::load_from_memory(png)
    .expect("a PNG")
    .to_luma8()
    .into_raw()
}

/// The one image a document's pages hold.
fn only_image(doc: &Document) -> PageImage {
  let mut images = extract_images_raw(doc);
  assert_eq!(images.len(), 1, "one image was expected");
  images.remove(0)
}

#[test]
fn a_jbig2_image_is_returned_as_a_png_with_its_ink_black() {
  let (doc, _) = document(gray_image(), jbig2_page());
  let image = only_image(&doc);
  assert_eq!((image.page, image.image_index), (1, 0));
  assert_eq!((image.width, image.height), (8, 2));
  assert_eq!(image.filter, "JBIG2Decode");
  assert_eq!(
    (image.color_space.as_str(), image.bits_per_component),
    ("DeviceGray", 1)
  );
  assert_eq!(pixels(&image.data), INK_BLACK);
}

#[test]
fn the_ocr_tier_is_handed_the_decoded_raster() {
  let (doc, page_id) = document(gray_image(), jbig2_page());
  let rasters = collect_page_decoded_images_named(&doc, page_id);
  let [(name, raster)] = rasters.as_slice() else {
    panic!("one raster was expected, found {}", rasters.len());
  };
  assert_eq!(name, "Im0");
  assert_eq!((raster.width(), raster.height()), (8, 2));
  assert_eq!(raster.to_luma8().into_raw(), INK_BLACK);
}

#[test]
fn a_decode_array_of_one_zero_swaps_ink_and_paper() {
  let mut image = gray_image();
  image.set("Decode", vec![1.into(), 0.into()]);
  let (doc, _) = document(image, jbig2_page());
  assert_eq!(pixels(&only_image(&doc).data), INK_WHITE);
}

#[test]
fn a_stencil_mask_is_its_ink_on_paper_and_is_reported_as_gray() {
  // A mask states no color space: it is painted in whatever the fill color is. Most scans that
  // keep their text as JBIG2 keep it as masks.
  let mut image = gray_image();
  image.remove(b"ColorSpace");
  image.set("ImageMask", true);
  let (doc, page_id) = document(image, jbig2_page());
  let found = only_image(&doc);
  assert_eq!(found.color_space, "DeviceGray");
  assert_eq!(pixels(&found.data), INK_BLACK);
  assert_eq!(collect_page_decoded_images(&doc, page_id).len(), 1);
}

#[test]
fn an_image_behind_a_transport_filter_is_unwrapped_first() {
  let mut image = gray_image();
  image.set(
    "Filter",
    vec![
      Object::Name(b"ASCIIHexDecode".to_vec()),
      Object::Name(b"JBIG2Decode".to_vec()),
    ],
  );
  let hex: String = jbig2_page()
    .iter()
    .map(|byte| format!("{byte:02X}"))
    .collect();
  let (doc, _) = document(image, format!("{hex}>").into_bytes());
  let found = only_image(&doc);
  assert_eq!(found.filter, "JBIG2Decode");
  assert_eq!(pixels(&found.data), INK_BLACK);
}

/// Shared segments, as a PDF keeps them in a `JBIG2Globals` stream: one pattern dictionary
/// (segment 0, type 16) holding two patterns of a single pixel each, white and then black.
fn jbig2_globals() -> Vec<u8> {
  let mut data = Vec::new();
  // Segment 0: pattern dictionary, on no page, with 11 bytes of data.
  data.extend_from_slice(&[0, 0, 0, 0, 0x10, 0x00, 0x00, 0, 0, 0, 11]);
  // Coded as MMR; patterns 1 by 1; the largest gray value is 1, so there are two of them.
  data.extend_from_slice(&[0x01, 1, 1, 0, 0, 0, 1]);
  // The two patterns side by side, one row "WB": 010 1 | 000000000001 000000000001
  // Vertical mode one to the left of the row's end, then vertical 0, then end of block.
  data.extend_from_slice(&[0x50, 0x01, 0x00, 0x10]);
  data
}

/// The same picture as [`jbig2_page`], drawn another way: a halftone region (segment 2, type 22)
/// that lays the patterns of segment 0 on a grid one pixel apart. Which pattern goes where is
/// the bitmap itself: black where the black pattern goes. It cannot be decoded without the
/// globals, which hold the patterns.
fn jbig2_halftone_page() -> Vec<u8> {
  let mut data = Vec::new();
  // Segment 1: page information for page 1, 8 by 2.
  data.extend_from_slice(&[0, 0, 0, 1, 0x30, 0x00, 0x01, 0, 0, 0, 19]);
  data.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0, 0]);
  // Segment 2: immediate halftone region on page 1, referring to one segment, number 0, with
  // 43 bytes of data.
  data.extend_from_slice(&[0, 0, 0, 2, 0x16, 0x20, 0x00, 0x01, 0, 0, 0, 43]);
  // The region: 8 by 2 at the page's corner, combined by OR.
  data.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0x00]);
  // Coded as MMR. A grid 8 wide and 2 tall starting at the corner, each step 256/256 of a
  // pixel across and none down.
  data.push(0x01);
  data.extend_from_slice(&[
    0, 0, 0, 8, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x00, 0x00, 0x00,
  ]);
  // One bit plane of pattern numbers: the bitmap of `jbig2_page`.
  data.extend_from_slice(&[0x2F, 0x46, 0x00, 0x20, 0x02]);
  data
}

/// A document whose image is the halftone page and names `globals` in its decode parameters.
fn with_globals(globals: Stream) -> Document {
  let mut doc = Document::with_version("1.7");
  let globals_id = doc.add_object(globals);
  let mut image = gray_image();
  image.set(
    "DecodeParms",
    Object::Dictionary(dictionary! { "JBIG2Globals" => Object::Reference(globals_id) }),
  );
  paint(&mut doc, Stream::new(image, jbig2_halftone_page()));
  doc
}

#[test]
fn a_page_is_decoded_with_the_globals_its_image_names() {
  let doc = with_globals(Stream::new(dictionary! {}, jbig2_globals()));
  assert_eq!(pixels(&only_image(&doc).data), INK_BLACK);
}

#[test]
fn shared_globals_are_read_through_their_own_filter() {
  // The globals written as hexadecimal text, a filter of the stream that has to be undone
  // before the bytes mean anything to a JBIG2 decoder.
  let hex: String = jbig2_globals()
    .iter()
    .map(|byte| format!("{byte:02X}"))
    .collect();
  let globals = Stream::new(
    dictionary! { "Filter" => "ASCIIHexDecode" },
    format!("{hex}>").into_bytes(),
  );
  let doc = with_globals(globals);
  assert_eq!(pixels(&only_image(&doc).data), INK_BLACK);
}

#[test]
fn a_stream_that_is_not_jbig2_is_left_out() {
  let (doc, page_id) = document(gray_image(), b"not a JBIG2 stream".to_vec());
  assert_eq!(extract_images_raw(&doc).len(), 0);
  assert_eq!(collect_page_decoded_images(&doc, page_id).len(), 0);
}

#[test]
fn a_stream_whose_size_differs_from_the_dictionary_is_scaled_to_the_dictionary() {
  // The dictionary's size is what a caller is told, so the picture is made to match it: here
  // twice as wide and twice as tall as the stream's own 8 by 2.
  let mut image = gray_image();
  image.set("Width", 16);
  image.set("Height", 4);
  let (doc, _) = document(image, jbig2_page());
  let found = only_image(&doc);
  assert_eq!((found.width, found.height), (16, 4));
  let wide = [W, W, W, W, B, B, B, B, B, B, W, W, W, W, W, W];
  let mut expected = Vec::new();
  expected.extend_from_slice(&wide);
  expected.extend_from_slice(&wide);
  expected.extend_from_slice(&[W; 32]);
  assert_eq!(pixels(&found.data), expected);
}

#[test]
fn a_page_too_large_to_hold_is_left_out() {
  // A page information segment alone, claiming 65,536 pixels each way: four gigapixels.
  let mut huge = vec![0, 0, 0, 0, 0x30, 0x00, 0x01, 0, 0, 0, 19];
  huge.extend_from_slice(&[0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0, 0]);
  let (doc, page_id) = document(gray_image(), huge);
  assert_eq!(extract_images_raw(&doc).len(), 0);
  assert_eq!(collect_page_decoded_images(&doc, page_id).len(), 0);
}
