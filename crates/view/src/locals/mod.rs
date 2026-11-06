//! What a local variable holds.
//!
//! The projection every source of a variable's type arrives behind: a parameter's hint, an
//! assignment, an inline `@var`, and one day a narrowing `instanceof`. Nothing above the view
//! changes when another one lands, which is the reason it is one method rather than several.
//!
//! # How the parts divide
//!
//! [`bindings`] is asked; everything else answers. A value is a **root** plus the **steps** taken
//! off it, and those are the two halves a new kind of value lands in -- [`path`] and [`steps`] --
//! so they are the two that must not share a file. The rest are the sources of a type those two
//! consult: what a guard leaves of a value ([`narrowing`]), what a call did to it ([`calls`]),
//! which earlier call produced it ([`produced_by`]), and the one place a value is named by a string
//! rather than by a type ([`catalog_keys`]).

mod arithmetic;
mod bindings;
pub(crate) mod calls;

mod catalog_keys;
mod constants;
mod engine;
mod filled;
mod identity;
mod membership;
mod narrowing;
mod path;
mod produced_by;
mod rewritten;
mod steps;
mod subclass;
