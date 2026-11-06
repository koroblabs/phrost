//! Twig templates, read far enough to say what they name.
//!
//! Twig is not PHP and mago has nothing to say about it, so this is the one place in the workspace
//! that knows its syntax -- the way `phrost-syntax` is the one place that knows mago. Everything
//! above asks it questions in byte ranges and plain strings, and nothing above names a token.
//!
//! # A lexer and readers, not a parser
//!
//! Twig's own compiler builds a tree of nodes, and nothing here needs one yet: the questions asked
//! of a template are about one construct at a time -- which template does this `include` name --
//! and every such construct is a short, flat run of tokens inside one tag. So the template is
//! lexed the way Twig lexes it, and a reader per question walks the tokens of the tags it cares
//! about. A tree is the thing to build when a question spans tags -- which variable a `for` binds,
//! where a block is overridden -- and not before.
//!
//! # Not a framework
//!
//! What a name *means* is the loader's business, and the loader is configured by whoever renders
//! the template: Drupal registers a namespace per extension, Symfony a directory and one per
//! bundle. None of that is here. What is here is what every Twig project shares -- the tags, the
//! functions that take a template's name, and the `@namespace/` spelling the filesystem loader
//! reads.

pub mod lexer;
pub mod names;
pub mod reading;

#[cfg(test)]
mod tests;

pub use names::{NameKind, TemplateName, namespace_of, template_names};
pub use reading::{
    Binding, BindingKind, Called, CalledKind, Chain, Include, Reading, Step, StepKind, Value, read,
};
