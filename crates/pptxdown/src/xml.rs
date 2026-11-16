//! Reading PresentationML and DrawingML out of a parsed part.
//!
//! A slide mixes several vocabularies (`p:` for the slide, `a:` for drawing, `dsp:` and `c:` in
//! the parts SmartArt and charts keep), and the same structure recurs under more than one of
//! them: a text body is `p:txBody` in a shape, `a:txBody` in a table cell and `dsp:txBody` in a
//! diagram. So elements are told apart by their local names, and attributes, which these
//! vocabularies write without a namespace, by their plain names.

use roxmltree::Node;

const MARKUP_COMPATIBILITY: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const RELATIONSHIPS_STRICT: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";

/// English Metric Units to the point.
const EMU_PER_POINT: f64 = 12_700.0;

/// The local name of an element; `None` for text and comments.
pub(crate) fn name<'a>(node: Node<'a, 'a>) -> Option<&'a str> {
  node.is_element().then(|| node.tag_name().name())
}

pub(crate) fn is(node: Node<'_, '_>, local: &str) -> bool {
  name(node) == Some(local)
}

pub(crate) fn elements<'a>(parent: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
  parent.children().filter(|n| n.is_element())
}

/// The first child element named `local`.
pub(crate) fn child<'a>(node: Node<'a, 'a>, local: &str) -> Option<Node<'a, 'a>> {
  node.children().find(|c| is(*c, local))
}

/// The element reached by following `path` one child at a time.
pub(crate) fn path<'a>(node: Node<'a, 'a>, path: &[&str]) -> Option<Node<'a, 'a>> {
  path.iter().try_fold(node, |at, local| child(at, local))
}

/// The first descendant element named `local`.
pub(crate) fn descendant<'a>(node: Node<'a, 'a>, local: &str) -> Option<Node<'a, 'a>> {
  node.descendants().find(|d| is(*d, local))
}

/// An attribute written without a namespace, as these vocabularies write theirs.
pub(crate) fn attr<'a>(node: Node<'a, 'a>, local: &str) -> Option<&'a str> {
  node
    .attributes()
    .find(|a| a.name() == local && a.namespace().is_none())
    .map(|a| a.value())
}

/// An `r:` attribute: the id of a relationship.
pub(crate) fn rel_attr<'a>(node: Node<'a, 'a>, local: &str) -> Option<&'a str> {
  node
    .attributes()
    .find(|a| {
      a.name() == local && matches!(a.namespace(), Some(RELATIONSHIPS | RELATIONSHIPS_STRICT))
    })
    .map(|a| a.value())
}

/// A number in an attribute; `None` when it is absent or not a number.
pub(crate) fn number<T: std::str::FromStr>(value: Option<&str>) -> Option<T> {
  value.and_then(|v| v.trim().parse().ok())
}

/// A length in English Metric Units, as points.
pub(crate) fn emu(value: Option<&str>) -> Option<f64> {
  number::<f64>(value).map(|e| e / EMU_PER_POINT)
}

/// A yes-or-no attribute: `1` and `true` are yes, `0` and `false` no.
pub(crate) fn flag(value: Option<&str>) -> Option<bool> {
  match value? {
    "1" | "true" => Some(true),
    "0" | "false" => Some(false),
    _ => None,
  }
}

/// Whether `node` is `mc:AlternateContent`, which holds the same content more than one way.
pub(crate) fn is_alternate(node: Node<'_, '_>) -> bool {
  let tag = node.tag_name();
  node.is_element()
    && tag.namespace() == Some(MARKUP_COMPATIBILITY)
    && tag.name() == "AlternateContent"
}

/// The branch of an `mc:AlternateContent` to read: the first choice, or the fallback when there
/// is none. Reading both would repeat the content.
pub(crate) fn chosen<'a>(alternate: Node<'a, 'a>) -> Option<Node<'a, 'a>> {
  child(alternate, "Choice").or_else(|| child(alternate, "Fallback"))
}
