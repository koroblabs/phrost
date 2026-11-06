//! A method a class is given at runtime: Laravel's `Collection::macro('sortByCollator', fn ...)`.
//!
//! Nothing declares it. The registration is a call in a service provider's body, and the closure
//! beside the name is the method -- its parameters are the method's and, inside it, `$this` is the
//! instance the method is called on. Read at ingest and stored with the file, for the reason every
//! fact a body states is (see `stated.rs`): a warm start lowers nothing. Which static method
//! registers one is the framework's to say, through `ReturnContracts::macro_registrars`; which class
//! takes registrations is asked where one is used, because that needs the hierarchy.

use phrost_defmap::FileScope;
use phrost_hir::{
    FileHir, ImportKind,
    body::{ClassTarget, ExprData, LiteralKind},
};
use phrost_model::{Interner, Name, TextRange, TextSize};
use phrost_ty::{TypeId, TypeTable};

use crate::{ProjectSkeleton, ReturnContracts};

/// One registration: the class it was made on, the method it gives the class, and what the closure
/// hands back.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MacroSite {
    /// The class the call names, resolved where it is written.
    pub class: Name,
    pub name: Name,
    /// Where the name is written, quotes excluded, which is where goto on a call to the macro lands.
    pub at: TextRange,
    /// What the closure declares it returns, with `static` and `$this` meaning the class it is
    /// called on.
    pub returns: Option<TypeId>,
    /// Otherwise the chain its one `return` hands back -- see [`MacroChain`].
    pub chain: Option<MacroChain>,
}

/// A returned chain, in the names a closed file can keep: a root and the members applied to it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MacroChain {
    /// The class a static call starts from, or `None` for `$this`, `static::` and `self::` -- the
    /// instance the macro is called on.
    pub root: Option<Name>,
    /// Member names in written order, never empty.
    pub steps: Box<[Name]>,
}

impl lspkit_memsize::MemSize for MacroSite {
    fn heap_size(&self) -> usize {
        self.chain
            .as_ref()
            .map_or(0, |chain| lspkit_memsize::flat(&chain.steps))
    }
}

/// How long a returned chain may be. A macro wraps a call or two; past this it is somebody's
/// pipeline, and its type is not a question this stores.
const CHAIN_LIMIT: usize = 8;

impl ProjectSkeleton {
    /// Every registration this file's bodies make.
    pub(crate) fn collect_macros(
        returns: &ReturnContracts,
        interner: &mut Interner,
        types: &mut TypeTable,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
    ) -> Box<[MacroSite]> {
        let registrars = returns.macro_registrars().collect::<Vec<_>>();
        if registrars.is_empty() {
            return Box::default();
        }
        let mut found = Vec::new();
        for (_, expr) in hir.expressions() {
            let ExprData::StaticCall {
                class: ClassTarget::Named(class),
                name: Some(method),
                args,
                ..
            } = expr
            else {
                continue;
            };
            if !registrars
                .iter()
                .any(|registrar| registrar.eq_ignore_ascii_case(method.as_str()))
            {
                continue;
            }
            let args = hir.expr_list(*args);
            let (Some(&named), Some(&closure)) = (args.first(), args.get(1)) else {
                continue;
            };
            let ExprData::Literal {
                kind: LiteralKind::String,
                range: at,
            } = hir.expr(named)
            else {
                continue;
            };
            let Some(name) = crate::dispatch::unquoted(text, *at) else {
                continue;
            };
            let ExprData::Closure {
                return_type, body, ..
            } = hir.expr(closure)
            else {
                continue;
            };
            let written = hir.name_ref(*class);
            let namespace = scope.scope_at(written.range.start());
            let class = namespace
                .resolve(written.path.as_str(), written.anchor, ImportKind::ClassLike)
                .primary()
                .fqn()
                .to_owned();
            let returns = return_type.map(|ty| {
                crate::types::TypeLowerer {
                    hir,
                    namespace,
                    interner,
                    types,
                }
                .lower(ty)
            });
            let chain = match returns {
                Some(_) => None,
                None => Self::returned_chain(interner, scope, hir, *body),
            };
            found.push(MacroSite {
                class: interner.name(&class),
                name: interner.name(name),
                // Inside the quotes, which `unquoted` has just seen are one character each.
                at: TextRange::new(at.start() + TextSize::from(1), at.end() - TextSize::from(1)),
                returns,
                chain,
            });
        }
        found.into_boxed_slice()
    }

    /// The chain the closure's one `return` hands back, when it is calls on `$this` or on a class.
    fn returned_chain(
        interner: &mut Interner,
        scope: &FileScope,
        hir: &FileHir,
        body: phrost_hir::BodyId,
    ) -> Option<MacroChain> {
        let [Some(mut value)] =
            phrost_body::returned_values(hir, hir.body(body).statements).as_slice()[..]
        else {
            return None;
        };
        let mut steps = Vec::new();
        let root = loop {
            if steps.len() > CHAIN_LIMIT {
                return None;
            }
            match hir.expr(value) {
                ExprData::MethodCall {
                    receiver,
                    name: Some(name),
                    ..
                } => {
                    steps.push(name.clone());
                    value = *receiver;
                }
                ExprData::StaticCall {
                    class,
                    name: Some(name),
                    ..
                } => {
                    steps.push(name.clone());
                    break match class {
                        ClassTarget::Named(class) => {
                            let written = hir.name_ref(*class);
                            let fqn = scope
                                .scope_at(written.range.start())
                                .resolve(
                                    written.path.as_str(),
                                    written.anchor,
                                    ImportKind::ClassLike,
                                )
                                .primary()
                                .fqn()
                                .to_owned();
                            Some(interner.name(&fqn))
                        }
                        ClassTarget::Relative {
                            relative:
                                phrost_model::RelativeTy::Static | phrost_model::RelativeTy::SelfTy,
                            ..
                        } => None,
                        _ => return None,
                    };
                }
                ExprData::Variable { name, .. } if name.as_str() == "this" && !steps.is_empty() => {
                    break None;
                }
                _ => return None,
            }
        };
        steps.reverse();
        Some(MacroChain {
            root,
            steps: steps.into_boxed_slice(),
        })
    }
}
