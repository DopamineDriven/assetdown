//! The OOXML package layer shared by the format crates.
//!
//! An `.xlsx`, `.docx` or `.pptx` is a zip of XML parts laid out by the Open Packaging
//! Conventions (ECMA-376 Part 2). This crate opens the zip within limits, finds parts through
//! relationships and types them by `[Content_Types].xml`. Beyond naming what kind of document a
//! package holds, it knows nothing about what the parts mean; that is the format crates' job.

mod error;
mod package;

pub use error::PackageError;
pub use package::{
  Limits, Package, PackageKind, Relationship, XML_NODES_LIMIT, parse_xml, rels_path,
};
