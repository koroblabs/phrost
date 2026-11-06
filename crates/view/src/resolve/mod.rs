//! Turning a written name into the thing it names.
//!
//! Everything here answers "what does this text mean, here": PHP's three symbol tables, the file's
//! imports, `self`/`static`/`parent`, and a stored type read back as a class. Nothing walks a body,
//! and the only thing a position decides is which namespace scope applies.
//!
//! Carved by the question asked, so that two people extending two of them never edit one file.
//! [`held`] is the vocabulary; [`name`] and [`target`] read what the source wrote; [`class_of`]
//! and [`hierarchy`] answer which class a type is and what several of them reduce to;
//! [`narrowing`] is the only module that rebuilds a type rather than reading one; [`predicate`],
//! [`element`] and [`indexing`] answer what a held type is and what is inside it, and
//! [`object_kinds`] what an object of a class can stand in for; [`magic`] answers
//! for the names a class has no declaration for.
//!
//! [`UnionOperand`] is declared here rather than in [`narrowing`] because it is the shape a caller
//! outside this module writes its predicate over, so the module root is the name it is reached by.

use phrost_model::{BuiltinTy, Name};

mod accessors;
mod class_of;
mod columns;
mod element;
mod fluent;
mod held;
mod hierarchy;
mod indexing;
mod invoking;
mod macros;
mod magic;
mod method_backed;
mod name;
mod narrowing;
mod object_kinds;
mod object_shape;
mod predicate;
mod prefixed;
mod rendered;
mod rendering;
mod stated;
mod target;

pub(crate) use self::{
    held::{Callable, Held, RelativeScope},
    hierarchy::HIERARCHY_DEPTH,
    indexing::Subscript,
    predicate::Answers,
};

/// One operand of a union, described the two ways a narrowing test asks about it.
pub(crate) struct UnionOperand {
    /// What to hold on to when this is the one that survives.
    pub(crate) held: Held,
    /// The class it names, when it names one.
    pub(crate) class: Option<Name>,
    /// What kind of value it is, as a type predicate would see it.
    pub(crate) kind: Option<BuiltinTy>,
}
