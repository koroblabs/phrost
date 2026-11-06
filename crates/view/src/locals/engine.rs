//! The variables PHP's engine binds, which no file declares.
//!
//! A sibling source of a local's type, consulted where the body has no binding to offer -- not a
//! fallback for one: `$_SERVER` is *declared* by the runtime exactly as a parameter is declared by
//! a signature, and the index holds that declaration. Nothing goes to disk and nothing is guessed;
//! a name the engine does not bind still answers nothing.
//!
//! Asked last, because a file may shadow one: `$_GET = [];` at the top of a script is legal and
//! rare, and what the file says about a name it wrote wins over what the engine says about the
//! name it started with.

use crate::{resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What the engine gives a variable it binds itself, if this is one of them.
    ///
    /// No scope travels with it: a relative type cannot be written in an engine binding, so there
    /// is nothing for `self` or `static` to mean.
    pub(crate) fn engine_variable(&self, name: &str) -> Option<Held> {
        let ty = self.skeleton.superglobal_type(name)?;
        Some(Held::Stored { ty, scope: None })
    }
}
