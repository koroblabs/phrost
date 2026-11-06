//! The only crate that knows the PHP parser exists.
//!
//! phrost parses with [`mago`](https://github.com/carthage-software/mago). This crate is the wall
//! around that choice: it owns the arena, turns mago's positions and errors into phrost types, and
//! answers the one question the tree cannot answer for itself — which docblock belongs to which
//! declaration.
//!
//! # Why the wall exists
//!
//! mago's tree is arena-bound (`Program<'arena>`), so a parsed node cannot be stored anywhere
//! long-lived, and its node shape has already changed under this project once: at 1.26 the tree
//! lived in an `ast` module and was not arena-bound, at 1.45 it is `cst` and is. Anything that can
//! name one of those types is something a parser upgrade can break. Only this crate and
//! `phrost-lower` may.
//!
//! # Lifecycle
//!
//! ```text
//! ParseSession::new()      -- owns a bump arena
//!   .parse(text)           -- tree borrows the arena
//!     lower to FileHir     -- owned, no lifetimes
//! drop(session)            -- arena and tree die together
//! ```
//!
//! Keep the session alive only while the tree is needed. For indexing that is the duration of one
//! lowering; for an open editor buffer it is as long as syntax-level features need the tree.

mod docblock;
mod error;
mod reference;
mod refused;
mod session;
mod strings;

pub use self::{
    docblock::Docblock,
    error::SyntaxError,
    reference::{ReferenceKind, see_reference},
    session::{ParseSession, ParsedFile},
    strings::{Lexed, string_ranges, strings_and_comments},
};

use phrost_model::{TextRange, TextSize};

/// Converts a mago span to a phrost range.
///
/// mago spans carry a file id, which phrost drops: a span is only ever interpreted against the
/// file that was just parsed, and phrost has its own file identity that means something.
pub fn text_range(span: mago_span::Span) -> TextRange {
    TextRange::new(
        TextSize::from(span.start.offset),
        TextSize::from(span.end.offset),
    )
}

/// Reads identifier bytes as text.
///
/// mago hands out `&[u8]` for names. The input to [`ParseSession::parse`] is `&str`, so these are
/// always valid UTF-8 in practice; the lossy decode is here so that a surprise cannot turn into a
/// panic deep inside lowering. Borrowing is the normal path and allocates nothing.
pub fn identifier_text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}
