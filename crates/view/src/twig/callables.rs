//! The filters, functions and tests a Twig template calls by name, and what runs each.
//!
//! An extension declares each by constructing it -- `new TwigFilter('t', 't')` -- and the index keeps
//! every such construction, Twig's own and a dependency's included (see `phrost_index::constructions`).
//! So a name in a template is a key of the catalog the Twig provider named for its kind, and what
//! runs it is a PHP function or method the engine already knows: where it is, what it says it
//! returns, what its docblock says.

use phrost_index::constructions::{Callable, ConstructionSite};
use phrost_model::{FileId, TextRange};
use phrost_sema::MemberKind;
use phrost_twig::CalledKind;

use super::variables::{TwigCandidate, TwigCandidateKind};
use crate::{location::SymbolLocation, resolve::Held, view::IndexView};

/// Which of the three a template calls by name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TwigCallableKind {
    Filter,
    Function,
    Test,
}

impl TwigCallableKind {
    pub(super) fn of(kind: CalledKind) -> Self {
        match kind {
            CalledKind::Filter => Self::Filter,
            CalledKind::Function => Self::Function,
            CalledKind::Test => Self::Test,
        }
    }
}

/// What a filter, function or test a template calls is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TwigCallable {
    pub kind: TwigCallableKind,
    pub name: Box<str>,
    /// The class whose extension declares it, and where its name is written there.
    pub declarer: Box<str>,
    pub declared: (FileId, TextRange),
    /// The PHP that runs it, when a function or a method does -- `None` for a closure, or for one
    /// the compiler writes in place.
    pub runs: Option<Box<SymbolLocation>>,
}

impl IndexView<'_> {
    /// The construction declaring the callable of this kind and name, the first in file order.
    fn twig_construction(
        &self,
        kind: TwigCallableKind,
        name: &str,
    ) -> Option<(FileId, &ConstructionSite)> {
        let catalog = self.twig_catalog(kind)?;
        // Twig's own names are case-sensitive, and so is this.
        self.skeleton
            .constructions_in(catalog)
            .into_iter()
            .find(|(_, site)| *site.key == *name)
    }

    fn twig_catalog(&self, kind: TwigCallableKind) -> Option<&str> {
        let [filters, functions, tests] = self.skeleton.returns().twig_callables()?;
        Some(match kind {
            TwigCallableKind::Filter => filters,
            TwigCallableKind::Function => functions,
            TwigCallableKind::Test => tests,
        })
    }

    /// What the filter, function or test called `name` is.
    pub(super) fn twig_callable(&self, kind: TwigCallableKind, name: &str) -> Option<TwigCallable> {
        let (file, site) = self.twig_construction(kind, name)?;
        Some(TwigCallable {
            kind,
            name: name.into(),
            declarer: site.declarer.clone(),
            declared: (file, site.key_range),
            runs: self.callable_location(&site.callable).map(Box::new),
        })
    }

    fn callable_location(&self, callable: &Callable) -> Option<SymbolLocation> {
        match callable {
            Callable::Function(name) => self.function_location(name),
            Callable::Method { class, method } => {
                self.member_location(class, MemberKind::Method, method)
            }
            Callable::Unnamed => None,
        }
    }

    /// What the callable of this kind and name returns, as its PHP declares.
    pub(super) fn twig_callable_returns(&self, kind: TwigCallableKind, name: &str) -> Option<Held> {
        match &self.twig_construction(kind, name)?.1.callable {
            Callable::Function(function) => Some(Held::Stored {
                ty: self.function_return(function)?,
                scope: None,
            }),
            Callable::Method { class, method } => {
                self.member_held(class, MemberKind::Method, method)
            }
            Callable::Unnamed => None,
        }
    }

    /// Every callable of this kind a template can call, each once, with what runs it beside it.
    pub(super) fn twig_callable_names(&self, kind: TwigCallableKind) -> Vec<TwigCandidate> {
        let Some(catalog) = self.twig_catalog(kind) else {
            return Vec::new();
        };
        let mut found: Vec<TwigCandidate> = Vec::new();
        for (_, site) in self.skeleton.constructions_in(catalog) {
            if found.iter().any(|known| *known.name == *site.key) {
                continue;
            }
            found.push(TwigCandidate {
                name: site.key.clone(),
                detail: match &site.callable {
                    Callable::Function(function) => Some(format!("{function}()").into()),
                    Callable::Method { class, method } => {
                        Some(format!("{}::{method}()", phrost_model::short_name(class)).into())
                    }
                    Callable::Unnamed => None,
                },
                kind: match kind {
                    TwigCallableKind::Filter => TwigCandidateKind::Filter,
                    TwigCallableKind::Function | TwigCallableKind::Test => {
                        TwigCandidateKind::Function
                    }
                },
            });
        }
        found.sort_by(|left, right| left.name.cmp(&right.name));
        found
    }
}
