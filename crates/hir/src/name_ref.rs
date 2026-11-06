use phrost_model::{Name, TextRange};

use crate::import::ImportKind;

/// A name exactly as the source wrote it, before any resolution.
///
/// Kept separate from a resolved definition on purpose. `extends Repo` and `extends \App\Repo`
/// mean the same thing only after the file's imports and namespace are known, and that knowledge
/// belongs to the def-map. Storing the written form plus enough context to resolve it later is
/// what lets the same HIR serve a file whose imports change.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NameRefData {
    /// The path as written, without a leading separator. `App\Repo` for both `App\Repo` and
    /// `\App\Repo`; the distinction is in [`Self::anchor`].
    pub path: Name,
    pub anchor: NameAnchor,
    /// Which of PHP's three symbol tables the name refers to.
    ///
    /// Recorded rather than assumed. PHP resolves classes, functions, and constants through separate
    /// tables, so `Repo()` and `new Repo` can name different things in the same file — and looking a
    /// function call up among classes finds the class whenever one shares the name, which is a wrong
    /// goto that looks like a working one.
    pub kind: ImportKind,
    /// Range of the name itself, which is what goto and rename operate on.
    pub range: TextRange,
    /// Written in a docblock rather than in code.
    ///
    /// The distinction is about what a missing name means. In code it is a fatal error; in a
    /// docblock it is a wrong annotation -- and the docblock type language has names PHP does not,
    /// starting with a `@template` parameter, so the two cannot be judged by the same rule.
    pub in_docblock: bool,
    /// A `@see` pointing at the name rather than a type written with it.
    ///
    /// Navigable like any other, and renamed with its class -- but not a claim about what a value
    /// is, so a check of the types a docblock states leaves it out: `@see RfcLogLevel` in prose is a
    /// pointer for a reader, whatever the file imports.
    pub cross_reference: bool,
}

impl NameRefData {
    /// Whether the path has more than one segment, and so cannot be satisfied by an import alias
    /// alone.
    pub fn is_qualified(&self) -> bool {
        self.path.as_str().contains('\\')
    }

    /// The last segment, which is what short-name lookup and display use.
    pub fn short(&self) -> &str {
        self.path
            .as_str()
            .rsplit('\\')
            .next()
            .unwrap_or(self.path.as_str())
    }
}

/// How a written name is anchored, which decides how the def-map resolves it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NameAnchor {
    /// `\App\Repo` — already absolute, imports and the current namespace are irrelevant.
    FullyQualified,
    /// `Repo` or `App\Repo` — resolved against imports first, then the current namespace.
    Relative,
    /// `namespace\Repo` — explicitly relative to the current namespace, skipping imports.
    CurrentNamespace,
}

impl NameAnchor {
    /// How a name spelled in source is anchored, and the path with the marker taken off.
    ///
    /// Here rather than at each reader because there are three of them -- lowering a mago
    /// identifier, lowering a docblock's written name, and the text-scanned receiver the completion
    /// path reads -- and they must agree. They did not: the scanner discarded the leading separator
    /// altogether, so `\Drupal::` resolved as if it had been written `Drupal`, which inside
    /// `namespace Drupal\app_core;` means `Drupal\app_core\Drupal` and is nothing. Completion after
    /// the commonest static receiver a Drupal project has -- 256 sites in one project's own code,
    /// 7 076 leading-separator receivers across its checkout -- offered nothing at all, while the
    /// same class reached through an import offered everything.
    ///
    /// The path comes back *without* the marker because that is what the def-map's `resolve` takes:
    /// the anchor carries what the marker said.
    pub fn of(written: &str) -> (Self, &str) {
        match written.strip_prefix('\\') {
            Some(rest) => (Self::FullyQualified, rest),
            None => match written.strip_prefix("namespace\\") {
                Some(rest) => (Self::CurrentNamespace, rest),
                None => (Self::Relative, written),
            },
        }
    }
}
