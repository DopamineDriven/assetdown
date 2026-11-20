use super::*;

fn tokens(text: &[u8]) -> Vec<Token<'_>> {
  Tokens::over(text).collect()
}

fn word(name: &str, parameter: Option<i32>) -> Token<'_> {
  Token::Word { name, parameter }
}

#[test]
fn a_group_holds_control_words_and_text() {
  assert_eq!(
    tokens(br"{\rtf1\ansi Hello\par}"),
    [
      Token::Open,
      word("rtf", Some(1)),
      word("ansi", None),
      Token::Text(b"Hello"),
      word("par", None),
      Token::Close,
    ]
  );
}

#[test]
fn one_space_after_a_control_word_ends_it_and_any_more_are_text() {
  assert_eq!(
    tokens(br"\b  two\i0x"),
    [
      word("b", None),
      Token::Text(b" two"),
      word("i", Some(0)),
      Token::Text(b"x"),
    ]
  );
}

#[test]
fn a_number_after_a_control_word_may_be_negative() {
  assert_eq!(
    tokens(br"\li-720\fi360"),
    [word("li", Some(-720)), word("fi", Some(360))]
  );
}

#[test]
fn a_backslash_before_anything_but_a_letter_is_a_symbol() {
  assert_eq!(
    tokens(br"\{\}\\\~\*"),
    [
      Token::Symbol(b'{'),
      Token::Symbol(b'}'),
      Token::Symbol(b'\\'),
      Token::Symbol(b'~'),
      Token::Symbol(b'*'),
    ]
  );
}

#[test]
fn an_apostrophe_and_two_hex_digits_are_one_byte() {
  assert_eq!(
    tokens(br"caf\'e9\'E9"),
    [Token::Text(b"caf"), Token::Hex(0xE9), Token::Hex(0xE9)]
  );
  // Cut short, or not hex: nothing is made of it.
  assert_eq!(tokens(br"\'e"), []);
  assert_eq!(tokens(br"\'zzok"), [Token::Text(b"ok")]);
}

#[test]
fn line_ends_are_not_text() {
  assert_eq!(
    tokens(b"one\r\ntwo\n{\\b\r\n three}"),
    [
      Token::Text(b"one"),
      Token::Text(b"two"),
      Token::Open,
      word("b", None),
      Token::Text(b" three"),
      Token::Close,
    ]
  );
}

#[test]
fn bin_is_followed_by_that_many_bytes_as_they_are() {
  // Braces and backslashes inside the bytes are bytes.
  assert_eq!(
    tokens(b"{\\bin4 {\\}\x00}"),
    [Token::Open, Token::Binary(b"{\\}\x00"), Token::Close]
  );
  // A count that runs past the end takes what there is.
  assert_eq!(tokens(b"\\bin9 abc"), [Token::Binary(b"abc")]);
  assert_eq!(tokens(b"\\bin0 x"), [Token::Binary(b""), Token::Text(b"x")]);
}

#[test]
fn a_backslash_at_the_end_of_the_file_is_nothing() {
  assert_eq!(tokens(b"end\\"), [Token::Text(b"end")]);
}
