//! Where the current file reads a local above everything that could have given it a value.
//!
//! A whole-file projection over the body, and a thin one: the ordering rules are the body's, and
//! what this layer adds is the one fact the body cannot know -- whether the file's own top level is
//! a scope anything else can share. Its sibling is
//! [`locals_never_bound`](IndexView::locals_never_bound), which asks whether anything binds the
//! name at all.

use phrost_body::EarlyRead;

use crate::view::IndexView;

impl IndexView<'_> {
    /// Locals in the current file read above every binding of their name.
    ///
    /// Empty when the file is not open, which is the shape of the two tiers rather than a fallback:
    /// this reads a body, and bodies exist for open buffers only.
    pub fn locals_read_before_bound(&self) -> Vec<EarlyRead> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        phrost_body::locals_read_before_bound(open.hir(), self.file_scope_is_private())
    }
}
