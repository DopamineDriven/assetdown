//! The containers a file can be: boxes, chunks, elements, a zip's entries, a compound file's
//! streams. Each reader here says what a container holds and nothing about what that makes the
//! file; `sniff` decides that. They are read the same way by every type kept in them.

pub(crate) mod cfb;
pub(crate) mod ebml;
pub(crate) mod exif;
pub(crate) mod icc;
pub(crate) mod ifd;
pub(crate) mod isobmff;
pub(crate) mod riff;
pub(crate) mod xmp;
pub(crate) mod zip;
