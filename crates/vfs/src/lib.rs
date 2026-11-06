//! File identity and source text for one analysis session.
//!
//! Everything above this crate refers to files by [`FileId`] and never by path. That is not just
//! a size optimization: a path string that travels between layers eventually gets normalized
//! differently in two of them, and then two layers disagree about which file they are discussing.
//!
//! This crate owns text and identity only. Parsing, semantics, and feature logic are elsewhere;
//! nothing here knows what PHP is.

mod document;
mod source;
mod vfs;

pub use self::{
    document::{DocumentVersion, OpenDocument},
    source::{FileFingerprint, FileStamp, SourceText},
    vfs::{Vfs, normalize},
};
