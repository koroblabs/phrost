//! What the coarse "does anything use this name" rule reads, gathered once for a file.
//!
//! Until the project has been swept, whether a declaration is reached is answered by words: some
//! other file's word set may hold the name, or this file writes it twice. Both used to be asked
//! per declaration -- a walk of the whole file table and a scan of the whole text for each one --
//! so a file of two hundred methods paid for two hundred of each, and on the reference project's
//! largest file that was nine of the sixteen milliseconds its diagnostics took.

use std::cell::OnceCell;

use phrost_model::FileId;
use rustc_hash::FxHashSet;

/// The words one file's declarations are judged by, before the sweep has landed.
pub(crate) struct WordEvidence<'a> {
    text: &'a str,
    /// The names some other file's word set may hold, out of the ones the file declares.
    outside: FxHashSet<&'a str>,
    /// Every word the file writes inside quotes, taken only when a name is not settled without.
    quoted: OnceCell<Vec<String>>,
}

impl<'a> WordEvidence<'a> {
    /// Asks the word sets about every name at once, in one pass over the file table.
    pub(crate) fn gather(
        skeleton: &phrost_index::ProjectSkeleton,
        file: FileId,
        text: &'a str,
        names: Vec<&'a str>,
    ) -> Self {
        let held = skeleton.mentioned_outside_among(file, &names);
        let outside = names
            .into_iter()
            .zip(held)
            .filter_map(|(name, held)| held.then_some(name))
            .collect();
        Self {
            text,
            outside,
            quoted: OnceCell::new(),
        }
    }

    /// Whether another file may write the name -- one of the names given to [`Self::gather`].
    pub(crate) fn mentioned_outside(&self, name: &str) -> bool {
        self.outside.contains(name)
    }

    /// Whether this file writes the name anywhere but at the declaration: a call, a string, a
    /// docblock. The word sets answer for every *other* file and this answers for this one.
    pub(crate) fn written_in(&self, name: &str) -> bool {
        self.quoted
            .get_or_init(|| crate::unused_members::quoted_words(self.text))
            .iter()
            .any(|word| word.eq_ignore_ascii_case(name))
            // Twice is all the rule asks, so the scan stops at the second.
            || self.text.match_indices(name).nth(1).is_some()
    }
}
