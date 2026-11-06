//! The namespaces a name being written can continue into.
//!
//! A namespace is declared by nothing of its own -- what says one exists is that something is
//! declared under it -- so the answer is read off the declared names, the same rule
//! applies to the classes beside it: a name that begins
//! with what was typed and has no separator after it is a class there, and one that does is the
//! way into a namespace.
//!
//! A walk over every declared name, and one: the names under a prefix and the namespaces past it
//! are the two halves of the same walk, and a request asks for both from [`IndexView::under`].

use std::{borrow::Cow, collections::BTreeMap};

use phrost_index::Declarer;

use crate::{
    completion::{Candidate, NameKinds},
    view::IndexView,
};

/// Everything a path written so far goes on into, from one walk.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PathContinuations {
    /// The names directly under it, ranked the way every page of names is.
    pub names: Vec<Candidate>,
    /// The namespaces that continue it, as declarations spell them.
    pub namespaces: Vec<String>,
}

/// One walk's two halves: what ends after the prefix, and what goes on past a separator.
pub(crate) struct Under<'a> {
    /// Sorted by name without case, so the shortest -- usually the one written -- comes first.
    pub(crate) names: Vec<(&'a str, &'a [Declarer])>,
    /// Each namespace, folded, with the claims on one name under it to read its spelling from.
    pub(crate) namespaces: BTreeMap<Box<str>, &'a [Declarer]>,
}

impl<'a> IndexView<'a> {
    /// The one walk over every declared name both halves of a path completion are read from.
    ///
    /// It used to be two, one per half, and the walk is the whole cost: a request at the end of a
    /// `use` line paid for every declared name twice. All three tables, since `\sprintf` is a
    /// function and `\E_ALL` a constant; a constant's name is not folded in its table, so it is
    /// compared without case here.
    pub(crate) fn under(&self, folded: &str) -> Under<'a> {
        let mut names = Vec::new();
        let mut namespaces: BTreeMap<Box<str>, &'a [Declarer]> = BTreeMap::new();
        for (name, declarers) in self.skeleton.owners().qualified_names() {
            let Some(head) = name.get(..folded.len()) else {
                continue;
            };
            if !head.eq_ignore_ascii_case(folded) {
                continue;
            }
            match name[folded.len()..].find('\\') {
                None => names.push((name, declarers)),
                Some(end) => {
                    let path = &name[..folded.len() + end];
                    let path: Cow<'_, str> =
                        match path.bytes().any(|byte| byte.is_ascii_uppercase()) {
                            true => Cow::Owned(path.to_ascii_lowercase()),
                            false => Cow::Borrowed(path),
                        };
                    if !namespaces.contains_key(path.as_ref()) {
                        namespaces.insert(path.into(), declarers);
                    }
                }
            }
        }
        names.sort_by(|(left, _), (right, _)| {
            left.bytes()
                .map(|byte| byte.to_ascii_lowercase())
                .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
        });
        Under { names, namespaces }
    }

    /// Every namespace that continues `prefix` up to its next separator, as declarations spell it.
    ///
    /// `prefix` is fully qualified with no leading separator, and may stop anywhere: `GuzzleHttp\Ex`
    /// continues into `GuzzleHttp\Exception`, `Gu` into `GuzzleHttp`, and an empty prefix into
    /// every top-level namespace. Compared without case, as PHP compares namespaces. At most
    /// `limit`.
    pub fn namespaces_continuing(&self, prefix: &str, limit: usize) -> Vec<String> {
        let under = self.under(&prefix.to_ascii_lowercase());
        self.spelled(under.namespaces, prefix, limit)
    }

    /// The namespaces under the file's own that a bare word begins: `Ty` inside
    /// `namespace Illuminate\JsonSchema` goes on into `Types`, which `Types\ArrayType` names.
    ///
    /// The other half of [`Self::path_continuations`], which refuses a bare word because a top-level
    /// namespace offered for one would be written into a path meaning something else. Relative is
    /// what a bare word is, so these are the namespaces it can begin: under the file's own, or -- in
    /// a file that declares none -- the top-level ones. Full paths, as declarations spell them.
    pub fn namespaces_beginning(
        &self,
        offset: phrost_model::TextSize,
        word: &str,
        limit: usize,
    ) -> Vec<String> {
        if word.contains('\\') {
            return Vec::new();
        }
        let Some(open) = self.current() else {
            return Vec::new();
        };
        // A range over the catalog's own namespace index rather than a walk over every qualified
        // name: a bare word is the commonest thing completion is asked about, and the walk cost
        // every one of them -- name completion p50 0.23 -> 0.40 ms on the reference project.
        let scope = open.scope().scope_at(offset);
        let parent = scope
            .name()
            .map(|namespace| {
                namespace
                    .as_str()
                    .trim_start_matches('\\')
                    .to_ascii_lowercase()
            })
            .unwrap_or_default();
        let mut found: Vec<String> = self
            .skeleton
            .owners()
            .namespaces_under(&parent, &word.to_ascii_lowercase())
            .into_iter()
            .map(|path| path.as_str().to_owned())
            // A segment the file imports is the import's: PHP resolves the first segment of a
            // qualified name through the `use` lines before the namespace, so `Support\X` under a
            // `use Other\Support;` is `Other\Support\X` whatever the file's own namespace holds.
            .filter(|path| {
                let segment = path.rsplit('\\').next().unwrap_or(path);
                scope
                    .imports()
                    .lookup(phrost_hir::ImportKind::ClassLike, segment)
                    .is_none()
            })
            .take(limit)
            .collect();
        // What was typed, in the case it was typed, first: the same order `spelled` gives.
        let segment = |path: &String| path.rsplit('\\').next().unwrap_or(path).to_owned();
        found.sort_by_key(|path| !segment(path).starts_with(word));
        found
    }

    /// What a qualified prefix written at `offset` goes on into -- the names under it and the
    /// namespaces past it -- from one walk per namespace the prefix can mean.
    ///
    /// Anchored with a leading separator, or resolved through the file's imports and namespace the
    /// way a complete name would be. Only a prefix holding a separator: a bare word in code is
    /// relative to the file's namespace, so a top-level namespace offered for it would be written
    /// into a path that means something else.
    pub fn path_continuations(
        &self,
        offset: phrost_model::TextSize,
        prefix: &str,
        kinds: NameKinds,
        limit: usize,
    ) -> PathContinuations {
        let mut found = PathContinuations::default();
        if !prefix.contains('\\') {
            return found;
        }
        // All three spellings, `\\App\\Repo`, `App\\Repo` and `namespace\\Repo` -- the third is how
        // PSL calls a function of its own namespace, and read as relative it named a namespace
        // called `namespace` and offered nothing.
        let (anchor, written) = phrost_hir::NameAnchor::of(prefix);
        let Some(open) = self.current() else {
            return found;
        };
        let resolution = open.scope().scope_at(offset).resolve(
            written,
            anchor,
            phrost_hir::ImportKind::ClassLike,
        );

        for candidate in resolution.candidates() {
            let resolved = candidate.fqn().trim_start_matches('\\');
            let under = self.under(&resolved.to_ascii_lowercase());
            found
                .names
                .extend(self.ranked_names(under.names, kinds, limit));
            for path in self.spelled(under.namespaces, resolved, limit) {
                if !found.namespaces.contains(&path) {
                    found.namespaces.push(path);
                }
            }
        }
        found.names.truncate(limit);
        found.namespaces.truncate(limit);
        found
    }

    /// The namespaces a walk found, spelled as a declaration under each writes them.
    ///
    /// What was typed in its own case first, the rest after, each half in name order. The match is
    /// folded, as PHP folds a namespace, but the case a person typed is still what they meant:
    /// `Drupal\Co` is reaching for `Drupal\Core`, and alphabetically twenty modules named
    /// `comment_*` stood in front of it.
    pub(crate) fn spelled(
        &self,
        namespaces: BTreeMap<Box<str>, &[Declarer]>,
        prefix: &str,
        limit: usize,
    ) -> Vec<String> {
        let mut spelled: Vec<String> = namespaces
            .into_iter()
            .take(limit)
            .filter_map(|(path, declarers)| {
                let declarer = declarers.first()?;
                let symbol = self
                    .skeleton
                    .file(declarer.file)?
                    .scope()
                    .symbols()
                    .get(declarer.symbol as usize)?;
                let written = symbol.fqn.as_str().trim_start_matches('\\');
                Some(written.get(..path.len())?.to_owned())
            })
            .collect();
        let typed = &prefix[prefix.rfind('\\').map_or(0, |at| at + 1)..];
        spelled.sort_by_key(|path| {
            let segment = &path[path.rfind('\\').map_or(0, |at| at + 1)..];
            !segment.starts_with(typed)
        });
        spelled
    }
}
