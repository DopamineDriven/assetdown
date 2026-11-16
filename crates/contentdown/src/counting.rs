//! Counts written the ways a list writes them: as roman numerals and as letters. Word and
//! PowerPoint number their lists by the same rules, so the format crates share them.

/// A count as a lowercase roman numeral: 1994 is `mcmxciv`. Zero is nothing.
pub fn roman(mut n: u32) -> String {
  const NUMERALS: [(u32, &str); 13] = [
    (1000, "m"),
    (900, "cm"),
    (500, "d"),
    (400, "cd"),
    (100, "c"),
    (90, "xc"),
    (50, "l"),
    (40, "xl"),
    (10, "x"),
    (9, "ix"),
    (5, "v"),
    (4, "iv"),
    (1, "i"),
  ];
  let mut out = String::new();
  for (value, numeral) in NUMERALS {
    while n >= value {
      out.push_str(numeral);
      n -= value;
    }
  }
  out
}

/// A count as lowercase letters, the way Office counts with them: a to z, then aa, bb, cc. Zero
/// is nothing.
pub fn letters(n: u32) -> String {
  let Some(before) = n.checked_sub(1) else {
    return String::new();
  };
  let letter = char::from_u32(u32::from(b'a') + before % 26).unwrap_or('a');
  let repeats = usize::try_from(before / 26)
    .unwrap_or(usize::MAX)
    .saturating_add(1);
  std::iter::repeat_n(letter, repeats).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn counts_are_written_as_numerals_and_as_letters() {
    assert_eq!(
      [1, 4, 9, 14, 40, 1994].map(roman),
      ["i", "iv", "ix", "xiv", "xl", "mcmxciv"]
    );
    assert_eq!(
      [1, 26, 27, 52, 53].map(letters),
      ["a", "z", "aa", "zz", "aaa"]
    );
    assert_eq!((roman(0), letters(0)), (String::new(), String::new()));
  }
}
