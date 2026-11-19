use super::*;
use crate::testing::XMP_PACKET;

#[test]
fn a_property_is_read_whether_it_is_an_attribute_an_element_or_a_list() {
  let xmp = parse(XMP_PACKET.as_bytes()).unwrap();
  assert_eq!(xmp.toolkit.as_deref(), Some("Adobe XMP Core 9.1-c002"));
  assert_eq!(
    xmp.creator_tool.as_deref(),
    Some("Adobe Photoshop 26.0 (Windows)")
  );
  assert_eq!(
    xmp.create_date.as_deref(),
    Some("2026-11-17T23:36:27-05:00")
  );
  assert_eq!(
    xmp.modify_date.as_deref(),
    Some("2026-11-18T08:00:00-05:00")
  );
  assert_eq!(xmp.document_id.as_deref(), Some("xmp.did:1234"));
  assert_eq!(
    (xmp.color_mode.as_deref(), xmp.icc_profile.as_deref()),
    (Some("3"), Some("Display P3"))
  );
  assert_eq!(xmp.format.as_deref(), Some("image/jpeg"));
  assert_eq!(xmp.title.as_deref(), Some("A harbor at dusk"));
  assert_eq!(xmp.creator.as_deref(), Some("First Maker, Second Maker"));
  assert_eq!((xmp.rights, xmp.producer), (None, None));
  // What the packet says of the picture's turn is the packet's, and is kept apart from EXIF's.
  assert_eq!(xmp.orientation.as_deref(), Some("5"));
}

#[test]
fn the_padding_around_a_packet_is_not_part_of_it() {
  let padded = format!("{XMP_PACKET}{}\0\0", " ".repeat(2000));
  assert!(parse(padded.as_bytes()).is_some());
}

#[test]
fn what_is_not_xml_is_not_a_packet() {
  assert_eq!(
    parse(b"<x:xmpmeta xmlns:x='adobe:ns:meta/'><unclosed>"),
    None
  );
  assert_eq!(parse(b""), None);
}
