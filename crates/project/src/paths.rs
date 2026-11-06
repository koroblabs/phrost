//! Which paths are not a project's code, and which are its code whatever else says.
//!
//! One type rather than two lists, because the rule is asked in two places that have to agree:
//! discovery steps over an excluded directory, and the server refuses a buffer opened inside one.
//! Held as two slices and threaded through both, an `include` that reached the walk and not the
//! buffer check would index a file and then decline to answer about it -- the shape §8 records for
//! a lowering gate that reached the skeleton and not the open set.
//!
//! # Why `include` outranks everything
//!
//! Three things exclude a path: the built-in names (a version store, a JavaScript dependency tree,
//! the caches PHP's own tools keep), whatever the detected framework said it generates, and what
//! the project itself listed. All three are rules written about something else -- a convention, a
//! framework's own layout, a directory somebody swept out once -- while `include` is a statement
//! about *this* directory. The specific claim wins.
//!
//! The project's ignore files are the fourth, and the one that matters most: a `.gitignore` line is
//! what keeps most generated PHP out of an index, and it is also what hides a real source tree in a
//! checkout that keeps its dependencies out of git. That one is not a predicate at all -- the
//! walker applies it -- so an included root is walked *again* with ignore files off, the way a
//! composer root outside `vendor/` already is.

use std::path::{Path, PathBuf};

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct PathRules {
    excluded: Vec<PathBuf>,
    included: Vec<PathBuf>,
}

impl PathRules {
    /// Absolute paths, both lists. A discovered path and an opened buffer are both absolute, and a
    /// relative rule would match neither.
    pub fn new(excluded: Vec<PathBuf>, included: Vec<PathBuf>) -> Self {
        Self { excluded, included }
    }

    /// Whether a walk should step over this path, and a buffer inside it be refused.
    ///
    /// By prefix, so naming a directory covers everything under it.
    pub fn skips(&self, path: &Path) -> bool {
        !self.claims(path)
            && self
                .excluded
                .iter()
                .any(|excluded| path.starts_with(excluded))
    }

    /// Whether the project named this path, or a directory above it, as its own code.
    ///
    /// Asked by the walk before the built-in names as well as before the exclusions: a project that
    /// names a directory has outranked a convention about what directories of that name usually
    /// hold.
    pub fn claims(&self, path: &Path) -> bool {
        self.included
            .iter()
            .any(|included| path.starts_with(included))
    }

    /// The roots to walk with ignore files off.
    pub fn included(&self) -> &[PathBuf] {
        &self.included
    }

    pub fn excluded(&self) -> &[PathBuf] {
        &self.excluded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three answers, and the one that makes the type worth having.
    #[test]
    fn include_outranks_exclude_for_the_path_it_names() {
        let rules = PathRules::new(
            vec![PathBuf::from("/p/generated")],
            vec![PathBuf::from("/p/generated/keep")],
        );

        assert!(rules.skips(Path::new("/p/generated/one.php")));
        assert!(
            !rules.skips(Path::new("/p/generated/keep/two.php")),
            "the more specific claim wins"
        );
        assert!(
            !rules.skips(Path::new("/p/src/three.php")),
            "a path no rule names is not skipped"
        );
        // By prefix, so the directory itself is claimed as well as what is under it -- the walk
        // asks about the directory before it can reach a file inside.
        assert!(rules.claims(Path::new("/p/generated/keep")));
        assert!(!rules.claims(Path::new("/p/generated")));
    }

    /// An excluded parent stays excluded, and that is why the second walk exists.
    ///
    /// The trap this pins: `skips` answers yes for `/p/a` even though `/p/a/b` is included, so a
    /// single walk would prune `/p/a` and never descend far enough to ask about `/p/a/b` --
    /// `include` would read as doing nothing while every predicate here answered correctly.
    /// [`Self::included`] is the resolution: those roots are walked *as roots of their own*, so
    /// reaching them never depends on the parent being walked. Anything asking `skips` about a
    /// directory has to be a walk that also walks the included roots.
    #[test]
    fn an_excluded_parent_is_why_included_roots_are_walked_separately() {
        let rules = PathRules::new(vec![PathBuf::from("/p/a")], vec![PathBuf::from("/p/a/b")]);
        assert!(rules.skips(Path::new("/p/a")));
        assert_eq!(rules.included(), [PathBuf::from("/p/a/b")]);
        // And from that root the walk is free: the root itself and everything under it is claimed.
        assert!(!rules.skips(Path::new("/p/a/b")));
        assert!(!rules.skips(Path::new("/p/a/b/deep/four.php")));
    }
}
