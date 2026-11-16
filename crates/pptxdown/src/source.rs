//! Where a presentation's parts come from: a package, or, in tests, parts held in memory.

use std::collections::HashMap;

use ooxmldown::{Package, PackageError, Relationship, rels_path};

use crate::error::PresentationError;

pub(crate) trait Source {
  fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError>;
  /// The relationships of the part at `part`.
  fn relationships(&mut self, part: &str) -> Result<Vec<Relationship>, PackageError>;
  /// What the package says the part at `part` is: `video/mp4`.
  fn content_type(&mut self, part: &str) -> Result<Option<String>, PackageError>;
}

impl Source for Package {
  fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError> {
    Package::part(self, path)
  }

  fn relationships(&mut self, part: &str) -> Result<Vec<Relationship>, PackageError> {
    Package::relationships(self, &rels_path(part))
  }

  fn content_type(&mut self, part: &str) -> Result<Option<String>, PackageError> {
    Package::content_type(self, part)
  }
}

/// A part's relationships by id; none when it has no relationships part.
pub(crate) fn relationships_of(
  source: &mut dyn Source,
  part: &str,
) -> Result<HashMap<String, Relationship>, PresentationError> {
  match source.relationships(part) {
    Ok(relationships) => Ok(
      relationships
        .into_iter()
        .map(|relationship| (relationship.id.clone(), relationship))
        .collect(),
    ),
    Err(PackageError::Malformed(_)) => Ok(HashMap::new()),
    Err(limit) => Err(limit.into()),
  }
}

/// A part the deck can do without: `None`, with a warning, when it cannot be read. A limit
/// error is still an error.
pub(crate) fn optional_part(
  source: &mut dyn Source,
  path: &str,
  warnings: &mut Vec<String>,
) -> Result<Option<Vec<u8>>, PresentationError> {
  match source.part(path) {
    Ok(bytes) => Ok(Some(bytes)),
    Err(PackageError::Malformed(detail)) => {
      warnings.push(format!("{path} was left out: {detail}"));
      Ok(None)
    }
    Err(limit) => Err(limit.into()),
  }
}

/// The target of the first relationship of a part whose type ends with `suffix` and that stays
/// inside the package.
pub(crate) fn target_of(
  relationships: &HashMap<String, Relationship>,
  suffix: &str,
) -> Option<String> {
  let mut found: Vec<&Relationship> = relationships
    .values()
    .filter(|relationship| !relationship.external && relationship.kind.ends_with(suffix))
    .collect();
  found.sort_by(|a, b| a.target.cmp(&b.target));
  found
    .first()
    .map(|relationship| relationship.target.clone())
}
