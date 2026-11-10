//! Every name the current file writes, and whether it names anything.
//!
//! The counterpart of [`member_accesses`](IndexView::member_accesses) for class-like names: a
//! whole-file projection rather than a position query, for anything that has to visit all of them.
//! Checking each one is the first use; listing where a class is referenced is the next.

use phrost_hir::ImportKind;
use phrost_model::{Name, TextRange};

use crate::view::IndexView;

/// One class-like name written in the file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NameReference {
    /// The name alone -- what a cursor lands on and what a diagnostic underlines.
    pub range: TextRange,
    /// The path as written, without a leading separator.
    pub path: Name,
    /// The declaration it resolves to, fully qualified. `None` when it resolves to nothing.
    ///
    /// The resolved name rather than a boolean, because that is what tells a reference to `Repo`
    /// apart from a reference to a different `Repo` two namespaces over -- and telling those apart is
    /// the whole job of finding references.
    pub resolved: Option<Name>,
    /// Written in a docblock rather than in code.
    ///
    /// Carried because a missing name means different things in the two places: in code it is a
    /// fatal error, in a docblock a wrong annotation -- and the docblock type language has names PHP
    /// does not, a `@template` parameter first among them.
    pub in_docblock: bool,
    /// The file asks somewhere whether this name is declared.
    ///
    /// `class_exists(Relay::class)` is an author saying the class may be absent and writing the
    /// branch for it, so a report that it is absent says what they already handled. See
    /// [`phrost_body::existence_tests`] for why the test need not enclose the use.
    pub existence_tested: bool,
    /// A `@see` pointing at the name -- see `phrost_hir::NameRefData::cross_reference`.
    pub cross_reference: bool,
}

/// What kind of name a docblock declared for its own type language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocblockName {
    /// `@template T`, in any of its spellings.
    Template,
    /// `@phpstan-type Row array{...}`, Psalm's form, or one imported from another class.
    TypeAlias,
}

impl IndexView<'_> {
    /// Every template parameter and type alias a docblock in the current file declares.
    ///
    /// The file rather than the declaration around a name, so a name one declaration's template
    /// spells is never reported in another's docblock either -- the quiet direction, for a check
    /// about names in the docblock language, where a template is the commonest name of all. Aliases
    /// for the same reason: one declared on a method, or in Psalm's `Name = type` form, is not one
    /// the class's alias table holds, and it is still no class.
    pub fn templates_declared(&self) -> Vec<String> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let mut names = open
            .hir()
            .docblocks()
            .flat_map(|(_, docblock)| {
                docblock
                    .template_names()
                    .chain(docblock.alias_names())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    /// The template parameters and type aliases a docblock at `offset` can name: what the docblock
    /// the offset is in declares, and what the docblock of the class around it does.
    ///
    /// The scope and not the file, unlike [`Self::templates_declared`] beside it. That one keeps a
    /// check quiet, where naming too much is the safe side; this one is offered to a person, and a
    /// template of some other class in the file is a name that means nothing where they are.
    /// In the order they are declared, the docblock's own first.
    pub fn templates_in_scope(
        &self,
        offset: phrost_model::TextSize,
    ) -> Vec<(String, DocblockName)> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let own = hir
            .docblocks()
            .find(|(_, docblock)| docblock.range.contains_inclusive(offset))
            .map(|(id, _)| id);
        let of_the_class = hir
            .classes()
            .filter(|(_, class)| class.full_range.contains_inclusive(offset))
            .min_by_key(|(_, class)| class.full_range.len())
            .and_then(|(_, class)| class.docblock);

        let mut names: Vec<(String, DocblockName)> = Vec::new();
        for id in [own, of_the_class.filter(|class| Some(*class) != own)]
            .into_iter()
            .flatten()
        {
            let docblock = hir.docblock(id);
            let declared = docblock
                .template_names()
                .map(|name| (name, DocblockName::Template))
                .chain(
                    docblock
                        .alias_names()
                        .map(|name| (name, DocblockName::TypeAlias)),
                );
            for (name, kind) in declared {
                if !names.iter().any(|(seen, _)| seen == name) {
                    names.push((name.to_owned(), kind));
                }
            }
        }
        names
    }

    /// Every class-like name written in the current file, in source order.
    ///
    /// Class-like only. Functions and constants are separate symbol tables in PHP, and this index
    /// has no catalog for constants at all -- so reporting on them would be reporting on a gap here
    /// rather than on anything in the file.
    ///
    /// `use` statements are included: importing a class that does not exist is the same defect as
    /// naming one, and it is the one a refactor leaves behind most often.
    pub fn name_references(&self) -> Vec<NameReference> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let tested = phrost_body::existence_tests(hir);
        let mut out = Vec::new();

        for (_, reference) in hir.name_refs() {
            if reference.kind != ImportKind::ClassLike {
                continue;
            }
            // `new class { ... }` names its own class so that the value has a type, and the name is
            // synthesised from where the `class` keyword sits. Nobody wrote it, so it is not a
            // reference a reader can act on: not a report when it resolves to nothing, not a rename
            // target, not a result in find-references.
            if phrost_hir::is_anonymous_class_name(reference.path.as_str()) {
                continue;
            }
            out.push(NameReference {
                range: reference.range,
                path: reference.path.clone(),
                resolved: self
                    .resolve_written_name(
                        reference.range.start(),
                        reference.path.as_str(),
                        reference.anchor,
                    )
                    .map(|location| location.fqn),
                in_docblock: reference.in_docblock,
                existence_tested: tested.tests_class(reference.path.as_str()),
                cross_reference: reference.cross_reference,
            });
        }

        for (_, import) in hir.imports() {
            if import.kind != ImportKind::ClassLike {
                continue;
            }
            // An import path is absolute, so nothing about where it sits changes what it means.
            // It may also name a *namespace* rather than a class: `use Symfony\...\Constraints;`
            // followed by `Constraints\NotBlank` is ordinary PHP, and with attributes it is the
            // common form.
            let resolved = self
                .class_location(import.path.as_str())
                .map(|location| location.fqn)
                // A `use` may name a namespace rather than a class; nothing declares the namespace
                // itself, so there is no name to report, only that the import is not broken.
                .or_else(|| {
                    self.is_namespace(import.path.as_str())
                        .then(|| import.path.clone())
                });

            out.push(NameReference {
                range: import.path_range,
                path: import.path.clone(),
                resolved,
                in_docblock: false,
                existence_tested: tested.tests_class(import.path.as_str()),
                cross_reference: false,
            });
        }

        out.sort_by_key(|reference| reference.range.start());
        out
    }

    /// Whether anything is declared under this name as a namespace.
    ///
    /// A namespace is declared by nothing of its own, so what says one exists is that something sits
    /// under it -- and *anything* does: a namespace holding only functions is how a library written
    /// in that style spells a module. `use Psl\Crypto\Internal;` beside `Internal\call_sodium(..)`
    /// is 40 reports of an undefined type on one such library, on imports that are correct.
    ///
    /// A scan of every declared name, so it is asked only after a class lookup has already missed --
    /// which on a real project is a handful of times per run, against the hundreds of thousands of
    /// names a scan would otherwise walk per import.
    fn is_namespace(&self, path: &str) -> bool {
        let prefix = format!("{}\\", path.to_ascii_lowercase());
        self.skeleton
            .owners()
            .declared_names()
            .any(|name| name.as_str().starts_with(&prefix))
    }
}
