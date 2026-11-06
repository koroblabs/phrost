//! Resolved PHP type references, interned once per project.
//!
//! A [`TypeRef`] is a type *as the author wrote it*, with every class name already resolved to a
//! fully-qualified one: `?User` inside `namespace App` is a `Nullable` around `Named("App\User")`.
//! It is deliberately not an inferred type — no union flattening, no subtyping, no narrowing. That
//! is the type layer's work, and it needs this as its input.
//!
//! # Why the index stores these instead of rendered text
//!
//! Resolution depends only on the declaring file's namespace and imports, so it can be done once at
//! ingest and can never go stale when another file changes. Rendered text has the same property but
//! answers only one question — "what does this look like" — and every other question about a type
//! (does this method return something with members? is this parameter nullable?) then has no source
//! but re-parsing the file it came from. That re-parse is exactly the kind of second path the
//! no-fallbacks rule exists to prevent.
//!
//! # Why it is interned
//!
//! A real project has hundreds of thousands of members and a few thousand distinct types: `string`,
//! `?string`, `int`, `array`, `ContainerInterface`. Interning makes each use a 4-byte [`TypeId`],
//! and makes two types equal exactly when their ids are — so comparing types is an integer compare
//! rather than a tree walk.

pub mod compat;
mod refine;
mod render;
mod table;
mod type_ref;

pub use self::{
    render::shape_key,
    table::{TypeId, TypeList, TypeTable},
    type_ref::TypeRef,
};
