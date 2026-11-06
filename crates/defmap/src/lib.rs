//! Namespace and import resolution: what a written PHP name could refer to.
//!
//! Lowering keeps names as written, because `Repo` means different things in different files and
//! even in different halves of one file. This crate applies the rules that turn a written name
//! into fully-qualified candidates.
//!
//! # It produces candidates, not answers
//!
//! Resolution here is **pure**: it takes a file's namespace and imports plus a written name, and
//! returns the fully-qualified names PHP would look for, in the order PHP would try them. Whether
//! any of them exists is a different question, answered by whoever owns the project-wide catalog.
//!
//! That split is deliberate. It keeps the rules — which are fiddly and asymmetric — testable on
//! their own, without a project, an index, or a filesystem. And it means this crate has no
//! opinion about what is indexed, so a name resolves the same way whether its target is user code,
//! vendor code, or missing entirely.
//!
//! # The rules, because they are not symmetric
//!
//! PHP keeps three separate symbol tables — class-like, function, constant — and treats them
//! differently in two ways that matter:
//!
//! - **Global fallback.** An unqualified *function* or *constant* name falls back to the global
//!   namespace when the current namespace does not define it. An unqualified *class* name does
//!   not: `new Repo()` inside `namespace App` looks only for `App\Repo`, never `\Repo`.
//! - **Case.** Class and function names match case-insensitively, constants exactly. So
//!   `use App\Repo;` lets you write `repo`, but `use const App\MAX;` does not let you write `max`.
//!
//! A *qualified* name (more than one segment) never falls back to global, whatever the table. Its
//! first segment is translated through the **class-like** import table even when the lookup is for
//! a function — that is what makes `use App\Support; Support\render();` work.
//!
//! # Not handled yet
//!
//! `class_alias()` and conditionally-declared classes are runtime constructs that live in
//! expression bodies, which are not lowered yet. Both need "declared only sometimes" semantics,
//! and this crate is the right place for them once bodies exist.

mod render;
mod resolve;
mod scope;
mod symbol;

#[cfg(test)]
mod tests;

pub use self::{
    render::render_file_scope,
    resolve::{Candidate, NameResolution},
    scope::{FileScope, ImportBinding, NamespaceScope, ScopeImports, ScopedAttributeLiteral},
    symbol::{DeclarationSource, DeclaredSymbol, SymbolWords},
};
