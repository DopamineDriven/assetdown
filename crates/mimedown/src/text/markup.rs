//! The root element of a piece of markup, found without reading the rest of it. The head of a
//! file is not whole XML, so this is a scanner that stops at the first start tag, past a
//! declaration, comments and a document type.

use crate::media_type::MediaType;

/// The first element of a piece of markup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Root {
  /// The element's name, without a namespace prefix.
  pub name: String,
  /// Its attributes, in order, each a name and a value.
  pub attributes: Vec<(String, String)>,
  /// The name a document type declaration gives, when there is one.
  pub doctype: Option<String>,
  /// The text opens with an XML declaration.
  pub declared_xml: bool,
}

impl Root {
  pub(crate) fn attribute(&self, name: &str) -> Option<&str> {
    self
      .attributes
      .iter()
      .find(|(known, _)| known == name)
      .map(|(_, value)| value.as_str())
  }

  /// The type a root makes its file. A root with no type of its own is XML when the text says
  /// it is XML, and nothing in particular when it does not.
  pub(crate) fn media_type(&self) -> Option<MediaType> {
    let html = self.name.eq_ignore_ascii_case("html")
      || self
        .doctype
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("html"));
    let xhtml = self.attribute("xmlns") == Some("http://www.w3.org/1999/xhtml");
    Some(match self.name.as_str() {
      // A page is XHTML when it opens as XML does and names XHTML's namespace. The namespace
      // and the doctype alone are on a great many pages that are served as HTML and do not
      // parse as XML; called XHTML, a browser reads one with its XML parser and shows nothing.
      _ if html && self.declared_xml && xhtml => MediaType::Xhtml,
      _ if html => MediaType::Html,
      "svg" => MediaType::Svg,
      "COLLADA" => MediaType::Collada,
      "X3D" => MediaType::X3d,
      "MPD" => MediaType::DashManifest,
      "abiword" => MediaType::Abw,
      _ if self.declared_xml
        || self
          .attributes
          .iter()
          .any(|(name, _)| name.starts_with("xmlns")) =>
      {
        MediaType::Xml
      }
      _ => return None,
    })
  }
}

/// What a text opens with, read as markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Opening {
  /// Its first start tag, all there.
  Root(Root),
  /// It opens as markup does and stops before its first start tag closes. More of the text
  /// could still say what it is.
  Unfinished,
  /// It is not markup, however much more of it there is.
  NotMarkup,
}

pub(crate) fn opening(text: &str) -> Opening {
  let mut root = Root::default();
  let mut rest = text.trim_start();
  // Whether anything that stands in front of a root was read: a declaration, a comment.
  let mut begun = false;
  loop {
    if rest.starts_with("<?xml") {
      let Some(after) = past(rest, "<?xml", "?>") else {
        return Opening::Unfinished;
      };
      root.declared_xml = true;
      rest = after.trim_start();
    } else if rest.starts_with("<?") {
      // An instruction that is never closed is how a script opens, and not markup.
      let Some(after) = past(rest, "<?", "?>") else {
        return Opening::NotMarkup;
      };
      rest = after.trim_start();
    } else if rest.starts_with("<!--") {
      let Some(after) = past(rest, "<!--", "-->") else {
        return Opening::Unfinished;
      };
      rest = after.trim_start();
    } else if rest
      .get(..9)
      .is_some_and(|opening| opening.eq_ignore_ascii_case("<!DOCTYPE"))
    {
      let declaration = &rest[9..];
      root.doctype = declaration
        .split(|c: char| c.is_whitespace() || c == '>' || c == '[')
        .find(|word| !word.is_empty())
        .map(str::to_string);
      // A document type may hold declarations of its own in brackets, with `>` inside them.
      let end = match (declaration.find('['), declaration.find('>')) {
        (Some(open), Some(close)) if open < close => match declaration[open..].find("]>") {
          Some(closed) => closed + open + 2,
          None => return Opening::Unfinished,
        },
        (_, Some(close)) => close + 1,
        _ => return Opening::Unfinished,
      };
      rest = declaration[end..].trim_start();
    } else {
      break;
    }
    begun = true;
  }
  let begins_a_name = |name: &str| {
    name
      .chars()
      .next()
      .is_some_and(|c| c.is_alphabetic() || c == '_')
  };
  let Some(tag) = rest.strip_prefix('<') else {
    return match rest.is_empty() && begun {
      true => Opening::Unfinished,
      false => Opening::NotMarkup,
    };
  };
  let Some(name_end) = tag.find(|c: char| c.is_whitespace() || c == '>' || c == '/') else {
    // The text stops inside the tag's name, or inside a `<!` that had not said what it was.
    return match tag.is_empty() || tag.starts_with('!') || begins_a_name(tag) {
      true => Opening::Unfinished,
      false => Opening::NotMarkup,
    };
  };
  let name = &tag[..name_end];
  if !begins_a_name(name) {
    return Opening::NotMarkup;
  }
  root.name = name.rsplit(':').next().unwrap_or(name).to_string();

  // The attributes, up to the `>` that closes the tag. A value stands in quotes, where a `>`
  // is part of it, or bare, where it runs to the next space; HTML also allows a name alone.
  let mut rest = tag[name_end..].trim_start();
  loop {
    if rest.starts_with('>') || rest.starts_with("/>") {
      return Opening::Root(root);
    }
    if let Some(after) = rest.strip_prefix('/') {
      rest = after.trim_start();
      continue;
    }
    let Some(name_end) = rest.find(|c: char| c.is_whitespace() || c == '=' || c == '>' || c == '/')
    else {
      return Opening::Unfinished;
    };
    let name = &rest[..name_end];
    if name.is_empty() {
      return Opening::NotMarkup;
    }
    let after = rest[name_end..].trim_start();
    let Some(value) = after.strip_prefix('=') else {
      root.attributes.push((name.to_string(), String::new()));
      rest = after;
      continue;
    };
    let value = value.trim_start();
    let (value, after) = match value.chars().next() {
      Some(quote @ ('"' | '\'')) => match value[1..].find(quote) {
        Some(close) => (&value[1..=close], &value[close + 2..]),
        None => return Opening::Unfinished,
      },
      Some(_) => match value.find(|c: char| c.is_whitespace() || c == '>') {
        Some(end) => (&value[..end], &value[end..]),
        None => return Opening::Unfinished,
      },
      None => return Opening::Unfinished,
    };
    root.attributes.push((name.to_string(), value.to_string()));
    rest = after.trim_start();
  }
}

/// What follows `opening` in `text`, up to and past `closing`.
fn past<'a>(text: &'a str, opening: &str, closing: &str) -> Option<&'a str> {
  let rest = text.strip_prefix(opening)?;
  Some(&rest[rest.find(closing)? + closing.len()..])
}

/// The root of the markup `text` begins with. None when it is not markup, or when its first
/// start tag is not all there.
pub(crate) fn root(text: &str) -> Option<Root> {
  match opening(text) {
    Opening::Root(root) => Some(root),
    Opening::Unfinished | Opening::NotMarkup => None,
  }
}
