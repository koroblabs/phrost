//! The names a file gives an imported class, and everywhere it writes them.
//!
//! `use Domain\ValueObject\UUIDv7 as QuoteId;` makes `QuoteId` a name that exists in one file and
//! nowhere else. It is not a declaration -- nothing is declared -- and it is not a reference to
//! `UUIDv7` either, because replacing it with whatever `UUIDv7` is renamed to leaves the `as
//! QuoteId` binding still saying `QuoteId`. It is a third thing, and it needs a projection of its
//! own or every reader has to re-derive it from imports and get it subtly wrong.
//!
//! The bug that says why: a rename asked at a use of `QuoteId` promised the editor it would edit
//! `"QuoteId"` and replaced `UUIDv7` at thirty sites in six files instead, including inside the
//! class's own file.

use phrost_hir::{ImportKind, NameAnchor};
use phrost_model::{Name, TextRange, TextSize};

use crate::view::IndexView;

/// A `use X as Y;` binding: the file's own name for a class declared somewhere else.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AliasBinding {
    /// The local name, `Y`. It means this class in this file and nothing anywhere else.
    pub name: Name,
    /// The imported path, `X`, as the `use` line wrote it.
    pub path: Name,
    /// The alias token itself, which is where the name comes into being.
    pub range: TextRange,
}

impl IndexView<'_> {
    /// The explicit alias the name at `offset` is a use of, or `None` when the name written there
    /// is not one.
    ///
    /// Both positions a person can ask from: the alias token in the `use` line, and any later use
    /// of the short name it bound. Only an *explicit* alias -- `use App\Repo;` binds `Repo` too,
    /// but to a name the author never typed, so there is nothing there a rename could edit.
    pub fn alias_at(&self, offset: TextSize) -> Option<AliasBinding> {
        let open = self.current()?;
        let hir = open.hir();

        for (_, import) in hir.imports() {
            if import.kind != ImportKind::ClassLike {
                continue;
            }
            let (Some(name), Some(range)) = (import.alias.as_ref(), import.alias_range) else {
                continue;
            };
            if range.contains_inclusive(offset) {
                return Some(AliasBinding {
                    name: name.clone(),
                    path: import.path.clone(),
                    range,
                });
            }
        }

        // A use of the alias is written as an ordinary name; what makes it an alias is the head
        // segment matching a binding in this file. Narrowest first, the same rule every other
        // position lookup here uses.
        let written = hir
            .name_refs()
            .filter(|(_, reference)| reference.kind == ImportKind::ClassLike)
            .filter(|(_, reference)| reference.anchor == NameAnchor::Relative)
            .filter(|(_, reference)| reference.range.contains_inclusive(offset))
            .min_by_key(|(_, reference)| reference.range.len())?
            .1;

        // On the head and not merely inside the path. In `N\Repo` where `N` is the alias, the
        // cursor on `Repo` is on a class name that happens to be reached through an alias -- a
        // different question with a different answer, and treating it as the alias would have the
        // rename edit `N` while the person was looking at `Repo`.
        if !head_range(open.text(), written.range).contains_inclusive(offset) {
            return None;
        }

        self.alias_named(head_of(written.path.as_str()))
    }

    /// The binding for a local name, when this file bound it with an explicit `as`.
    fn alias_named(&self, name: &str) -> Option<AliasBinding> {
        let hir = self.current()?.hir();
        hir.imports()
            .filter(|(_, import)| import.kind == ImportKind::ClassLike)
            .find_map(|(_, import)| {
                let alias = import.alias.as_ref()?;
                if !alias.as_str().eq_ignore_ascii_case(name) {
                    return None;
                }
                Some(AliasBinding {
                    name: alias.clone(),
                    path: import.path.clone(),
                    range: import.alias_range?,
                })
            })
    }

    /// Every range in the current file that writes `alias`, its own `use` line included.
    ///
    /// The head segment only. `use App\Nested as N;` followed by `N\Repo` writes the alias in the
    /// first segment and a class name in the second, and a range covering both would have the
    /// rename replace `Repo`.
    ///
    /// One file, always: an alias is bound by a `use` line and a `use` line reaches nothing outside
    /// the file it is written in. A sweep for the spelling would rewrite an unrelated class of the
    /// same short name in every other file.
    pub fn alias_occurrences(&self, alias: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let text = open.text();
        let mut found = Vec::new();

        for (_, import) in hir.imports() {
            if import.kind != ImportKind::ClassLike {
                continue;
            }
            if import
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str().eq_ignore_ascii_case(alias))
            {
                found.extend(import.alias_range);
            }
        }

        for (_, reference) in hir.name_refs() {
            if reference.kind != ImportKind::ClassLike
                || reference.anchor != NameAnchor::Relative
                || !head_of(reference.path.as_str()).eq_ignore_ascii_case(alias)
            {
                continue;
            }
            found.push(head_range(text, reference.range));
        }

        found.sort_by_key(|range| range.start());
        found.dedup();
        found
    }
}

/// The first segment of a written path, which is the only one an import can have bound.
fn head_of(path: &str) -> &str {
    path.split('\\').next().unwrap_or(path)
}

/// The part of a written name its head segment occupies.
fn head_range(text: &str, range: TextRange) -> TextRange {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let Some(at) = text.get(start..end).and_then(|written| written.find('\\')) else {
        return range;
    };
    TextRange::new(
        range.start(),
        range.start() + TextSize::try_from(at).unwrap_or_default(),
    )
}

/// A name this file bound with `as`, offered where a name is being written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AliasCandidate {
    /// The local name, which is what gets written.
    pub name: Name,
    /// What it means: the declaration the import reaches.
    pub fqn: Name,
    pub kind: phrost_model::SymbolKind,
}

impl IndexView<'_> {
    /// The explicit aliases this file binds whose name matches what was typed, and that the place
    /// can take.
    ///
    /// A name completion is drawn from the catalog, which holds declarations -- and an alias is not
    /// one, so `use Illuminate\Support\Collection as BaseCollection;` made a name the file writes
    /// that completion had never heard of. Laravel aliases imports throughout, and `new BaseColl`
    /// offered nothing there. Only a name the import resolves: an alias of something the index does
    /// not hold is a name nothing could be said about.
    pub fn aliases_matching(&self, typed: &str, kinds: crate::NameKinds) -> Vec<AliasCandidate> {
        self.bound_by_import(typed, kinds, true)
    }

    /// The declarations this file imports under their own names, where the name matches what was
    /// typed and the place can take it.
    ///
    /// The other half of [`Self::aliases_matching`], and what the catalog cannot say about its own
    /// rows: *which* class the file means by a name several declare. `RuntimeException` is PHP's
    /// and the MongoDB extension's; a file that writes `use RuntimeException;` has chosen, and
    /// among the two rows for `new RuntimeE` the one it did not choose stood first -- they were
    /// ordered by where each is declared, and `mongodb.php` sorts before `spl.php`.
    pub fn imported_as_written(&self, typed: &str, kinds: crate::NameKinds) -> Vec<AliasCandidate> {
        self.bound_by_import(typed, kinds, false)
    }

    /// The names an import of this file binds, by whether they were bound with `as`.
    fn bound_by_import(
        &self,
        typed: &str,
        kinds: crate::NameKinds,
        aliased: bool,
    ) -> Vec<AliasCandidate> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        open.hir()
            .imports()
            .filter(|(_, import)| import.alias.is_some() == aliased)
            .filter_map(|(_, import)| {
                if !phrost_model::matches_loosely(import.local.as_str(), typed) {
                    return None;
                }
                let path = import.path.as_str().trim_start_matches('\\');
                let located = match import.kind {
                    ImportKind::ClassLike => self.class_location(path),
                    ImportKind::Function => self.function_location(path),
                    ImportKind::Constant => self.constant_location(path),
                }?;
                kinds.accepts(located.kind).then(|| AliasCandidate {
                    name: import.local.clone(),
                    fqn: located.fqn,
                    kind: located.kind,
                })
            })
            .collect()
    }
}

impl IndexView<'_> {
    /// The namespaces this file imported by name that match a bare word being typed: `use Psl\Graph;`
    /// makes `Graph` the way to write `Graph\add_edge()`.
    ///
    /// A `use` naming a namespace binds its last segment exactly as one naming a class does, and
    /// PSL is written that way throughout -- `Graph\`, `TCP\`, `Async\`, `Network\` -- so its code
    /// offered nothing at the first segment of nearly every call. Only an import that names no
    /// class: one that does is the catalog's to offer, under the class's own name.
    ///
    /// Answered as the local name, which is what gets written, and the path it stands for.
    pub fn imported_namespaces_matching(&self, typed: &str) -> Vec<(Name, String)> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        open.hir()
            .imports()
            .filter(|(_, import)| import.kind == ImportKind::ClassLike)
            .filter(|(_, import)| phrost_model::matches_loosely(import.local.as_str(), typed))
            .filter_map(|(_, import)| {
                let path = import.path.as_str().trim_start_matches('\\');
                if self.class_location(path).is_some() {
                    return None;
                }
                // Asked of the catalog's namespace index, which holds a path exactly when
                // something nameable sits under it -- a lookup, where this was a walk over every
                // qualified name for each import a bare word matched.
                self.skeleton
                    .owners()
                    .holds_namespace(&path.to_ascii_lowercase())
                    .then(|| (import.local.clone(), path.to_owned()))
            })
            .collect()
    }
}
