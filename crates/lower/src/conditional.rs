//! Declarations only some runs reach.
//!
//! PHP hoists a `class` or `function` written at a file's top level, so it exists before the first
//! line runs. It hoists nothing written anywhere else. Two shapes rely on that:
//!
//! ```php
//! if (!function_exists('dump')) { function dump(mixed ...$vars): mixed { /* .. */ } }
//! function outer(): void { function inner(): void {} }
//! ```
//!
//! The first is how PHP ships a polyfill and the second is how a script keeps a helper out of the
//! global table until it is wanted, and both were dropped whole: the declaration walk covered a
//! file's top level, a namespace, a bare block and a `declare` body, and every other statement fell
//! past it. A call to one then resolved to nothing, exactly as a typo does.
//!
//! # Why the flag rather than only the lowering
//!
//! Lowering these on its own was tried and measured, and it costs the standard library. A guarded
//! declaration means *only if nothing else declared this*, and on a 30 871-file checkout 402 names
//! are written that way -- 113 of them polyfills of functions the bundled stubs already declare.
//! Ranked by origin alone a vendor declarer beats a stub, so `str_starts_with(string, string)`
//! becomes symfony's `(?string, ?string)` and every real `possibly-null-argument` against it
//! disappears. So the walk records *that the declaration is conditional*, and
//! `ProjectSkeleton::rank` puts it below every unguarded declarer including the stubs.
//!
//! # Where a nested declaration can be, and where this stops
//!
//! Every statement mago can nest a statement inside is here: `if` and its `elseif` and `else`
//! clauses, `switch` cases, `try`/`catch`/`finally`, the four loops, and a function's or method's
//! own body -- each in both the braced and the colon-delimited form, because `if (..): class X {}
//! endif;` is the same declaration written differently.
//!
//! It stops at an expression. A `function` statement inside a *closure* body is reachable only
//! through the expression walk, which runs for open buffers alone -- so descending there would give
//! a name that exists while a file is open and not while it is closed, which is worse than the gap.
//! Nothing in the measured corpus writes one, and a polyfill never does: the guard has to run at
//! include time, which is what puts it at statement level.

use mago_syntax::cst::{Block, ForBody, ForeachBody, IfBody, Statement, SwitchBody, WhileBody};

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// Lowers the declarations a statement wraps, as ones only some runs reach.
    ///
    /// Does nothing for a statement that wraps no statement list, which is most of them -- an
    /// `echo` or a `return` declares nothing, and the shapes that declare something themselves were
    /// handled before this was reached.
    pub(crate) fn wrapped_declarations(&mut self, statement: &Statement<'p>) {
        match statement {
            Statement::If(node) => self.conditionally(|lower| match &node.body {
                IfBody::Statement(body) => {
                    lower.statement(body.statement);
                    for clause in body.else_if_clauses.iter() {
                        lower.statement(clause.statement);
                    }
                    if let Some(clause) = &body.else_clause {
                        lower.statement(clause.statement);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    lower.statements(body.statements.as_slice());
                    for clause in body.else_if_clauses.iter() {
                        lower.statements(clause.statements.as_slice());
                    }
                    if let Some(clause) = &body.else_clause {
                        lower.statements(clause.statements.as_slice());
                    }
                }
            }),
            Statement::Switch(node) => self.conditionally(|lower| match &node.body {
                SwitchBody::BraceDelimited(body) => {
                    for case in body.cases.iter() {
                        lower.statements(case.statements());
                    }
                }
                SwitchBody::ColonDelimited(body) => {
                    for case in body.cases.iter() {
                        lower.statements(case.statements());
                    }
                }
            }),
            Statement::Try(node) => self.conditionally(|lower| {
                lower.statements(node.block.statements.as_slice());
                for clause in node.catch_clauses.iter() {
                    lower.statements(clause.block.statements.as_slice());
                }
                if let Some(clause) = &node.finally_clause {
                    lower.statements(clause.block.statements.as_slice());
                }
            }),
            Statement::While(node) => self.conditionally(|lower| match &node.body {
                WhileBody::Statement(body) => lower.statement(body),
                WhileBody::ColonDelimited(body) => lower.statements(body.statements.as_slice()),
            }),
            Statement::DoWhile(node) => self.conditionally(|lower| lower.statement(node.statement)),
            Statement::For(node) => self.conditionally(|lower| match &node.body {
                ForBody::Statement(body) => lower.statement(body),
                ForBody::ColonDelimited(body) => lower.statements(body.statements.as_slice()),
            }),
            Statement::Foreach(node) => self.conditionally(|lower| match &node.body {
                ForeachBody::Statement(body) => lower.statement(body),
                ForeachBody::ColonDelimited(body) => lower.statements(body.statements.as_slice()),
            }),
            _ => {}
        }
    }

    /// The declarations written inside a function-like body.
    ///
    /// Separate from [`Self::wrapped_declarations`] because a body is not a statement: a method's
    /// hangs off the member and a free function's off the declaration, and neither passes through
    /// the statement walk. Always conditional -- `function outer() { function inner() {} }` gives
    /// `inner` to the global table only once `outer` has been called.
    ///
    /// Walked whether or not bodies are being lowered. The body *IR* is what an open buffer pays
    /// for; what a body **declares** is a cross-file fact that a closed file has to answer too, or
    /// a name would appear when someone opened the file that declares it.
    pub(crate) fn body_declarations(&mut self, block: &Block<'p>) {
        self.conditionally(|lower| lower.statements(block.statements.as_slice()));
    }

    /// Runs `walk` with everything it declares marked as reached only when the code around it runs.
    ///
    /// Saved and restored rather than set, so that a declaration nested two constructs deep is
    /// still conditional when the inner one is finished -- and so that a top-level declaration
    /// written after an `if` is not.
    fn conditionally(&mut self, walk: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.conditional, true);
        walk(self);
        self.conditional = outer;
    }
}
