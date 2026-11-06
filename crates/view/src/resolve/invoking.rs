//! What invoking a value gives back, when its *type* says.
//!
//! `@param callable(Item): string $fn` followed by `$fn($item)`. The claim is in a type rather than
//! in a value, which is what makes this its own read: a closure literal and a first-class callable
//! both arrive as a [`Held::Callable`] carrying their return, and a docblock's callable arrives as
//! an ordinary type node that happens to hold one argument.
//!
//! Kept apart from [`super::element`] because the question is different. Iterating a container asks
//! what it holds; this asks what calling it produces, and a callable is not a container -- `foreach
//! ($make as $row)` iterates the closure, not the query.

use phrost_hir::{FileHir, TypeRefData, TypeRefId};
use phrost_model::BuiltinTy;
use phrost_ty::TypeId;

use crate::{resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What invoking this value gives back, when the value's type says so.
    ///
    /// `None` for everything else, which is the honest answer and also the cheap one: a receiver
    /// that is not a callable-with-a-return costs one match arm, and the caller then goes on to
    /// `__invoke` exactly as it did.
    ///
    /// Three spellings, because a callable arrives as all three: built by resolution when a closure
    /// was written, interned when the declaration is in another file, and written into this file's
    /// arena when it is here. A rule taught to one of them governs the shape it was tested with and
    /// nothing else -- which is how `@param callable(Foo): Bar` came to be the only callable that
    /// answered nothing while a closure literal and `strlen(...)` both worked.
    pub(crate) fn invoked_return(&self, hir: &FileHir, held: &Held) -> Option<Held> {
        match held {
            Held::Callable(callable) => Some(callable.returns.clone()),
            // A null is not callable, and what the author meant to call is what is inside.
            Held::Nullable(inner) => self.invoked_return(hir, inner),
            Held::Written(id) => Self::written_callable_return(hir, *id).map(Held::Written),
            // The scope travels with the return: a `callable(): static` means the same class it
            // meant where it was declared.
            Held::Stored { ty, scope } => Some(Held::Stored {
                ty: self.stored_callable_return(*ty)?,
                scope: scope.clone(),
            }),
            _ => None,
        }
    }

    /// The return argument of a `callable<T>` written in this file's arena.
    fn written_callable_return(hir: &FileHir, id: TypeRefId) -> Option<TypeRefId> {
        match hir.type_ref(id) {
            TypeRefData::Nullable(inner) => Self::written_callable_return(hir, *inner),
            TypeRefData::Generic { base, args, .. }
                if matches!(
                    hir.type_ref(*base),
                    TypeRefData::Builtin(BuiltinTy::Callable)
                ) =>
            {
                hir.type_ref_list(*args).first().copied()
            }
            _ => None,
        }
    }

    /// The same, interned.
    pub(crate) fn stored_callable_return(&self, id: TypeId) -> Option<TypeId> {
        let types = self.skeleton.types();
        match types.get(id) {
            phrost_ty::TypeRef::Nullable(inner) => self.stored_callable_return(*inner),
            phrost_ty::TypeRef::Generic { base, args, .. }
                if matches!(
                    types.get(*base),
                    phrost_ty::TypeRef::Builtin(BuiltinTy::Callable)
                ) =>
            {
                types.operands(*args).first().copied()
            }
            _ => None,
        }
    }
}
