//! A `return` or a `yield` the declared return type refuses.
//!
//! Four of PHP's compile-time fatals, each stated by one declaration and its own body:
//!
//! * `return 1;` in a `: void` function -- `A void function must not return a value`.
//! * `return;` in a function with any other type -- `A function with return type must return a
//!   value`.
//! * any `return` at all in a `: never` function -- `A never-returning function must not return`,
//!   which PHP says for a bare one as much as for a value.
//! * `yield` where the declared type cannot be a generator -- `Generator return type must be a
//!   supertype of Generator`.
//!
//! The last one is narrower than it looks and was run rather than reasoned: `object` and `mixed`
//! both accept a generator, as do `iterable`, `Traversable`, `Iterator` and `Generator`. Only
//! `void`, `never` and the scalars refuse it -- `array` among them, which is the one a reader
//! expects to work.

use phrost_hir::{BodyId, FileHir, MemberKind, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, TextRange};

use crate::view::IndexView;

/// One `return` or `yield` the declaration will not take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RefusedReturn {
    pub range: TextRange,
    pub because: WrongReturn,
}

/// Which of the four it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WrongReturn {
    /// A value handed back by a `: void`.
    ValueFromVoid,
    /// A bare `return;` where a type was declared.
    NothingFromTyped,
    /// Any `return` in a `: never`.
    ReturnFromNever,
    /// A `yield` under a type no generator fits.
    YieldFromNonGenerator,
}

impl IndexView<'_> {
    /// Every `return` and `yield` in the current file its own declaration refuses.
    pub fn refused_returns(&self) -> Vec<RefusedReturn> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for (id, class) in hir.classes() {
            for &member_id in hir.member_list(class.members) {
                let member = hir.member(member_id);
                if member.owner != id || member.kind != MemberKind::Method {
                    continue;
                }
                if let Some(body) = member.body {
                    self.judge(hir, member.type_ref, body, false, &mut out);
                }
            }
        }
        for (_, function) in hir.functions() {
            if let Some(body) = function.body {
                self.judge(hir, function.return_type, body, false, &mut out);
            }
        }
        phrost_body::visit_every_expression(hir, &mut |hir, expr| {
            if let phrost_hir::body::ExprData::Closure {
                kind,
                return_type,
                body,
                ..
            } = hir.expr(expr)
            {
                self.judge(hir, *return_type, *body, kind.is_arrow(), &mut out);
            }
        });

        out.sort_by_key(|one| one.range.start());
        out.dedup();
        out
    }

    /// One function-like, judged against what it declared.
    fn judge(
        &self,
        hir: &FileHir,
        declared: Option<TypeRefId>,
        body: BodyId,
        is_arrow: bool,
        out: &mut Vec<RefusedReturn>,
    ) {
        let Some(declared) = declared else {
            return;
        };
        let kind = match hir.type_ref(declared) {
            TypeRefData::Builtin(builtin) => Some(builtin.unrefined()),
            _ => None,
        };
        let data = hir.body(body);

        // Two bodies whose `return` is not the one these rules are about.
        //
        // A **generator**'s: a bare one ends the iteration and a value is what `getReturn()` hands
        // back, so neither rule below applies. Symfony writes 56 of these and psl 23, all
        // `yield ...; return;` inside an `: iterable`.
        //
        // An **arrow function**'s, which is synthesized by lowering rather than written: the body
        // *is* the expression. psl writes `static fn (Throwable $t): never => throw $t` sixteen
        // times, and PHP accepts it -- a `throw` never returns, and nothing here is asked to know
        // that. `fn (): void => 1` stays reported, because the body of an arrow function always
        // has a value and PHP refuses that one whatever the value is.
        for (value, range) in match data.is_generator {
            true => Vec::new(),
            false => phrost_body::return_sites(hir, data.statements),
        } {
            if is_arrow && kind != Some(BuiltinTy::Void) {
                continue;
            }
            let because = match (kind, value.is_some()) {
                (Some(BuiltinTy::Never), _) => WrongReturn::ReturnFromNever,
                (Some(BuiltinTy::Void), true) => WrongReturn::ValueFromVoid,
                (Some(BuiltinTy::Void), false) => continue,
                (_, false) => WrongReturn::NothingFromTyped,
                (_, true) => continue,
            };
            out.push(RefusedReturn { range, because });
        }

        // `object` and `mixed` accept a generator, and so does every iterable-shaped class name --
        // PHP asks only that the declared type be a *supertype* of `Generator`. What is left is
        // `void`, `never` and the scalars, `array` among them.
        if data.is_generator
            && matches!(
                kind,
                Some(
                    BuiltinTy::Void
                        | BuiltinTy::Never
                        | BuiltinTy::Int
                        | BuiltinTy::Float
                        | BuiltinTy::String
                        | BuiltinTy::Bool
                        | BuiltinTy::True
                        | BuiltinTy::False
                        | BuiltinTy::Array
                        | BuiltinTy::Null
                )
            )
        {
            out.push(RefusedReturn {
                range: data.range,
                because: WrongReturn::YieldFromNonGenerator,
            });
        }
    }
}
