//! Twig templates: the names they load other templates by, and the variables they read.
//!
//! A template is text the index holds nothing about, answered the way a configuration buffer is:
//! `phrost-twig` reads it, and what each thing it reads *is* comes from catalogs and contracts a
//! framework filled -- which template a name loads, which call or array renders a template and what
//! it hands it.

mod callables;
mod names;
mod variables;

pub use callables::{TwigCallable, TwigCallableKind};
pub use variables::{TwigCandidate, TwigCandidateKind, TwigOrigin, TwigSymbol, TwigSymbolKind};

use crate::view::IndexView;

/// What a Twig template is called, whatever it renders.
const TWIG: &str = ".twig";

/// Every name `text` renders another template by with its variables: what it includes, embeds and
/// extends -- the facts a workspace records about each of the project's templates, so that the one
/// included can find who includes it.
pub fn included_template_names(text: &str) -> Vec<Box<str>> {
    phrost_twig::template_names(text)
        .into_iter()
        .filter(|name| {
            matches!(
                name.kind,
                phrost_twig::NameKind::Include
                    | phrost_twig::NameKind::Embed
                    | phrost_twig::NameKind::Extends
                    | phrost_twig::NameKind::IncludeFunction
            )
        })
        .map(|name| name.name.into_boxed_str())
        .collect()
}

impl IndexView<'_> {
    /// Whether a file of this name is a template the project's Twig loader reads.
    pub(crate) fn reads_twig(&self, file_name: &str) -> bool {
        file_name.ends_with(TWIG) && !self.skeleton.returns().twig_loaders().is_empty()
    }
}
