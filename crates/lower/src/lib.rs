//! Parse tree to owned per-file IR.
//!
//! This crate and `phrost-syntax` are the only two that know the shape of mago's tree. Lowering
//! reads it once, produces a [`FileHir`] that owns everything it needs, and never looks again —
//! after which the parse arena can be dropped.
//!
//! # What is lowered, and what is not
//!
//! Declarations always: namespaces, imports, classes and their members, functions, and constants.
//! Statement bodies only when asked for — [`lower_text_with_bodies`] — because they are much larger,
//! needed far less often, and only for open files.
//!
//! Nothing is resolved here. `extends Repo` becomes a written name plus how it was anchored, not a
//! pointer to a class — the def-map decides what `Repo` means, because that depends on imports and
//! namespace, which can change without this file changing at all.
//!
//! # Structural translations
//!
//! mago's tree and phrost's IR disagree in a few places, on purpose:
//!
//! - **Union and intersection hints are binary trees in mago, N-ary here.** `A|B|C` arrives as
//!   nested pairs and is flattened, because every consumer wants the operand list.
//! - **Group imports are flattened.** `use App\{A, B as C};` becomes two imports, so no consumer
//!   has to re-derive what a group import means.
//! - **One declaration can hold several members.** `public int $a, $b;` becomes two properties.
//! - **Promoted constructor parameters also become properties**, so member lookup does not need to
//!   know that constructor property promotion exists.

mod annotations;
mod attributes;
mod body;
mod conditional;
mod constant_values;
mod constructions;
mod defined_constants;
mod docblock;
mod docblock_members;
mod docblock_templates;
mod expression;
mod hooks;
mod lowerer;
mod members;
mod own_arguments;
mod params;
mod returned_local;
mod returns;
mod string_holding;
mod types;
mod yields;

#[cfg(test)]
mod tests;

use phrost_hir::FileHir;
use phrost_model::{FileId, Interner};
use phrost_syntax::{ParseSession, ParsedFile};

pub use attributes::KeyedAttributes;

/// Re-exported because it is part of this crate's return type.
///
/// A plain phrost value — a message and a range — with nothing of the parser in it, so it can
/// travel to the diagnostics layer without carrying the parse boundary along with it.
pub use phrost_syntax::SyntaxError;

/// Everything one file's lowering produced.
#[derive(Debug)]
pub struct LoweredFile {
    pub hir: FileHir,
    /// Syntax errors from the parse, carried along so the caller does not need the tree to report
    /// them. A file with errors is still lowered: whatever parsed correctly is still useful, and
    /// an editor buffer is briefly invalid most of the time it is being typed into.
    pub errors: Vec<SyntaxError>,
}

/// Parses and lowers source text, then drops the parse arena.
///
/// This is the entry point for indexing, and the reason it takes text rather than a tree: a caller
/// that had to hand in a `ParsedFile` would need to name a `phrost-syntax` type, which puts the
/// parser back into the dependency graph of everything that indexes a file -- exactly what the
/// boundary exists to prevent. The session lives for the length of this call and no longer.
///
/// The interner is borrowed rather than owned so names are shared across the whole project. A
/// per-file interner would defeat the point: `getName` appears in thousands of files and should be
/// one allocation, not thousands.
pub fn lower_text(
    interner: &mut Interner,
    file: FileId,
    text: &str,
    attributes: &KeyedAttributes,
) -> LoweredFile {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    lower_file(interner, file, &parsed, attributes)
}

/// Lowers a tree the caller already has.
///
/// For callers that legitimately hold a live parse -- the syntax-only feature crate, which needs
/// tokens and trivia for an open buffer -- and for tests that want to inspect the tree as well.
pub fn lower_file(
    interner: &mut Interner,
    file: FileId,
    parsed: &ParsedFile<'_>,
    attributes: &KeyedAttributes,
) -> LoweredFile {
    LoweredFile {
        hir: lowerer::Lowerer::new(interner, file, parsed, attributes).run(),
        errors: parsed.errors(),
    }
}

/// The same, plus every function-like's statement body.
///
/// For open buffers only. Bodies are an order of magnitude larger than the declarations they belong
/// to, and no cross-file question reads one: where a class is declared and what members it has are
/// the same answers whether or not anybody looked inside a method. Indexing the project with this
/// would multiply the work by the size of the code for facts nothing consults.
pub fn lower_text_with_bodies(
    interner: &mut Interner,
    file: FileId,
    text: &str,
    attributes: &KeyedAttributes,
) -> LoweredFile {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    LoweredFile {
        hir: lowerer::Lowerer::new(interner, file, &parsed, attributes)
            .with_bodies()
            .run(),
        errors: parsed.errors(),
    }
}
