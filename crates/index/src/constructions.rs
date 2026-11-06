//! What the objects a named method builds declare: Twig's filters, functions and tests.
//!
//! `new TwigFilter('t', 't')` in an extension's `getFilters()` is the only statement anywhere that
//! the filter `t` exists and runs the function `t()`. Lowering keeps each such construction --
//! see `KeyedAttributes::keeping_constructions` -- and this resolves it against the file's imports
//! at ingest and stores it with the file, the way a property default naming a class is stored: it
//! comes out of a body, which a closed file does not keep.

use phrost_defmap::{DeclarationSource, FileScope};
use phrost_hir::{FileHir, ImportKind, KeptCallableData};
use phrost_model::TextRange;

use crate::{ProjectSkeleton, returns::ReturnContracts};

/// One entry an object's construction declares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConstructionSite {
    /// The catalog the entry is a key of, by the rule naming the class constructed.
    pub catalog: Box<str>,
    pub key: Box<str>,
    /// Where the key is written, inside its quotes.
    pub key_range: TextRange,
    /// The class declaring the method that builds it, fully qualified.
    pub declarer: Box<str>,
    pub callable: Callable,
}

/// What an entry runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Callable {
    /// A function, fully qualified without its leading separator.
    Function(Box<str>),
    /// A method of a class, the class fully qualified.
    Method { class: Box<str>, method: Box<str> },
    /// Something nothing can name: a closure, or a node the compiler builds.
    Unnamed,
}

impl lspkit_memsize::MemSize for ConstructionSite {
    fn heap_size(&self) -> usize {
        self.catalog.len()
            + self.key.len()
            + self.declarer.len()
            + match &self.callable {
                Callable::Function(name) => name.len(),
                Callable::Method { class, method } => class.len() + method.len(),
                Callable::Unnamed => 0,
            }
    }
}

impl ProjectSkeleton {
    /// Every kept construction in this file of a class a rule names.
    pub(crate) fn collect_constructions(
        returns: &ReturnContracts,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Box<[ConstructionSite]> {
        let rules = returns.constructed_entries();
        if rules.is_empty() {
            return Box::default();
        }
        hir.kept_constructions()
            .iter()
            .filter_map(|kept| {
                let declarer = scope.symbol_of(DeclarationSource::Class(kept.class))?;
                let namespace = scope.scope_of(hir.class(kept.class).namespace);
                let resolve = |name: phrost_hir::NameRefId| -> Box<str> {
                    let written = hir.name_ref(name);
                    namespace
                        .resolve(written.path.as_str(), written.anchor, ImportKind::ClassLike)
                        .primary()
                        .fqn()
                        .into()
                };
                let constructed = resolve(kept.constructed);
                let catalog = rules
                    .iter()
                    .find(|(class, _)| class.eq_ignore_ascii_case(&constructed))
                    .map(|(_, catalog)| catalog.clone())?;
                let declarer: Box<str> = declarer.fqn.as_str().into();
                let callable = match &kept.callable {
                    KeptCallableData::Function(name) => {
                        Callable::Function(name.as_str().trim_start_matches('\\').into())
                    }
                    KeptCallableData::Method { class, method } => Callable::Method {
                        class: class.map_or_else(|| declarer.clone(), resolve),
                        method: method.as_str().into(),
                    },
                    KeptCallableData::NamedMethod { class, method } => Callable::Method {
                        class: class.as_str().trim_start_matches('\\').into(),
                        method: method.as_str().into(),
                    },
                    KeptCallableData::Unnamed => Callable::Unnamed,
                };
                Some(ConstructionSite {
                    catalog,
                    key: kept.key.as_str().into(),
                    key_range: kept.key_range,
                    declarer,
                    callable,
                })
            })
            .collect()
    }
}
