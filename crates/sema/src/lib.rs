//! Flattening a PHP class hierarchy into the members a class actually has.
//!
//! Given a class, this crate answers "what members does it have, where did each come from, and
//! which declaration wins" — after inheritance, trait application, and PHP's conflict rules.
//!
//! # It reads through a trait, not a store
//!
//! Flattening needs to walk from a class to its parents, interfaces, and traits, which live in
//! other files. Rather than depend on the index, this crate declares [`ClassSource`] and the index
//! implements it. Three things follow:
//!
//! - The algorithm is unit-testable against a hand-built hierarchy, with no project, no
//!   filesystem, and no indexing. PHP's precedence rules are subtle enough that this matters.
//! - The dependency arrow points the right way: storage depends on semantics, not the reverse.
//! - Nothing here can accidentally start reading a second source of truth.
//!
//! # PHP's precedence, in one place
//!
//! Own members beat trait members, which beat inherited ones. That order is the whole reason a
//! class can `use` a trait to override something its parent defined while still being overridable
//! by its own declaration.
//!
//! Two rules are easy to get backwards and both are covered by tests:
//!
//! - **Private members are not inherited.** A parent's private method is not part of the child's
//!   table at all, and a child declaring the same name is not overriding anything.
//! - **Traits are copy-paste, so their private members *are* included.** A trait's private method
//!   behaves as if it had been written in the using class.
//!
//! Conflicts between two traits are a fatal error in PHP unless `insteadof` resolves them, so they
//! are reported rather than silently resolved — picking a winner would make the language server
//! disagree with the runtime.

mod flatten;
mod member;
mod render;
mod source;
mod table;

#[cfg(test)]
mod tests;

pub use self::{
    flatten::{BACKED_ENUM, FlattenMemo, UNIT_ENUM, flatten_class, flatten_class_with},
    member::{
        ConditionTest, ConditionalBranch, ConstantTable, Contract, Hooks, MemberKind,
        MemberModifiers, MemberRow, ParamPromotion, ParamRow, ReturnBranch, ShapeLiteral, TestCase,
    },
    render::render_member_table,
    source::{ClassFacts, ClassKind, ClassRef, ClassSource, TraitAdaptation, TraitAdaptationKind},
    table::{MemberConflict, MemberProvenance, MemberTable, ResolvedMember},
};
