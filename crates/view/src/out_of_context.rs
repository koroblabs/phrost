//! `$this`, `self`, `static` and `parent` written where the language gives them nothing.
//!
//! Four of PHP's `Error`s, each fatal the moment the line runs, and each stated entirely by the
//! file: `Using $this when not in object context`, `Cannot use "self" when no class scope is
//! active`, the same for `static`, and `Cannot use "parent" when current class scope has no
//! parent`. The mistake behind the first is ordinary -- a method copied and given `static`, or a
//! closure written `static function` for the garbage collector with a `$this` still inside it.
//!
//! [`phrost_body::out_of_context`] finds the positions and keeps `parent` as a question, because
//! whether the enclosing class has one is the only half that needs a hierarchy. This answers it.

use phrost_body::Written;
use phrost_model::TextRange;

use crate::view::IndexView;

/// One name with nothing to refer to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OutOfContext {
    pub range: TextRange,
    pub written: Written,
}

impl IndexView<'_> {
    /// Every `$this` or relative word in the current file PHP would refuse.
    pub fn out_of_context(&self) -> Vec<OutOfContext> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::out_of_context(hir)
            .into_iter()
            .filter(|found| match found.written {
                // `parent` needs a class *with* a parent, and a class this index cannot resolve
                // answers nothing -- the same silence every check here keeps over a gap.
                // A trait's `parent::` is resolved against the *using* class, which the trait
                // cannot know -- Symfony's `MicroKernelTrait` writes twenty-two of them.
                Written::Parent => found.enclosing.is_some_and(|id| {
                    let class = hir.class(id);
                    class.extends.is_empty() && class.kind != phrost_hir::ClassKind::Trait
                }),
                _ => true,
            })
            .map(|found| OutOfContext {
                range: found.range,
                written: found.written,
            })
            .collect()
    }
}
