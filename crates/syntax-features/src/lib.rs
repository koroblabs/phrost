//! The features that need tokens and trivia rather than a resolved project.
//!
//! Folding, and the syntax-level features that follow it, answer from one file's text and nothing
//! else. Putting them in `phrost-analysis` would mean letting the parse tree past the boundary that
//! exists to keep a parser upgrade cheap; putting them here makes "syntactic feature" an explicit,
//! short list instead of a hole in that boundary.
//!
//! # Why every entry point takes `&str`
//!
//! Nothing above may name a parse tree, so an entry point cannot accept one: the caller would have
//! to hold what it is not allowed to mention. So each one parses the text it is given, answers, and
//! drops the arena before returning — which makes "never store a parsed tree" structural here
//! rather than a rule somebody has to remember.
//!
//! The cost is one parse per request. That is the price of the boundary and it is small: parsing is
//! the cheap half of indexing, and these requests are about a single open buffer. Should two of
//! these features ever be asked for together, one parse feeding both is a change to make then, with
//! a measurement to justify it.
//!
//! # What is not here
//!
//! Anything that needs to know what a name means. `Foo` is a class, an interface or an enum
//! depending on a declaration in another file, and this crate cannot see one. A feature that starts
//! wanting that answer belongs in `phrost-analysis`, over `phrost-view`.

pub mod folding;
pub mod format;
pub mod lint;
pub mod regions;
pub mod selection;
pub mod tokens;
