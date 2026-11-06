//! A returned value the declaration cannot hold.
//!
//! `function f(): int { return 'x'; }` is a `TypeError` on the run that reaches the line, and it is
//! the one shape of the compatibility judgement that needs no second file: both halves are written
//! in the declaration a reader is already looking at. The judgement itself is
//! [`IndexView::fit_held`], and its precision rules -- reject only what can be proven wrong -- are
//! the whole of what keeps this quiet on real code.
//!
//! # The three things that make a return different from an argument
//!
//! **A generator's declared type is about the generator.** `function ids(): iterable { yield 1; }`
//! returns an `iterable` by *being* one, and a `return` inside it hands a value to
//! `Generator::getReturn()` rather than to the caller. So a body with a `yield` anywhere in it is
//! skipped whole, on [`phrost_hir::body::BodyData::is_generator`], which is scoped per body: a
//! closure that yields does not make the function around it a generator.
//!
//! **A `return` belongs to the nearest function-like, not to the outermost one.** A closure is its
//! own scope with its own declared type, so [`phrost_body::returned_values`] descends through
//! statements only. A closure body hangs off an *expression*, which that walk does not follow --
//! the same rule every walk in `phrost_body` states, arrived at for the same reason.
//!
//! **Two tiers hold the two halves.** The body is only in the open buffer's HIR, and the declared
//! return type is only interned in the skeleton, because interning needs the table open for
//! writing and a reader does not have it. Nothing carries both, so they are joined on the name
//! range -- see [`IndexView::bodies_by_name`].
//!
//! # What is deliberately not reported
//!
//! - **`void` and `never`.** Returning a value from either is a fatal, but it is a claim about the
//!   *declaration* rather than about the value's type, and the message this check would give
//!   ("declares void, returns int") describes the wrong mistake.
//! - **Anything that may be null, by the type check.** The judgement rejects a nullable against a
//!   non-nullable, so without this every `return $this->maybeNull;` in a project would arrive here
//!   wearing a type message. Null has its own vocabulary in this checker -- `null-argument`,
//!   `possibly-null-receiver`, each with its own severity split -- and [`IndexView::null_returns`]
//!   is the return's entry in it.
//! - **A union with one branch that fits.** The rule the measurement forced, and the one that
//!   decides how this check reads on real code -- see [`IndexView::alternatives`] -- except where
//!   the union is a parameter's own declaration arriving unnarrowed.

#[cfg(test)]
mod tests;

use phrost_hir::{BodyId, ExprId, FileHir, TypeRefData};
use phrost_index::FileSkeleton;
use phrost_model::{BuiltinTy, TextRange};
use phrost_sema::MemberKind;
use phrost_ty::{TypeId, TypeRef, compat::Fit};
use rustc_hash::FxHashMap;

use crate::compat::Expectation;
use crate::variables::Naming;
use crate::{
    resolve::Held,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One `return` whose value no value of the declared type can be.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncompatibleReturn {
    /// The returned expression, which is what a report underlines: the declaration is often pages
    /// away and the thing to change is here.
    pub range: TextRange,
    /// The type the declaration promises, rendered.
    pub declared: String,
    /// The type the value has, rendered.
    pub returned: String,
}

/// One `return` handing back a value that may be null where the declaration admits none.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NullReturn {
    /// The returned expression, for the reason [`IncompatibleReturn::range`] gives.
    pub range: TextRange,
    /// The type the declaration promises, rendered.
    pub declared: String,
    /// The type the value has, rendered -- which says *how* it may be null: `?Foo`, `Foo|null`.
    pub returned: String,
    /// Whether the value is the sentinel on every run, or only on some.
    pub certain: bool,
    /// Which sentinel: null, or the `false` PHP's standard library hands back to mean the same.
    ///
    /// Not folded into the rendered type, because the *code* a reader suppresses and the guard they
    /// write both depend on which it is -- `=== NULL` does nothing for a `false`.
    pub null: bool,
}

impl IndexView<'_> {
    /// Every `return` in the current file handing back something its declaration cannot hold.
    ///
    /// Only what the judgement can *prove* wrong; `Fit::Unknown` -- most of what it says about real
    /// code -- is silence. See this module's own docs for the four shapes left alone on purpose.
    pub fn incompatible_returns(&self) -> Vec<IncompatibleReturn> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();
        for (value, declared) in self.judged_returns() {
            out.extend(self.rejected_return(hir, value, declared));
        }
        out.sort_by_key(|found| found.range.start());
        out
    }

    /// Every `return` in the current file handing back a value that may be null where the
    /// declaration admits none.
    ///
    /// The return's half of `?T where T expected`, beside `null_arguments` and the null receiver
    /// check. `function get(): string { return $this->maybe(); }` is a `TypeError` on the run where
    /// `maybe()` hands back nothing, and it is the one shape of that bug the other two cannot see:
    /// the value goes out of the function rather than into a call or through an arrow. The same
    /// narrowing that keeps those two liveable keeps this one -- a guard above the `return` has
    /// taken the null off the type by the time the value is resolved here -- and the same severity
    /// split applies: a value that is null on every run is an error, one that may be is the branch
    /// nobody tested.
    pub fn null_returns(&self) -> Vec<NullReturn> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();
        for (value, declared) in self.judged_returns() {
            out.extend(self.null_return(hir, value, declared));
        }
        out.sort_by_key(|found| found.range.start());
        out
    }

    /// Every returned expression in the current file beside the type its declaration promises.
    ///
    /// One walk for the two checks that judge the pair, so what each declines is declined by both:
    /// a generator's declared type describes the generator, so nothing a `return` inside one hands
    /// back is measured against it; and a bare `return;` hands back no value, so there is nothing
    /// to judge -- what is wrong with it is that the function ends without producing one, which is
    /// what `missing_returns` reports.
    fn judged_returns(&self) -> Vec<(ExprId, Expectation)> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let Some(skeleton) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
        else {
            return Vec::new();
        };
        let hir = open.hir();
        let bodies = Self::bodies_by_name(hir);
        let mut out = Vec::new();

        for (name_range, declared) in self.declared_returns(skeleton) {
            let Some(&body) = bodies.get(&name_range) else {
                continue;
            };
            if hir.body(body).is_generator {
                continue;
            }
            for value in phrost_body::returned_values(hir, hir.body(body).statements)
                .into_iter()
                .flatten()
            {
                out.push((value, Expectation::Stored(declared)));
            }
        }

        // And every closure, which no skeleton declares and which used to be a stated gap in this
        // module's own docs: a `function (Row $r): Item { ... }` written as a callback declares a
        // return exactly as a method does, and all three checks here were silent inside one. Three
        // of four shapes on a four-line fixture -- a closure, an arrow function and a static
        // closure -- reported nothing where the method beside them reported. What closed it is
        // [`Expectation`], which already carries the difference between a declaration the project
        // interned and one that is an index into this file's own arena.
        //
        // `returned_values` walks statements and never an expression, so a closure nested in a
        // method is reached only here and its returns are judged against its own declaration.
        for (_, expr) in hir.expressions() {
            let phrost_hir::body::ExprData::Closure {
                return_type: Some(declared),
                body,
                ..
            } = expr
            else {
                continue;
            };
            if hir.body(*body).is_generator {
                continue;
            }
            for value in phrost_body::returned_values(hir, hir.body(*body).statements)
                .into_iter()
                .flatten()
            {
                out.push((value, Expectation::Written(*declared)));
            }
        }
        out
    }

    /// What a declaration says, however it was written, for a message to name.
    fn render_expectation(&self, hir: &FileHir, expected: Expectation) -> String {
        match expected {
            Expectation::Stored(ty) => self.skeleton.types().render_qualified(ty),
            Expectation::Written(id) => phrost_hir::render::render_type_ref(hir, id),
        }
    }

    /// The declaration's verdict on the nullness of one returned expression, or nothing when the
    /// value cannot be null or the declaration admits one.
    fn null_return(
        &self,
        hir: &FileHir,
        value: ExprId,
        declared: Expectation,
    ) -> Option<NullReturn> {
        let range = hir.expr(value).range();
        let at = range.start();
        let path = phrost_body::value_path_of(hir, value)?;
        let Some(held) = self.resolve_path(at, hir, &path, FOLLOW_LIMIT) else {
            // No type, and still a `NULL` the author wrote that can come back: the fallback of a
            // `?? NULL` over a value nothing describes. See `writes_a_null`.
            return (!self.expectation_admits(hir, declared, phrost_model::BuiltinTy::Null)
                && self.writes_a_null(at, hir, &path, FOLLOW_LIMIT))
            .then(|| NullReturn {
                null: true,
                range,
                declared: self.render_expectation(hir, declared),
                returned: "a value that falls back to NULL".to_owned(),
                certain: false,
            });
        };
        // Null first where both can arrive, for the reason the receiver check gives: one report per
        // return, and the guard is the same work either way. Each sentinel is asked only where the
        // declaration does not already admit it.
        let (null, reach) = self.reachable_sentinel(hir, &held, declared)?;
        let certain = match reach {
            crate::references::SentinelReach::Impossible => return None,
            crate::references::SentinelReach::Possible => false,
            crate::references::SentinelReach::Certain => true,
        };
        Some(NullReturn {
            null,
            range,
            declared: self.render_expectation(hir, declared),
            returned: self
                .render_held(at, hir, &held, Naming::Short)
                .unwrap_or_else(|| "a value that may be null".to_owned()),
            certain,
        })
    }

    /// Which sentinel can reach a returned value that the declaration does not admit.
    ///
    /// `true` in the pair is null. One place rather than two conditions written twice, because
    /// `null_return` reports it and `rejected_return` steps aside for exactly the same cases -- and
    /// the two drifting apart is how a value gets reported once for being a sentinel and once for
    /// being the wrong type.
    fn reachable_sentinel(
        &self,
        hir: &FileHir,
        held: &crate::resolve::Held,
        declared: Expectation,
    ) -> Option<(bool, crate::references::SentinelReach)> {
        for (null, sentinel) in [
            (true, phrost_model::BuiltinTy::Null),
            (false, phrost_model::BuiltinTy::False),
        ] {
            if self.expectation_admits(hir, declared, sentinel) {
                continue;
            }
            let reach = match null {
                true => self.null_reach_of(hir, held),
                false => self.false_reach_of(hir, held),
            };
            if reach != crate::references::SentinelReach::Impossible {
                return Some((null, reach));
            }
        }
        None
    }

    /// The declaration's verdict on one returned expression, or nothing when it is not provably
    /// wrong.
    fn rejected_return(
        &self,
        hir: &FileHir,
        value: ExprId,
        declared: Expectation,
    ) -> Option<IncompatibleReturn> {
        let range = hir.expr(value).range();
        let at = range.start();
        let path = phrost_body::value_path_of(hir, value)?;
        let held = self.resolve_path(at, hir, &path, FOLLOW_LIMIT)?;

        // A sentinel is a separate vocabulary in this checker and this check does not speak it; see
        // the module docs. Tested before the judgement rather than after, because such a value is
        // rejected for *being* one and the message would then name the wrong problem. Both
        // sentinels, because `null_return` now reports both and one report per return is the rule.
        //
        // About the *value* alone and deliberately not about what the declaration admits. Asking
        // the declaration-aware question here was measured: it lets a value through whose sentinel
        // is admitted, and then judges the rest of the union -- which reported two working methods
        // on the reference project, `array_search`'s `int|string|false` behind a `=== FALSE` ternary
        // and a `getCode()` typed loosely. Both are the "the union that arrives is not the union the
        // declaration says" family this module's own docs decline to report on.
        let sentinel_reaches = self.null_reach_of(hir, &held)
            != crate::references::SentinelReach::Impossible
            || self.false_reach_of(hir, &held) != crate::references::SentinelReach::Impossible;
        if sentinel_reaches {
            return None;
        }
        // `self` and `static` are the class the code is written in, which the judgement cannot see
        // and answers `Unknown` for. Here it can: a value of a class that refuses the enclosing one
        // is never an instance of it, nor of anything `static` may stand for below it.
        if let Some(class) = self.relative_return_class(at, hir, declared) {
            let value_class = self.class_of_held(at, hir, &held)?;
            if self.relate_classes(&value_class, &class) != Fit::Rejected {
                return None;
            }
            return Some(IncompatibleReturn {
                range,
                declared: self.render_expectation(hir, declared),
                returned: self
                    .render_held(at, hir, &held, Naming::Short)
                    .unwrap_or_else(|| value_class.to_string()),
            });
        }
        if self.fit_written_value(at, hir, value, &held, declared) != Fit::Rejected {
            return None;
        }
        // One rejected branch of a union is what the judgement rejects on, and that is one branch
        // too few here: see [`Self::alternatives`].
        if !self.declared_union_arrives(at, hir, value, &held)
            && !self
                .alternatives(hir, &held)
                .iter()
                .all(|one| self.fit_written_value(at, hir, value, one, declared) == Fit::Rejected)
        {
            return None;
        }

        Some(IncompatibleReturn {
            range,
            declared: self.render_expectation(hir, declared),
            returned: self
                .render_held(at, hir, &held, Naming::Short)
                .unwrap_or_else(|| "a value of another type".to_owned()),
        })
    }

    /// Whether the value is a parameter's declared union and nothing has narrowed it: a bare
    /// local, bound by its parameter at the `return`, holding the very type that parameter wrote.
    ///
    /// The one union [`Self::alternatives`] does not shelter, because neither of its families can
    /// reach it. An overload the argument decides is a *call's* return, and a narrowing that has not
    /// landed leaves a guard, which binds the name as a narrowing rather than as the parameter.
    /// `function f(A|B $x): B { return $x; }` is a `TypeError` on every call with an `A`, and the
    /// union is the author's own declaration three tokens to the left. Measured at no new finding on
    /// six corpora; what it does catch is the code that forgot the guard.
    fn declared_union_arrives(
        &self,
        at: phrost_model::TextSize,
        hir: &FileHir,
        value: ExprId,
        held: &Held,
    ) -> bool {
        let phrost_hir::body::ExprData::Variable { name, .. } = hir.expr(value) else {
            return false;
        };
        let Some(bound) = phrost_body::local_binding(
            hir,
            at,
            name.as_str(),
            &|hir, expr| self.call_never_returns(at, hir, expr),
            &self.asserting(),
        ) else {
            return false;
        };
        bound.kind == phrost_body::BindingKind::Parameter
            && bound.value.steps.is_empty()
            && matches!(
                (&bound.value.root, held),
                (phrost_body::PathRoot::Written(declared), Held::Written(arrived)) if declared == arrived
            )
    }

    /// The class a declared `self` or `static` return stands for: the one the `return` is written
    /// in. `None` for any other declaration, and for a nullable or a union holding one -- those keep
    /// the general judgement.
    fn relative_return_class(
        &self,
        at: phrost_model::TextSize,
        hir: &FileHir,
        declared: Expectation,
    ) -> Option<phrost_model::Name> {
        let relative = match declared {
            Expectation::Stored(ty) => match self.skeleton.types().get(ty) {
                TypeRef::Relative(relative) => *relative,
                _ => return None,
            },
            Expectation::Written(id) => match hir.type_ref(id) {
                TypeRefData::Relative(relative) => *relative,
                _ => return None,
            },
        };
        match relative {
            phrost_model::RelativeTy::SelfTy | phrost_model::RelativeTy::Static => {
                self.enclosing_class(at)
            }
            phrost_model::RelativeTy::Parent => None,
        }
    }

    /// The branches a value's type spells out, or the value itself when it spells out one.
    ///
    /// The rule this whole check's precision rests on: **a union is reported only when every branch
    /// of it is impossible.** `phrost_ty` rejects a union as soon as one branch cannot land, which
    /// is what PHP does -- but the unions that reach a `return` are almost never the union that
    /// actually arrives, and what widens them is not visible from here.
    ///
    /// Two of the things that widened them are gone:
    ///
    /// - ~~**`false` is not narrowed.**~~ `$t = strtotime($s); if ($t !== false) { return $t; }` now
    ///   leaves an `int` at the `return`, so a guarded value no longer looks like an unguarded one.
    /// - ~~**`str_replace` is an overload its third argument decides.**~~ It says so itself now. The
    ///   stub's else branch used to be a second conditional, which lowering has no type to take out
    ///   of and so drops the contract whole -- the docblock was there and inert. Flattened, a
    ///   `string` subject gets a `string` back.
    ///
    /// What is left is why the rule stays. Measured with both of those in place, relaxing it on a
    /// 1 662-file project reports 17 findings, of which **11 are wrong**, in two families:
    ///
    /// - **Seven are narrowing that has not landed yet**: `maybe() ?: 0`, `$t === false ? 0 : $t`,
    ///   and the early-return guard `if (!$r) { return null; } return $r;`. Each is a union
    ///   something already settled, arriving here whole. This family closes on its own.
    /// - **Four are an overload the argument decides, and none of the four can be told so.**
    ///   `pathinfo` is `array|string` because `$flags is 15` picks, and that test is outside the
    ///   contract's vocabulary twice over: an integer literal is no
    ///   [`phrost_hir::ConditionTestData`], and real code writes `PATHINFO_EXTENSION`, a constant,
    ///   which is not a literal argument at all. The same shape sits in vendor code no stub edit
    ///   reaches -- AWS's `unmarshalItem(array $data, $mapAsObject = false): array|stdClass` -- and
    ///   once more with a subject typed only as an untyped array's element, where the conditional
    ///   that *is* readable settles nothing.
    ///
    /// Measured before either fix: every one of the 24 findings was a union with a branch that fits,
    /// and 17 of them were plainly correct code. Requiring every branch to be impossible keeps
    /// `string|array` where an `int` is declared -- which no narrowing saves -- and drops the rest.
    ///
    /// Re-taken on 2026-10-23, after the narrowing family had closed, because an argument and an
    /// assignment both report the union this declines and PHPStan reports it at level 8. Relaxed,
    /// seven projects went from 65 findings to 214 and the reference project from 2 to 4. What came
    /// in is the overload family above and nothing new: the reference project's two are
    /// `unmarshalItem(): array|stdClass` and a translation helper's `string|MarkupInterface`, and
    /// most of Monica's 23 are `findOrFail()` given an id of no known type -- a model on every run
    /// that passes a scalar. The rule stays.
    ///
    /// A `bool` is two branches here, `true` and `false`, as it is to the judgement: `return
    /// (bool) $check;` where `array|false` is declared has a branch that fits, and is the same
    /// family as the rest. The `@var` on an assignment asks the same question -- see
    /// [`IndexView::incompatible_assignments`] -- because an annotation narrower than its value is
    /// an assertion, not a contradiction.
    pub(crate) fn alternatives(&self, hir: &FileHir, held: &Held) -> Vec<Held> {
        let halves = || {
            vec![
                Held::Builtin(BuiltinTy::True),
                Held::Builtin(BuiltinTy::False),
            ]
        };
        self.spelled_alternatives(hir, held)
            .into_iter()
            .flat_map(|one| match self.is_plain_bool(hir, &one) {
                true => halves(),
                false => vec![one],
            })
            .collect()
    }

    /// A `bool` in any of its three spellings, with nothing wrapped around it.
    fn is_plain_bool(&self, hir: &FileHir, held: &Held) -> bool {
        match held {
            Held::Builtin(builtin) => *builtin == BuiltinTy::Bool,
            Held::Stored { ty, .. } => {
                self.skeleton.types().get(*ty) == &TypeRef::Builtin(BuiltinTy::Bool)
            }
            Held::Written(id) => hir.type_ref(*id) == &TypeRefData::Builtin(BuiltinTy::Bool),
            _ => false,
        }
    }

    fn spelled_alternatives(&self, hir: &FileHir, held: &Held) -> Vec<Held> {
        match held {
            Held::OneOf(operands) => operands.to_vec(),
            Held::Stored { ty, scope } => match self.skeleton.types().get(*ty) {
                TypeRef::Union(operands) => self
                    .skeleton
                    .types()
                    .operands(*operands)
                    .iter()
                    .map(|&operand| Held::Stored {
                        ty: operand,
                        scope: scope.clone(),
                    })
                    .collect(),
                _ => vec![held.clone()],
            },
            Held::Written(id) => match hir.type_ref(*id) {
                TypeRefData::Union(parts) => hir
                    .type_ref_list(*parts)
                    .iter()
                    .map(|&part| Held::Written(part))
                    .collect(),
                _ => vec![held.clone()],
            },
            _ => vec![held.clone()],
        }
    }

    /// What each function-like the current file declares promises to return, by the name range that
    /// identifies it.
    ///
    /// Read from the skeleton because that is the only tier that has interned the type, and the
    /// expected side of the judgement must be interned. `void` and `never` are dropped here rather
    /// than at the finding: what is wrong with a value returned from either is the declaration, not
    /// the value's type.
    fn declared_returns(&self, skeleton: &FileSkeleton) -> Vec<(TextRange, TypeId)> {
        let types = self.skeleton.types();
        let mut out = Vec::new();

        for (index, symbol) in skeleton.scope().symbols().iter().enumerate() {
            let Some(function) = skeleton.function_of(u32::try_from(index).unwrap_or(u32::MAX))
            else {
                continue;
            };
            if let Some(declared) = function
                .effective_ty(types)
                .filter(|&ty| self.is_judged(ty))
            {
                out.push((symbol.name_range, declared));
            }
        }

        for class in skeleton.classes() {
            for member in class.members() {
                if member.kind != MemberKind::Method {
                    continue;
                }
                if let Some(declared) = member.effective_ty(types).filter(|&ty| self.is_judged(ty))
                {
                    out.push((member.name_range, declared));
                }
            }
        }

        out
    }

    /// Whether a declared return type is one this check has anything to say about.
    fn is_judged(&self, declared: TypeId) -> bool {
        !matches!(
            self.skeleton.types().get(declared),
            TypeRef::Builtin(BuiltinTy::Void | BuiltinTy::Never)
        )
    }

    /// Where each of the current file's declarations keeps its body, keyed by its name range.
    ///
    /// The join between the two tiers. Both the skeleton's rows and this HIR are written from one
    /// lowering of one text -- the buffer and the skeleton are updated from the same string -- so
    /// the name range identifies the same declaration in each, and it is the only field of a
    /// declaration that both tiers keep and that survives a cache round-trip.
    fn bodies_by_name(hir: &FileHir) -> FxHashMap<TextRange, BodyId> {
        let mut out = FxHashMap::default();

        for (_, function) in hir.functions() {
            if let Some(body) = function.body {
                out.insert(function.name_range, body);
            }
        }
        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if let Some(body) = member.body {
                    out.insert(member.name_range, body);
                }
            }
        }

        out
    }
}
