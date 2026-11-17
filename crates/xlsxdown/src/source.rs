//! Where a workbook's parts come from: a package, or, in tests, parts held in memory.

use ooxmldown::{Package, PackageError, Relationship, rels_path};

pub(crate) trait Source {
  fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError>;
  /// The relationships of the part at `part`.
  fn relationships(&mut self, part: &str) -> Result<Vec<Relationship>, PackageError>;
}

impl Source for Package {
  fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError> {
    Package::part(self, path)
  }

  fn relationships(&mut self, part: &str) -> Result<Vec<Relationship>, PackageError> {
    Package::relationships(self, &rels_path(part))
  }
}
