//! The class a property's default names: `protected $model = User::class;`.
//!
//! Laravel's factory says which model it makes this way, and the property's declared type --
//! `class-string<TModel>|null` on the framework's class, `string` in the docblock a factory copies --
//! says nothing about which. The default is the statement, and a default is only in the body of the
//! declaration, so it is read at ingest and stored with the file, for the reason every such fact is
//! (see `stated.rs`). Which properties are kept at all is what the framework's rules name, through
//! `KeyedAttributes`.

use phrost_defmap::{DeclarationSource, FileScope};
use phrost_hir::{FileHir, ImportKind};

use crate::ProjectSkeleton;

/// One property default naming a class.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassDefault {
    /// The class declaring the property, fully qualified.
    pub declarer: Box<str>,
    pub property: Box<str>,
    /// The class the default names, resolved against the file's imports.
    pub class: Box<str>,
}

impl lspkit_memsize::MemSize for ClassDefault {
    fn heap_size(&self) -> usize {
        self.declarer.len() + self.property.len() + self.class.len()
    }
}

impl ProjectSkeleton {
    /// Every kept property default in this file that names a class.
    pub(crate) fn collect_class_defaults(scope: &FileScope, hir: &FileHir) -> Box<[ClassDefault]> {
        hir.kept_classes()
            .iter()
            .filter_map(|kept| {
                let declarer = scope.symbol_of(DeclarationSource::Class(kept.class))?;
                let namespace = scope.scope_of(hir.class(kept.class).namespace);
                let written = hir.name_ref(kept.named);
                let class = namespace
                    .resolve(written.path.as_str(), written.anchor, ImportKind::ClassLike)
                    .primary()
                    .fqn()
                    .into();
                Some(ClassDefault {
                    declarer: declarer.fqn.as_str().into(),
                    property: kept.property.as_str().into(),
                    class,
                })
            })
            .collect()
    }
}
