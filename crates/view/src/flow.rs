//! Two facts about how control moves through the current file's bodies.
//!
//! Both come out of one walk in [`phrost_body`] and are projected together because they are the same
//! question asked from either end: *can control get past the end of this block*. A function whose
//! declaration promises a value and whose end control can reach raises a `TypeError` on the run that
//! gets there; a statement after a block that control cannot get past runs never.
//!
//! # Why this reads the HIR and not the skeleton
//!
//! Unlike [`IndexView::incompatible_returns`], which has to intern the declared type to judge a
//! value against it, neither of these needs a type at all -- only whether one was *written*, and
//! which keyword it was. That is in the buffer's own tree, so there is no two-tier join here and no
//! name-range keying: a declaration and its body are one node.
//!
//! It also settles a precision question the skeleton would have got wrong. `FunctionSkeleton`'s
//! effective type merges the docblock's `@return` into the native declaration, and PHP enforces
//! only the native one -- so a doc-only `@return string` on an untyped function is a stale comment
//! rather than a `TypeError`, and reporting it would put a runtime claim on code that runs fine.
//!
//! # What falling off the end is not reported for
//!
//! - **No native return type, `void`, or `never`.** Nothing is promised, so nothing is broken.
//! - **A generator.** `function ids(): iterable { yield 1; }` returns an `iterable` by *being* one,
//!   and running off its end is how a generator finishes. Read per body, so a closure that yields
//!   does not excuse the function around it.
//! - **No body.** An abstract method, an interface method and a `get;` hook declare a return type
//!   and have no statements, and none of them is a defect. They arrive here as
//!   [`phrost_hir::MemberData::body`] of `None`, so the rule costs nothing to state.
//! - **An arrow function.** Its body is one expression, which is the value it returns. A closure is
//!   reported on its `function` keyword, since it has no name and underlining a whole multi-line
//!   callback would be worse than silence.
//! - **A body whose last statement is one the tree cannot read.** `goto` and `echo` lower to the
//!   same node, and one of them does not fall through -- see `phrost_body`'s own notes on what a
//!   guessed exit is. The cost is a function ending in an `echo`; the gain is Symfony's
//!   `PhpArrayAdapter::get`, whose only `catch` ends in `goto get_from_pool`.
//!
//! A **nullable** return is reported, deliberately. `function f(): ?string {}` is a `TypeError` in
//! PHP exactly as `: string` is -- an implicit `null` is returned only for `void`, and a nullable
//! declaration still demands an explicit `return null;`. It reads like an exception and is not one.

use std::cell::Cell;

use phrost_hir::{BodyId, FileHir, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, TextRange};

use crate::view::IndexView;

/// One function-like whose end control can reach although its declaration promises a value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MissingReturn {
    /// The declaration's name, because that is what identifies the function in a list and the
    /// declared type has no range of its own to point at.
    pub range: TextRange,
    /// The type the declaration promises, as written.
    pub declared: String,
}

/// One run of statements nothing can reach.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnreachableCode {
    /// The whole run, not its first line: dead code is something to delete, and an editor fades the
    /// range it is given.
    pub range: TextRange,
}

impl IndexView<'_> {
    /// Every function, method or closure in the current file that promises a value and can end
    /// without producing one.
    ///
    /// The `: never` question is asked twice on purpose. A body is first walked knowing nothing
    /// about what a callee returns, which is free; only the ones that came back "can fall off the
    /// end" are walked again with the resolution that recognises `$this->fail()` where `fail():
    /// never`. That order is the whole cost model: the second walk is a full resolution at the
    /// follow limit, and running it over every body of every request is what took diagnostics from
    /// 13.5 ms to 51.5 ms the last time something asked this question per branch.
    pub fn missing_returns(&self) -> Vec<MissingReturn> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for (_, function) in hir.functions() {
            out.extend(self.falls_off_end(
                hir,
                function.return_type,
                function.body,
                function.name_range,
            ));
        }
        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.kind != phrost_hir::MemberKind::Method || member.from_docblock {
                    continue;
                }
                out.extend(self.falls_off_end(
                    hir,
                    member.type_ref,
                    member.body,
                    member.name_range,
                ));
            }
        }
        // And every closure, reported on its `function` keyword: the one word that names it, where
        // underlining a callback thirty lines long would be worse than silence. An arrow function
        // is an expression and cannot fall off anything.
        for (_, expr) in hir.expressions() {
            let phrost_hir::body::ExprData::Closure {
                kind,
                return_type,
                body,
                range,
                ..
            } = expr
            else {
                continue;
            };
            if kind.is_arrow() {
                continue;
            }
            let Some(keyword) = Self::function_keyword(open.text(), *range) else {
                continue;
            };
            out.extend(self.falls_off_end(hir, *return_type, Some(*body), keyword));
        }

        out.sort_by_key(|found| found.range.start());
        out
    }

    /// The `function` a closure is written with, past a `static` in front of it.
    fn function_keyword(text: &str, closure: TextRange) -> Option<TextRange> {
        const KEYWORD: &str = "function";
        let written = text.get(usize::from(closure.start())..usize::from(closure.end()))?;
        let at = written.find(KEYWORD)?;
        let start = closure.start() + phrost_model::TextSize::try_from(at).ok()?;
        Some(TextRange::at(start, phrost_model::TextSize::of(KEYWORD)))
    }

    /// Every run of statements in the current file that no path reaches.
    ///
    /// Every body of the file, the statements written at file scope included -- a `.module` file or
    /// a script keeps its code there, and a `return` at file scope ends the include exactly as one
    /// in a function ends the call.
    pub fn unreachable_code(&self) -> Vec<UnreachableCode> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for body in Self::every_body(hir) {
            out.extend(
                phrost_body::unreachable_regions(hir, hir.body(body).statements)
                    .into_iter()
                    .map(|range| UnreachableCode { range }),
            );
        }

        out.sort_by_key(|found| found.range.start());
        out
    }

    /// One declaration's verdict, or nothing when its declaration promises nothing enforceable.
    fn falls_off_end(
        &self,
        hir: &FileHir,
        declared: Option<TypeRefId>,
        body: Option<BodyId>,
        name_range: TextRange,
    ) -> Option<MissingReturn> {
        let declared = declared?;
        let body = body?;
        if !Self::is_promised(hir, declared) || hir.body(body).is_generator {
            return None;
        }

        let statements = hir.body(body).statements;
        // A body with nothing in it is not code that falls off its end -- there is nothing to fall
        // off. `function f(): float {}` is a *stub*, which is how every extension stub and every
        // hand-kept `stubs/redis.php` is written, and it is also how a declaration somebody has not
        // filled in yet reads. Neither is "not every path returns a value": there are no paths.
        //
        // Measured because the number is the argument. `phrost check` run over this repository, whose
        // vendored `third_party/php-prelude` is 137 such files, reported **5 505** of these -- one per
        // stubbed function -- against 5 553 findings in total. Anyone who keeps a stub file as their
        // own code, which is the ordinary way to type a native extension, got the same experience.
        // On the reference project, which has no such file, this changes nothing: `missing-return`
        // stands at zero there.
        if hir.stmt_list(statements).is_empty() {
            return None;
        }
        // The cheap walk first, and it decides the common case: a body that cannot fall off its end
        // knowing nothing about a callee cannot fall off it knowing more. The flag says whether the
        // walk ever *reached* a bare call -- an empty body and a guard with no `else` never do, and
        // for those the expensive answer cannot differ from the cheap one.
        let asked = Cell::new(false);
        if !phrost_body::falls_off_end(hir, statements, &|_, _| {
            asked.set(true);
            false
        }) {
            return None;
        }
        if asked.get()
            && !phrost_body::falls_off_end(hir, statements, &|hir, expr| {
                self.call_never_returns(hir.expr(expr).range().start(), hir, expr)
            })
        {
            return None;
        }

        Some(MissingReturn {
            range: name_range,
            declared: phrost_hir::render::render_type_ref_short(hir, declared),
        })
    }

    /// Whether a written return type is one PHP makes the function keep.
    ///
    /// `void` promises nothing and `never` promises not to come back at all -- running off the end
    /// of one is wrong, but what is wrong with it is the declaration, and "not every path returns a
    /// value" describes the other mistake.
    fn is_promised(hir: &FileHir, declared: TypeRefId) -> bool {
        !matches!(
            hir.type_ref(declared),
            TypeRefData::Builtin(BuiltinTy::Void | BuiltinTy::Never)
        )
    }

    /// Every body the file's declarations hold, file scope first.
    ///
    /// Closures are absent on purpose: their bodies hang off expressions, and
    /// [`phrost_body::unreachable_regions`] descends into them itself so that a closure written in a
    /// method is found once rather than twice.
    fn every_body(hir: &FileHir) -> Vec<BodyId> {
        let mut out = Vec::new();
        out.extend(hir.root_body());

        for (_, function) in hir.functions() {
            out.extend(function.body);
        }
        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                out.extend(member.body);
                for &hook in hir.hook_list(member.hooks) {
                    out.extend(hir.hook(hook).body);
                }
            }
        }

        out
    }
}
