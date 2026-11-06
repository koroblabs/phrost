//! Closures and arrow functions.

use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{ArrowFunction, Closure, Expression};
use phrost_hir::{
    BodyId,
    body::{BodyData, ClosureKind, ExprData, StmtData},
};
use phrost_model::TextRange;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    pub(crate) fn closure(&mut self, closure: &Closure<'p>, range: TextRange) -> ExprData {
        let params = self.params(
            &closure.parameter_list,
            self.within_the_text(closure.span()).start(),
        );
        let capture_ids = closure
            .use_clause
            .as_ref()
            .map(|clause| {
                clause
                    .variables
                    .iter()
                    .map(|captured| {
                        let name = self.variable_name(captured.variable.name);
                        let range = self.within_the_text(captured.variable.span);
                        let variable = self.builder.add_expr(ExprData::Variable { name, range });
                        // `use (&$found)` is a *binding*, and the `&` is the whole of what says so:
                        // the closure writes the enclosing scope's variable whenever it runs, so
                        // what the visible assignments say about the name is not what it holds.
                        // Wrapped the way every other by-reference target is, which is the shape
                        // `bound_by` already reads.
                        match captured.ampersand {
                            Some(ampersand) => self.builder.add_expr(ExprData::Reference {
                                value: variable,
                                range: self.within_the_text(ampersand).cover(range),
                            }),
                            None => variable,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let captures = self.builder.alloc_expr_list(&capture_ids);
        // A `yield` in here belongs to the closure, not to whatever is lowering around it.
        let outer = std::mem::take(&mut self.saw_yield);
        let statements = self.statement_list(closure.body.statements.as_slice());
        let is_generator = std::mem::replace(&mut self.saw_yield, outer);
        let body = self.builder.add_body(BodyData {
            statements,
            range: self.within_the_text(closure.body.span()),
            is_generator,
        });

        ExprData::Closure {
            // `static function () {}` does not take `$this`, whatever encloses it.
            kind: match closure.r#static.is_some() {
                true => ClosureKind::StaticClosure,
                false => ClosureKind::Closure,
            },
            params,
            captures,
            return_type: self.return_type(closure.return_type_hint.as_ref()),
            body,
            range,
        }
    }

    pub(crate) fn arrow_function(
        &mut self,
        function: &ArrowFunction<'p>,
        range: TextRange,
    ) -> ExprData {
        let params = self.params(
            &function.parameter_list,
            self.within_the_text(function.span()).start(),
        );
        let body = self.expression_body(function.expression);

        ExprData::Closure {
            kind: match function.r#static.is_some() {
                true => ClosureKind::StaticArrowFunction,
                false => ClosureKind::ArrowFunction,
            },
            params,
            // An arrow function captures by value automatically, so there is nothing written to
            // record -- which also means a variable inside one may come from the enclosing scope.
            captures: IdSlice::EMPTY,
            return_type: self.return_type(function.return_type_hint.as_ref()),
            body,
            range,
        }
    }

    /// A body holding a single `return expr`, for an arrow function.
    pub(crate) fn expression_body(&mut self, expression: &Expression<'p>) -> BodyId {
        let range = self.within_the_text(expression.span());
        let outer = std::mem::take(&mut self.saw_yield);
        let value = self.expression(expression);
        let is_generator = std::mem::replace(&mut self.saw_yield, outer);
        let statement = self.builder.add_stmt(StmtData::Return {
            value: Some(value),
            range,
        });
        let statements = self.builder.alloc_stmt_list(&[statement]);
        self.builder.add_body(BodyData {
            statements,
            range,
            is_generator,
        })
    }
}
