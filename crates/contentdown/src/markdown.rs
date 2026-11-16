//! Writing Markdown: what has to be escaped so that text stays text, and how emphasis, links
//! and pictures are spelled. The format crates that offer Markdown write through this, so they
//! all escape alike.

/// Where text is being written, which decides how a line break and a `|` are spelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
  Flow,
  /// A heading, which is one line and already stands out: bold adds nothing to it.
  Heading,
  /// A pipe table's cell, which must stay on one line.
  Cell,
}

/// A hard line break inside a paragraph.
pub const HARD_BREAK: &str = "\\\n";

/// The emphasis Markdown can show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Emphasis {
  pub bold: bool,
  pub italic: bool,
  pub strike: bool,
}

/// Writes `text` with whatever Markdown would read as markup escaped.
pub fn escape(text: &str, place: Place, out: &mut String) {
  let mut previous: Option<char> = None;
  let mut chars = text.chars().peekable();
  while let Some(c) = chars.next() {
    let next = chars.peek().copied();
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let special = match c {
      '\\' | '`' | '*' | '[' | ']' | '<' => true,
      '|' => place == Place::Cell,
      // An underscore inside a word is only an underscore.
      '_' => !(word(previous) && word(next)),
      '~' => previous == Some('~') || next == Some('~'),
      _ => false,
    };
    if special {
      out.push('\\');
    }
    // A line break in the text itself is white space, not a break someone asked for.
    out.push(if matches!(c, '\n' | '\r') { ' ' } else { c });
    previous = Some(c);
  }
}

/// Keeps the start of a line from reading as a heading, a quote, a list item or a rule.
pub fn guard_line(line: &str) -> String {
  let bytes = line.as_bytes();
  let ends = |at: usize| matches!(bytes.get(at), Some(b' ') | None);
  let escape_at = match bytes.first() {
    Some(b'#' | b'>') => Some(0),
    Some(b'-' | b'+') if ends(1) || (bytes.len() >= 3 && bytes.iter().all(|b| *b == b'-')) => {
      Some(0)
    }
    Some(b'0'..=b'9') => {
      let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
      (matches!(bytes.get(digits), Some(b'.' | b')')) && ends(digits + 1)).then_some(digits)
    }
    _ => None,
  };
  match escape_at {
    Some(at) => format!("{}\\{}", &line[..at], &line[at..]),
    None => line.to_owned(),
  }
}

/// White space at the start of `text`, what stands between, and white space at its end.
fn edges(text: &str) -> (&str, &str, &str) {
  let lead = &text[..text.len() - text.trim_start().len()];
  let trail = &text[text.trim_end().len()..];
  (lead, text.trim(), trail)
}

/// Writes a stretch of text with one emphasis. White space at its ends goes outside the markers,
/// where Markdown needs it.
pub fn span(emphasis: Emphasis, text: &str, place: Place, out: &mut String) {
  let (lead, core, trail) = edges(text);
  if core.is_empty() {
    out.push_str(text);
    return;
  }
  let marks = match (emphasis.bold, emphasis.italic) {
    (true, true) => "***",
    (true, false) => "**",
    (false, true) => "*",
    (false, false) => "",
  };
  let strike = if emphasis.strike { "~~" } else { "" };
  out.push_str(lead);
  out.push_str(strike);
  out.push_str(marks);
  escape(core, place, out);
  out.push_str(marks);
  out.push_str(strike);
  out.push_str(trail);
}

/// A link target or a picture's address in angle brackets, which lets it hold spaces.
fn address(target: &str, out: &mut String) {
  out.push_str("(<");
  out.push_str(&target.replace('<', "%3C").replace('>', "%3E"));
  out.push_str(">)");
}

/// Writes a link over `label`, which is Markdown already. White space at the ends of the label
/// goes outside the link; a label with nothing in it is written as it is, with no link.
pub fn link(label: &str, target: &str, out: &mut String) {
  let (lead, core, trail) = edges(label);
  if core.is_empty() {
    out.push_str(label);
    return;
  }
  out.push_str(lead);
  out.push('[');
  out.push_str(core);
  out.push(']');
  address(target, out);
  out.push_str(trail);
}

/// Writes a picture with its description.
pub fn image(alt: &str, target: &str, place: Place, out: &mut String) {
  out.push_str("![");
  escape(alt, place, out);
  out.push(']');
  address(target, out);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn escaped(text: &str, place: Place) -> String {
    let mut out = String::new();
    escape(text, place, &mut out);
    out
  }

  #[test]
  fn what_markdown_would_read_as_markup_is_escaped() {
    assert_eq!(
      escaped(
        r"2 * 3, a_b, _x_, [r], <t>, C:\d, `c`, a ~~ b, ~5",
        Place::Flow
      ),
      r"2 \* 3, a_b, \_x\_, \[r\], \<t>, C:\\d, \`c\`, a \~\~ b, ~5"
    );
    assert_eq!(escaped("a|b\nc", Place::Flow), "a|b c");
    assert_eq!(escaped("a|b", Place::Cell), r"a\|b");
  }

  #[test]
  fn the_start_of_a_line_is_kept_from_reading_as_structure() {
    let guarded: Vec<String> = [
      "# h", "> q", "- i", "+ i", "12. i", "3) i", "---", "-", "+1", "12.5", "x",
    ]
    .map(guard_line)
    .to_vec();
    assert_eq!(
      guarded,
      [
        r"\# h", r"\> q", r"\- i", r"\+ i", r"12\. i", r"3\) i", r"\---", r"\-", "+1", "12.5", "x"
      ]
    );
  }

  #[test]
  fn emphasis_links_and_pictures_keep_white_space_outside_their_markers() {
    let mut out = String::new();
    let bold = Emphasis {
      bold: true,
      ..Emphasis::default()
    };
    span(bold, " Total: ", Place::Flow, &mut out);
    span(
      Emphasis {
        bold: true,
        italic: true,
        strike: true,
      },
      "all",
      Place::Flow,
      &mut out,
    );
    span(bold, "  ", Place::Flow, &mut out);
    link(" the site ", "https://example.com/a b", &mut out);
    link("  ", "https://example.com", &mut out);
    image("A [logo]", "media/image 1.png", Place::Flow, &mut out);
    assert_eq!(
      out,
      " **Total:** ~~***all***~~   [the site](<https://example.com/a b>)   ![A \\[logo\\]](<media/image 1.png>)"
    );
  }
}
