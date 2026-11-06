//! Where a `foreach` is handed something it cannot iterate.
//!
//! The third of a family. [`non_array_subscripts`](Self::non_array_subscripts) asks whether a value
//! holds elements at all, [`null_subscripts`](Self::null_subscripts) asks whether it is there, and
//! this asks whether a `foreach` can walk it -- the same two questions about the one statement that
//! reads a container whole.
//!
//! # Why there is no list of kinds here
//!
//! There nearly was, and it would have had to differ from the subscript one by exactly one type: a
//! **string** is subscriptable and is not iterable. `$word[0]` is a character and PHP says so;
//! `foreach ($word as $c)` is `foreach() argument must be of type array|object, string given`. Two
//! lists that differ by one entry are a shape §8.1 names, and the measurement said not to build it
//! -- see [`NotIterable`] for the two sites it found and what was wrong with them.
//!
//! What is left is the pair of sentinels, which is also what PHPStan's `foreach.nonIterable`
//! population on a real project is made of. And note that what *is* iterable is wider than what
//! holds elements: **any object** is, because `foreach` over one walks its public properties
//! whether or not it implements `Traversable`.

use phrost_body::IteratedSubject;
use phrost_model::{BuiltinTy, TextRange};

use crate::{
    references::SentinelReach,
    view::{FOLLOW_LIMIT, IndexView},
};

/// Why a `foreach` subject cannot be walked.
///
/// **The sentinels only, and that is a measurement rather than a limit of the machinery.** A third
/// arm asking whether the *kind* can be walked at all -- an int, a string -- was written, ran over
/// five projects and found **two** sites in thirty thousand files. One was `foreach` over a
/// `get_theme_mod('x')` typed as exactly `false`, which it is not: the return is bound to the
/// `$default_value = false` parameter and reading an omitted default as the whole answer is a
/// confidence the code does not support. The other resolved an element read to a `string` for
/// reasons an hour did not settle. A rule that fires twice and is wrong at least once is noise with
/// a good story, so what ships is the half that was sampled true a hundred times.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotIterable {
    /// It may not be there. `certain` where null arrives on every run rather than on some.
    Null { certain: bool },
    /// The `false` a standard-library call hands back for *not found*, uncompared.
    False { certain: bool },
}

/// One `foreach` over something it cannot walk.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NotIterableLoop {
    /// The subject as written, which is what a report underlines.
    pub range: TextRange,
    /// What it holds, rendered -- the evidence, on the line the claim sits on.
    pub rendered: Option<String>,
    pub why: NotIterable,
}

impl IndexView<'_> {
    /// Every `foreach` in the current file whose subject cannot be iterated.
    ///
    /// Empty when the file is not open, which is the shape of the two tiers rather than a fallback:
    /// a `foreach` lives in a body, and bodies exist for open buffers only.
    pub fn not_iterable_loops(&self) -> Vec<NotIterableLoop> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::iterated_subjects(hir)
            .into_iter()
            .filter_map(|site| self.walkable(hir, site))
            .collect()
    }

    fn walkable(
        &self,
        hir: &phrost_hir::FileHir,
        site: IteratedSubject,
    ) -> Option<NotIterableLoop> {
        let held = self.resolve_path(site.range.start(), hir, &site.subject, FOLLOW_LIMIT)?;
        // `mixed` answers `Possible` to the null question without meaning it -- PHP has no `?mixed`,
        // so a declaration writes `mixed|null` for what is already every type, and reading that as
        // a null nobody handled reports every value nothing describes. `states_non_null` names the
        // same exception for the same reason.
        if self.kind_of_held(hir, &held) == Some(BuiltinTy::Mixed) {
            return None;
        }
        // The sentinels first and in this order, which is the order the receiver check reads them:
        // a value that is both is one mistake, and the null is the one a person guards against.
        let why = match (
            self.null_reach_of(hir, &held),
            self.false_reach_of(hir, &held),
        ) {
            (SentinelReach::Certain, _) => NotIterable::Null { certain: true },
            (SentinelReach::Possible, _) => NotIterable::Null { certain: false },
            (_, SentinelReach::Certain) => NotIterable::False { certain: true },
            (_, SentinelReach::Possible) => NotIterable::False { certain: false },
            _ => return None,
        };
        Some(NotIterableLoop {
            range: site.range,
            rendered: self.render_held(
                site.range.start(),
                hir,
                &held,
                crate::variables::Naming::Short,
            ),
            why,
        })
    }
}
