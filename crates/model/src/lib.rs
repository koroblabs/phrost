//! Shared identities and PHP primitives for every phrost layer.
//!
//! This crate deliberately owns **no** `*Data`, no lowering, no resolution, and no logic beyond
//! trivial accessors. It exists so that two layers can talk about the same entity without either
//! one depending on the other's storage: they exchange ids, not structures.
//!
//! That makes it the one crate everything may depend on, which also makes it the one crate that
//! will rot fastest if it is treated as a `utils` dump. Adding something here needs a reason of
//! the form "more than one layer needs to name this, and neither owns it".
//!
//! Definition identity (`DefId` and friends) is intentionally absent: it is a Phase 2 decision
//! that belongs with the def-map, and guessing at it now would lock in the wrong shape.

/// The stack a thread needs to survive a PHP file nobody wrote by hand.
///
/// Lowering, resolution and the linter all recurse over the expression tree, so the *smallest*
/// stack in the process decides what phrost survives -- and a stack overflow is the one failure
/// no panic containment can catch, because it is an abort rather than an unwind. The number is
/// measured: mago recurses roughly 0.5 KiB per level, a tokio worker's default is 2 MiB, and a
/// 4 000-deep method chain in an open buffer overflowed one and took the whole server with it.
/// 64 MiB is about 130 000 levels.
///
/// Nearly free, which is why it is this large rather than tuned: a thread stack is reserved address
/// space faulted in a page at a time, measured at no change in resident memory.
///
/// # Why it lives here
///
/// Three places spawn threads that lower PHP -- the server's runtime, the CLI's sweep, and the
/// parallel ingest inside the index -- and none of them owns the fact. Two of them had the default
/// stack until this constant existed, which is [`phrost-model`]'s stated bar for a new item: more
/// than one layer needs to name it and neither owns it. Measured before it was one number: a
/// 4 000-deep chain, a 20 000-term `+` and a 20 000-term `.` each aborted `phrost check` while the
/// server answered all three.
pub const DEEP_RECURSION_STACK: usize = 64 * 1024 * 1024;

mod file;
mod fqn;
mod line_index;
mod matching;
mod php_type;
mod php_version;
pub mod schema;
mod symbol;
mod visibility;

pub use phrost_intern::{
    Interner, InternerStats, Name, NameKey, display_name, is_an_integer_key, slot_name,
};
pub use text_size::{TextRange, TextSize};

pub use self::{
    file::{FileId, SourceOrigin},
    fqn::{namespace_of, short_name},
    line_index::{LineCol, LineIndex},
    matching::{letters_of, match_score, matches_loosely, starts_with_ignoring_case},
    php_type::{Absence, BuiltinTy, DeclaredInput, RelativeTy, SUPERGLOBALS, ValueShape},
    php_version::PhpVersion,
    symbol::SymbolKind,
    visibility::Visibility,
};
