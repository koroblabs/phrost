use phrost_hir::{ImportKind, NameAnchor};

use crate::scope::{ImportBinding, NamespaceScope};

/// The fully-qualified names PHP would look for, in the order it would try them.
///
/// At most two, and the shape says why: PHP tries the current namespace, and for an unqualified
/// function or constant it then tries global. Nothing else is ever tried, so a `Vec` would invite
/// a third candidate that does not exist.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NameResolution {
    primary: Candidate,
    /// The global-namespace fallback, present only for an unqualified function or constant name.
    fallback: Option<Candidate>,
}

impl NameResolution {
    /// Candidates in the order PHP tries them.
    pub fn candidates(&self) -> impl Iterator<Item = &Candidate> {
        std::iter::once(&self.primary).chain(self.fallback.as_ref())
    }

    pub fn primary(&self) -> &Candidate {
        &self.primary
    }

    pub fn fallback(&self) -> Option<&Candidate> {
        self.fallback.as_ref()
    }
}

/// One fully-qualified candidate, and how it was arrived at.
///
/// The provenance is not decoration: goto on a name that resolved through an import should land on
/// the import, and rename has to know whether the `use` line needs rewriting.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    fqn: String,
    origin: CandidateOrigin,
}

impl Candidate {
    /// Fully qualified, with no leading separator.
    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn origin(&self) -> CandidateOrigin {
        self.origin
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CandidateOrigin {
    /// The name was already fully qualified.
    Absolute,
    /// A `use` statement supplied the prefix.
    Import(phrost_hir::ImportId),
    /// The enclosing namespace supplied the prefix.
    CurrentNamespace,
    /// The global namespace, reached either directly or as the fallback.
    Global,
}

impl NamespaceScope {
    /// Resolves a written name to the candidates PHP would look for.
    ///
    /// `path` is the name without a leading separator, as lowering stores it; `anchor` says how it
    /// was written; `table` says which of PHP's three symbol tables the lookup is for.
    ///
    /// Pure: no filesystem, no catalog, no notion of what exists. The caller checks the candidates
    /// against whatever it has indexed, in the order returned.
    pub fn resolve(&self, path: &str, anchor: NameAnchor, table: ImportKind) -> NameResolution {
        // Already absolute: imports and the current namespace are both irrelevant.
        if anchor == NameAnchor::FullyQualified {
            return NameResolution {
                primary: Candidate {
                    fqn: path.to_owned(),
                    origin: CandidateOrigin::Absolute,
                },
                fallback: None,
            };
        }

        // `namespace\Foo` asks explicitly for the current namespace and skips imports.
        if anchor == NameAnchor::CurrentNamespace {
            return NameResolution {
                primary: self.in_current_namespace(path),
                fallback: None,
            };
        }

        let qualified = path.split_once('\\');

        if let Some(binding) = self.import_of(path, anchor, table) {
            return NameResolution {
                primary: Candidate {
                    // A qualified name keeps every segment after the one the import supplied:
                    // `use App\Support;` turns `Support\render` into `App\Support\render`.
                    fqn: match qualified {
                        Some((_, rest)) => format!("{}\\{rest}", binding.path()),
                        None => binding.path().as_str().to_owned(),
                    },
                    origin: CandidateOrigin::Import(binding.import()),
                },
                fallback: None,
            };
        }

        // No import supplied a prefix, so the current namespace does. An unqualified function or
        // constant then falls back to global; an unqualified class never does — adding it would
        // resolve `new Repo()` inside a namespace to a global class PHP would not have found — and
        // a qualified name never does either, whatever table it is for.
        let falls_back_to_global =
            qualified.is_none() && matches!(table, ImportKind::Function | ImportKind::Constant);

        NameResolution {
            primary: self.in_current_namespace(path),
            fallback: (falls_back_to_global && !self.is_global()).then(|| Candidate {
                fqn: path.to_owned(),
                origin: CandidateOrigin::Global,
            }),
        }
    }

    /// The import a written name is bound through, if one is.
    ///
    /// The import half of [`Self::resolve`], which delegates to it — so the two cannot disagree
    /// about which `use` a name reaches. Separate because an unused-import check asks only *which*
    /// import a name reached: it never reads the fully-qualified name resolution would build, and
    /// building one per written name in a file is a few hundred allocations it throws away.
    ///
    /// A multi-segment name's first segment goes through the **class-like** table whatever the
    /// lookup is for, which is what makes `use App\Support; Support\render();` work.
    pub fn import_of(
        &self,
        path: &str,
        anchor: NameAnchor,
        table: ImportKind,
    ) -> Option<&ImportBinding> {
        // `\App\Repo` is already absolute and `namespace\Repo` asks explicitly for the current
        // namespace; neither consults an import.
        if anchor != NameAnchor::Relative {
            return None;
        }

        match path.split_once('\\') {
            Some((first, _)) => self.imports().lookup(ImportKind::ClassLike, first),
            None => self.imports().lookup(table, path),
        }
    }

    fn in_current_namespace(&self, path: &str) -> Candidate {
        match self.name() {
            Some(prefix) => Candidate {
                fqn: format!("{prefix}\\{path}"),
                origin: CandidateOrigin::CurrentNamespace,
            },
            // In the global namespace the two are the same thing, and saying so keeps a caller
            // from having to special-case an empty prefix.
            None => Candidate {
                fqn: path.to_owned(),
                origin: CandidateOrigin::Global,
            },
        }
    }
}
