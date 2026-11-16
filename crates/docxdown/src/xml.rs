//! Reading WordprocessingML out of a parsed part: which vocabulary an element belongs to, its
//! attributes, and the two value forms that recur everywhere (on/off, and twentieths of a point).

use roxmltree::Node;

const WML: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// The same vocabulary as a Strict document names it.
const WML_STRICT: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
const MARKUP_COMPATIBILITY: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const RELATIONSHIPS_STRICT: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";

fn is_wml(namespace: Option<&str>) -> bool {
  matches!(namespace, Some(WML | WML_STRICT))
}

/// The local name of a WordprocessingML element; `None` for anything else.
pub(crate) fn wml<'a>(node: Node<'a, 'a>) -> Option<&'a str> {
  let tag = node.tag_name();
  (node.is_element() && is_wml(tag.namespace())).then(|| tag.name())
}

/// The local name of an element of any vocabulary: drawings mix several, and only the name
/// matters there.
pub(crate) fn local<'a>(node: Node<'a, 'a>) -> Option<&'a str> {
  node.is_element().then(|| node.tag_name().name())
}

/// Whether `node` is the markup-compatibility element `name` (`AlternateContent`, `Choice`,
/// `Fallback`).
pub(crate) fn is_mc(node: Node<'_, '_>, name: &str) -> bool {
  let tag = node.tag_name();
  node.is_element() && tag.namespace() == Some(MARKUP_COMPATIBILITY) && tag.name() == name
}

/// A `w:` attribute.
pub(crate) fn attr<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
  node
    .attributes()
    .find(|a| a.name() == name && is_wml(a.namespace()))
    .map(|a| a.value())
}

/// An `r:` attribute: the id of a relationship.
pub(crate) fn rel_attr<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
  node
    .attributes()
    .find(|a| {
      a.name() == name && matches!(a.namespace(), Some(RELATIONSHIPS | RELATIONSHIPS_STRICT))
    })
    .map(|a| a.value())
}

/// An attribute with no namespace, as drawing elements write theirs.
pub(crate) fn plain_attr<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
  node
    .attributes()
    .find(|a| a.name() == name && a.namespace().is_none())
    .map(|a| a.value())
}

/// `w:val`.
pub(crate) fn val<'a>(node: Node<'a, 'a>) -> Option<&'a str> {
  attr(node, "val")
}

/// The first WordprocessingML child named `name`.
pub(crate) fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
  node.children().find(|c| wml(*c) == Some(name))
}

/// The first descendant, of any vocabulary, whose local name is `name`.
pub(crate) fn descendant<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
  node.descendants().find(|d| local(*d) == Some(name))
}

/// An on/off property: present with no value is on; `0`, `false` and `off` are off.
pub(crate) fn on_off(property: Option<Node<'_, '_>>) -> Option<bool> {
  property.map(|node| !matches!(val(node), Some("0" | "false" | "off")))
}

/// A whole number in an attribute; `None` when it is absent or not a number.
pub(crate) fn number<T: std::str::FromStr>(value: Option<&str>) -> Option<T> {
  value.and_then(|v| v.trim().parse().ok())
}

/// A length in twentieths of a point, as points.
pub(crate) fn twips(value: Option<&str>) -> Option<f64> {
  number::<f64>(value).map(|t| t / 20.0)
}

/// A length in English Metric Units, as points: 12,700 to the point.
pub(crate) fn emu(value: Option<&str>) -> Option<f64> {
  number::<f64>(value).map(|e| e / 12_700.0)
}
