//! XMP: a packet of XML that travels inside an image, in which the software that made it and
//! the people who own it write what they like. A packet is whole XML, so it is parsed as XML.

use roxmltree::{Document, Node, ParsingOptions};

/// What an XMP packet says, for the fields that are worth a column or a search.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Xmp {
  /// The library that wrote the packet.
  pub toolkit: Option<String>,
  pub creator_tool: Option<String>,
  pub create_date: Option<String>,
  pub modify_date: Option<String>,
  pub metadata_date: Option<String>,
  pub document_id: Option<String>,
  pub instance_id: Option<String>,
  pub original_document_id: Option<String>,
  pub creator: Option<String>,
  pub title: Option<String>,
  pub description: Option<String>,
  pub format: Option<String>,
  pub rights: Option<String>,
  /// Photoshop's number for how color is kept: 3 for RGB, 4 for CMYK.
  pub color_mode: Option<String>,
  /// The name of the color profile, as Photoshop wrote it down.
  pub icc_profile: Option<String>,
  pub date_time_original: Option<String>,
  /// How the packet says the picture is turned, from 1 to 8. This is the packet's own word;
  /// the turn a viewer applies is EXIF's.
  pub orientation: Option<String>,
  pub producer: Option<String>,
}

const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XMP: &str = "http://ns.adobe.com/xap/1.0/";
const MEDIA: &str = "http://ns.adobe.com/xap/1.0/mm/";
const DUBLIN_CORE: &str = "http://purl.org/dc/elements/1.1/";
const PHOTOSHOP: &str = "http://ns.adobe.com/photoshop/1.0/";
const EXIF: &str = "http://ns.adobe.com/exif/1.0/";
const TIFF: &str = "http://ns.adobe.com/tiff/1.0/";
const PDF: &str = "http://ns.adobe.com/pdf/1.3/";

/// No packet has more nodes than this.
const MOST_NODES: u32 = 200_000;

/// What the packet in `bytes` says, when it is XML.
pub(crate) fn parse(bytes: &[u8]) -> Option<Xmp> {
  let text = String::from_utf8_lossy(bytes);
  // A packet is padded with spaces so that it can be rewritten in place, and some writers pad
  // with zeros.
  let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
  let options = ParsingOptions {
    nodes_limit: MOST_NODES,
    ..ParsingOptions::default()
  };
  let document = Document::parse_with_options(text, options).ok()?;
  let descriptions: Vec<Node<'_, '_>> = document
    .descendants()
    .filter(|node| {
      node.tag_name().name() == "Description" && node.tag_name().namespace() == Some(RDF)
    })
    .collect();
  // A property is an attribute of a description or an element inside one.
  let property = |namespace: &str, name: &str| {
    descriptions.iter().find_map(|description| {
      let written = description
        .attribute((namespace, name))
        .map(str::to_string)
        .or_else(|| {
          let element = description.children().find(|child| {
            child.tag_name().name() == name && child.tag_name().namespace() == Some(namespace)
          })?;
          Some(value(element))
        })?;
      let written = written.trim().to_string();
      (!written.is_empty()).then_some(written)
    })
  };
  let toolkit = document
    .descendants()
    .find(|node| node.tag_name().name() == "xmpmeta")
    .and_then(|meta| {
      meta
        .attributes()
        .find(|attribute| attribute.name() == "xmptk")
    })
    .map(|attribute| attribute.value().to_string());
  Some(Xmp {
    toolkit,
    creator_tool: property(XMP, "CreatorTool"),
    create_date: property(XMP, "CreateDate"),
    modify_date: property(XMP, "ModifyDate"),
    metadata_date: property(XMP, "MetadataDate"),
    document_id: property(MEDIA, "DocumentID"),
    instance_id: property(MEDIA, "InstanceID"),
    original_document_id: property(MEDIA, "OriginalDocumentID"),
    creator: property(DUBLIN_CORE, "creator"),
    title: property(DUBLIN_CORE, "title"),
    description: property(DUBLIN_CORE, "description"),
    format: property(DUBLIN_CORE, "format"),
    rights: property(DUBLIN_CORE, "rights"),
    color_mode: property(PHOTOSHOP, "ColorMode"),
    icc_profile: property(PHOTOSHOP, "ICCProfile"),
    date_time_original: property(EXIF, "DateTimeOriginal"),
    orientation: property(TIFF, "Orientation"),
    producer: property(PDF, "Producer"),
  })
}

/// What an element says: its own text, or the items of the list it holds, one after another.
fn value(element: Node<'_, '_>) -> String {
  let items: Vec<&str> = element
    .descendants()
    .filter(|node| node.tag_name().name() == "li" && node.tag_name().namespace() == Some(RDF))
    .filter_map(|item| item.text())
    .map(str::trim)
    .filter(|item| !item.is_empty())
    .collect();
  if items.is_empty() {
    element.text().unwrap_or_default().to_string()
  } else {
    items.join(", ")
  }
}

#[cfg(test)]
mod tests;
