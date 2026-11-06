//! Type arguments that cannot be what a generic declaration asks for.
//!
//! The judgement in `phrost_ty::compat` drops type arguments on both sides: `Box<string>` where
//! `Box<int>` is declared is a `Box` where a `Box` is wanted, and fits. Comparing arguments for real
//! is a variance question -- a template is invariant unless declared otherwise, and most libraries
//! declare nothing while meaning covariance -- so read strictly it would report every
//! `Collection<User>` handed to a `Collection<Model>`.
//!
//! What is reported instead is the case no variance saves: two arguments **no value can be both
//! of**. Two different scalar kinds, a scalar against an object, two classes each refusing the
//! other. `int` against `float` is left alone, since PHP converts one into the other, and anything
//! this cannot classify -- a template passed along, a union, `mixed` -- is silence.

use phrost_hir::{FileHir, TypeRefData};
use phrost_model::{BuiltinTy, Name, TextSize};
use phrost_ty::{TypeId, TypeRef, compat::Fit};

use crate::{IndexView, compat::Expectation, generics::Landing, resolve::Held};

/// What one type argument is, as far as telling two apart needs.
#[derive(Clone, PartialEq, Eq, Debug)]
enum ArgumentKind {
    Scalar(BuiltinTy),
    Class(Name),
    Unknown,
}

impl ArgumentKind {
    fn of_builtin(builtin: BuiltinTy) -> Self {
        // What a string holds tells two arguments apart no more than a kind does.
        let builtin = builtin.unrefined();
        match builtin {
            BuiltinTy::Int | BuiltinTy::Float | BuiltinTy::String | BuiltinTy::Bool => {
                Self::Scalar(builtin)
            }
            BuiltinTy::True | BuiltinTy::False => Self::Scalar(BuiltinTy::Bool),
            _ => Self::Unknown,
        }
    }
}

impl IndexView<'_> {
    /// Whether `held`, whose class already fits `expected`, carries a type argument no value of the
    /// one `expected` declares can be.
    pub(crate) fn type_arguments_refused(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        expected: Expectation,
    ) -> bool {
        let Some((class, declared)) = self.declared_arguments(offset, hir, expected) else {
            return false;
        };
        if self
            .class_templates(class.as_str())
            .is_none_or(|templates| templates.len() != declared.len())
        {
            return false;
        }
        let carried = self.carried_arguments(offset, hir, held, &class, declared.len());
        carried
            .iter()
            .zip(&declared)
            .any(|(value, wanted)| self.disjoint(value, wanted))
    }

    /// The class a generic expectation names, and what it declares for each of its templates.
    fn declared_arguments(
        &self,
        offset: TextSize,
        hir: &FileHir,
        expected: Expectation,
    ) -> Option<(Name, Vec<ArgumentKind>)> {
        match expected {
            Expectation::Stored(ty) => {
                let types = self.skeleton.types();
                let ty = match types.get(ty) {
                    TypeRef::Nullable(inner) => *inner,
                    _ => ty,
                };
                let TypeRef::Generic { base, args, .. } = types.get(ty) else {
                    return None;
                };
                let class = self.class_of_stored(*base, None)?;
                let kinds = types
                    .operands(*args)
                    .iter()
                    .map(|&argument| self.stored_argument(argument))
                    .collect();
                Some((class, kinds))
            }
            Expectation::Written(id) => {
                let TypeRefData::Generic { base, args, .. } = hir.type_ref(id) else {
                    return None;
                };
                let class = self.class_of_type_ref(offset, hir, *base)?;
                let kinds = hir
                    .type_ref_list(*args)
                    .iter()
                    .map(|&argument| match hir.type_ref(argument) {
                        TypeRefData::Builtin(builtin) => ArgumentKind::of_builtin(*builtin),
                        TypeRefData::Named(_) => self
                            .class_of_type_ref(offset, hir, argument)
                            .filter(|class| self.class_location(class.as_str()).is_some())
                            .map_or(ArgumentKind::Unknown, ArgumentKind::Class),
                        _ => ArgumentKind::Unknown,
                    })
                    .collect();
                Some((class, kinds))
            }
        }
    }

    /// Whether a closure literal declares a return no value of the one `expected`'s signature
    /// promises can be: `fn (int $v): int` where `callable(int): string` is declared.
    ///
    /// Only the declared return. What an arrow function's body yields is inferred, and a guess is
    /// not what a report should stand on. The parameters are [`Self::callback_parameter_refused`]'s,
    /// because a declaration keeps them in a slot of their own rather than in the type.
    pub(crate) fn callable_return_refused(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        expected: Expectation,
    ) -> bool {
        let Held::Callable(callable) = held else {
            return false;
        };
        let Some(literal) = callable.literal else {
            return false;
        };
        let phrost_hir::body::ExprData::Closure {
            return_type: Some(written),
            ..
        } = hir.expr(literal)
        else {
            return false;
        };
        let Some(promised) = self.signature_return(offset, hir, expected) else {
            return false;
        };
        let declared = match hir.type_ref(*written) {
            TypeRefData::Builtin(builtin) => ArgumentKind::of_builtin(*builtin),
            TypeRefData::Named(_) => self
                .class_of_type_ref(offset, hir, *written)
                .filter(|class| self.class_location(class.as_str()).is_some())
                .map_or(ArgumentKind::Unknown, ArgumentKind::Class),
            _ => ArgumentKind::Unknown,
        };
        self.disjoint(&declared, &promised)
    }

    /// The first parameter of a closure literal that declares a type no value a documented
    /// signature hands it can be: `fn (string $v)` where `callable(int): string` is declared.
    ///
    /// PHP checks a closure's parameters when it is *called*, so what matters is what the callee
    /// passes -- which is what its docblock signature states -- and how the callee's own code makes
    /// the call. An internal function calls a callback the way a file without `strict_types` does,
    /// so `array_map(fn (string $s) => $s, [1, 2])` converts every element and runs, while a user
    /// function declared under `strict_types=1` handing an `int` to the same closure is a
    /// `TypeError`. So two scalar kinds count only when `strict` says the caller is strict, and an
    /// object against a scalar, or two unrelated classes, count everywhere: no mode converts those.
    pub(crate) fn callback_parameter_refused(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        handed: TypeId,
        strict: impl FnOnce() -> bool,
    ) -> bool {
        let Some(literal) = self.closure_literal_of(held) else {
            return false;
        };
        let phrost_hir::body::ExprData::Closure { params, .. } = hir.expr(literal) else {
            return false;
        };
        let types = self.skeleton.types();
        let TypeRef::Generic { args, .. } = types.get(handed) else {
            return false;
        };
        let mut scalar_refused = false;
        for (&param, &given) in hir.param_list(*params).iter().zip(types.operands(*args)) {
            let param = hir.param(param);
            // A variadic takes whatever is left, which the signature's positions do not line up
            // with.
            if param.is_variadic {
                break;
            }
            let Some(written) = param.type_ref else {
                continue;
            };
            let declared = match hir.type_ref(written) {
                TypeRefData::Builtin(builtin) => ArgumentKind::of_builtin(*builtin),
                TypeRefData::Named(_) => self
                    .class_of_type_ref(offset, hir, written)
                    .filter(|class| self.class_location(class.as_str()).is_some())
                    .map_or(ArgumentKind::Unknown, ArgumentKind::Class),
                _ => ArgumentKind::Unknown,
            };
            let given = self.stored_argument(given);
            if !self.disjoint(&given, &declared) {
                continue;
            }
            match (&given, &declared) {
                (ArgumentKind::Scalar(_), ArgumentKind::Scalar(_)) => scalar_refused = true,
                _ => return true,
            }
        }
        scalar_refused && strict()
    }

    /// A documented callback's whole signature, `callable(int): string`, from the two places a
    /// declaration keeps it: the parameters in their own slot and the return in the type.
    ///
    /// `None` unless the type is that signature and nothing more -- a `?callable(int): string` is
    /// rendered by the type's own renderer, which says what else it takes -- and unless every
    /// parameter is a type this can name.
    pub(crate) fn signature_rendered(
        &self,
        handed: TypeId,
        expected: Expectation,
    ) -> Option<String> {
        let Expectation::Stored(ty) = expected else {
            return None;
        };
        let types = self.skeleton.types();
        let TypeRef::Generic { base, args, .. } = types.get(ty) else {
            return None;
        };
        if !matches!(types.get(*base), TypeRef::Builtin(BuiltinTy::Callable)) {
            return None;
        }
        let [returned] = types.operands(*args) else {
            return None;
        };
        let TypeRef::Generic { args: handed, .. } = types.get(handed) else {
            return None;
        };
        // A class's template is held under the name lowering qualified it to, which is no class at
        // all: `callable(\Psl\Option\T): bool` is worse than the ellipsis.
        if types.operands(*handed).iter().any(|&parameter| {
            self.class_of_stored(parameter, None)
                .is_some_and(|class| self.class_location(class.as_str()).is_none())
        }) {
            return None;
        }
        let parameters = types
            .operands(*handed)
            .iter()
            .map(|&parameter| types.render_qualified(parameter))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "callable({parameters}): {}",
            types.render_qualified(*returned)
        ))
    }

    /// What a `callable(...): T` or `Closure(...): T` expectation promises its result is.
    fn signature_return(
        &self,
        offset: TextSize,
        hir: &FileHir,
        expected: Expectation,
    ) -> Option<ArgumentKind> {
        match expected {
            Expectation::Stored(ty) => {
                let types = self.skeleton.types();
                let TypeRef::Generic { base, args, .. } = types.get(ty) else {
                    return None;
                };
                let callable = match types.get(*base) {
                    TypeRef::Builtin(BuiltinTy::Callable) => true,
                    TypeRef::Named(name) => name.as_str().eq_ignore_ascii_case("Closure"),
                    _ => false,
                };
                match (callable, types.operands(*args)) {
                    (true, [returned]) => Some(self.stored_argument(*returned)),
                    _ => None,
                }
            }
            Expectation::Written(id) => {
                let TypeRefData::Generic { base, args, .. } = hir.type_ref(id) else {
                    return None;
                };
                let callable = match hir.type_ref(*base) {
                    TypeRefData::Builtin(BuiltinTy::Callable) => true,
                    TypeRefData::Named(_) => self
                        .class_of_type_ref(offset, hir, *base)
                        .is_some_and(|class| class.as_str().eq_ignore_ascii_case("Closure")),
                    _ => false,
                };
                match (callable, hir.type_ref_list(*args)) {
                    (true, [returned]) => Some(match hir.type_ref(*returned) {
                        TypeRefData::Builtin(builtin) => ArgumentKind::of_builtin(*builtin),
                        _ => ArgumentKind::Unknown,
                    }),
                    _ => None,
                }
            }
        }
    }

    /// What a value supplies for each of `class`'s templates: its own arguments when it is that
    /// class, or what its declaration wrote for them when it is below it -- `IntBox extends
    /// Box<int>`.
    fn carried_arguments(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        class: &Name,
        count: usize,
    ) -> Vec<ArgumentKind> {
        // A type written for the value -- `@var Box<string>`, a property declared `Box<int>` -- is
        // read the way the expectation is.
        let written = match held {
            Held::Stored { ty, .. } => Some(Expectation::Stored(*ty)),
            Held::Written(id) => Some(Expectation::Written(*id)),
            _ => None,
        };
        if let Some(written) = written {
            return match self.declared_arguments(offset, hir, written) {
                Some((named, kinds)) if named.as_str().eq_ignore_ascii_case(class.as_str()) => {
                    kinds
                }
                _ => Vec::new(),
            };
        }
        let (named, arguments) = match held {
            Held::Applied { class, arguments } => (class, Some(arguments)),
            Held::Class(class) => (class, None),
            _ => return Vec::new(),
        };
        if named.as_str().eq_ignore_ascii_case(class.as_str()) {
            return arguments
                .map(|arguments| {
                    arguments
                        .iter()
                        .map(|argument| self.held_argument(hir, argument))
                        .collect()
                })
                .unwrap_or_default();
        }
        (0..count)
            .map(|position| {
                self.type_argument(
                    named.as_str(),
                    class.as_str(),
                    position,
                    crate::generics::CHASE,
                    Landing::Template,
                )
                .map_or(ArgumentKind::Unknown, |argument| {
                    self.stored_argument(argument)
                })
            })
            .collect()
    }

    fn stored_argument(&self, ty: TypeId) -> ArgumentKind {
        match self.skeleton.types().get(ty) {
            TypeRef::Builtin(builtin) => ArgumentKind::of_builtin(*builtin),
            // A template name resolves to no class; only a declared one is a class.
            TypeRef::Named(name) if self.class_location(name.as_str()).is_some() => {
                ArgumentKind::Class(name.clone())
            }
            _ => ArgumentKind::Unknown,
        }
    }

    fn held_argument(&self, hir: &FileHir, held: &Held) -> ArgumentKind {
        match held {
            Held::Class(class) | Held::Applied { class, .. } => ArgumentKind::Class(class.clone()),
            _ => match self.kind_of_held(hir, held) {
                Some(kind) => ArgumentKind::of_builtin(kind),
                None => ArgumentKind::Unknown,
            },
        }
    }

    /// Whether no value can be both.
    fn disjoint(&self, value: &ArgumentKind, wanted: &ArgumentKind) -> bool {
        match (value, wanted) {
            (ArgumentKind::Scalar(left), ArgumentKind::Scalar(right)) => {
                let numeric = |kind: &BuiltinTy| matches!(kind, BuiltinTy::Int | BuiltinTy::Float);
                left != right && !(numeric(left) && numeric(right))
            }
            (ArgumentKind::Scalar(_), ArgumentKind::Class(_))
            | (ArgumentKind::Class(_), ArgumentKind::Scalar(_)) => true,
            (ArgumentKind::Class(left), ArgumentKind::Class(right)) => {
                self.relate_classes(left, right) == Fit::Rejected
                    && self.relate_classes(right, left) == Fit::Rejected
            }
            _ => false,
        }
    }
}
