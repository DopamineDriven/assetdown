//! What can go wrong with a compound file.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfbError {
  /// The bytes do not begin as a compound file does.
  NotCompound,
  /// The bytes begin as one, and its header, its tables, or its directory cannot be read.
  Malformed(&'static str),
  /// What was to be written is not something the format holds.
  Unwritable(&'static str),
}

impl fmt::Display for CfbError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CfbError::NotCompound => f.write_str("not a compound file"),
      CfbError::Malformed(what) => write!(f, "malformed compound file: {what}"),
      CfbError::Unwritable(what) => write!(f, "not writable as a compound file: {what}"),
    }
  }
}

impl std::error::Error for CfbError {}
