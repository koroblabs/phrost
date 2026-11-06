//! The counts a project has agreed to, and refusing to let them grow. The counting, the comparison
//! and the file are `lspkit_ratchet`'s, shared with orthra; what is phrost's is below.
//!
//! `docs/02-testing.md` names this directly and says why it is load-bearing rather than tidy:
//! **strictness without this ratchet becomes noise**. Keeping `?T where T expected` on is a
//! deliberate choice here, and it is only liveable if the number of findings can go down and cannot
//! go up -- otherwise the list grows past the point anyone reads it and the strictness has bought
//! nothing.
//!
//! # Why per code and not a total
//!
//! A total hides a trade. Closing four `undefined-method` findings while a change adds four
//! `possibly-null-receiver` reads as no change at all, and the two are not the same fact about the
//! project. So every code is ratcheted on its own, and a rise in any one of them fails.
//!
//! # What is counted, and what is not
//!
//! The index's findings, and not the syntax rules. Those are chosen by configuration -- a project
//! that turns a category on would break a ratchet by agreeing with itself -- and mago is pinned, so
//! they cannot drift on their own. What this guards is the thing whose precision was argued for one
//! check at a time.
//!
//! # Why a new code fails
//!
//! A check nothing has agreed to is a change to what the project is told about itself, and that is a
//! decision rather than an accident. Adding one means accepting a new baseline, which is a diff
//! somebody reads.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use lspkit_ratchet::Counts;

/// The file's name, beside the project it describes.
///
/// With the corpus and not with this repository, because that is what it is about: a baseline taken
/// on one checkout says nothing about another, and the file has to travel with the code it counts.
/// That is also why a fall is not written back on its own, as orthra's is: this file lives in other
/// people's checkouts, and a check does not write there unasked.
const FILE: &str = "phrost-baseline.toml";

/// How the file is written again, in the words its own header uses.
pub(super) const COMMAND: &str = "phrost check --ratchet --accept";

pub(super) fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// This run's counts, as the ratchet keeps them.
pub(super) fn counts(by_code: &BTreeMap<&'static str, usize>) -> Counts {
    by_code
        .iter()
        .map(|(code, count)| ((*code).to_owned(), *count))
        .collect()
}
