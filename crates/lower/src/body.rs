//! Statement and expression bodies, lowered only when asked for.
//!
//! Two things make this a separate pass rather than part of declaration lowering. A body is much
//! larger than the declaration it belongs to, and nothing cross-file ever reads one — where a class
//! is declared and what members it has are the same answers whether or not anybody looked inside a
//! method. So the skeleton pays nothing, and an open buffer gets everything.
//!
//! # What is modelled
//!
//! The shapes that carry a variable, a member access, a written name, or a branch. Everything else
//! becomes `Unmodelled` with its range: the tree stays complete, and the gaps show up in a snapshot
//! instead of being silently absent. Growing the set is then driven by a test that wants a shape,
//! not by guessing which of PHP's forty statement kinds matter.
//!
//! # Normalizations
//!
//! - **`elseif` chains become nested ifs.** One shape to narrow against instead of a chain.
//! - **`while` and `do ... while` are one node.** Whether the body runs before the first test
//!   matters to a runtime, not to what any of this answers.
//! - **An arrow function becomes a closure whose body is one `return`.** Both then have one shape,
//!   so inference does not need two paths to find what a callable produces.
//! - **Parentheses disappear.** `($x)` is `$x`; keeping the wrapper would make every consumer
//!   unwrap it.

use lspkit_arena::IdSlice;
use mago_span::{HasSpan as _, Span};
use mago_syntax::cst::{
    Block, DirectVariable, DoWhile, For, ForBody, Foreach, ForeachBody, ForeachTarget, If, IfBody,
    Node, Statement, StaticItem, Switch, SwitchCase, Try, Variable, While, WhileBody,
};
use phrost_hir::body::{Passage, UnmodelledKind};
use phrost_hir::{
    BodyId, CatchId, ExprId, StmtId, SwitchCaseId, TypeRefId,
    body::{
        AssignOp, BindKind, BodyData, CatchData, ExprData, LeaveKind, StmtData, SwitchCaseData,
    },
};
use phrost_model::{TextRange, TextSize};
use phrost_syntax::text_range;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// Lowers a `{ ... }` body, or nothing when bodies were not asked for.
    pub(crate) fn body(&mut self, block: &Block<'p>) -> Option<BodyId> {
        if !self.lower_bodies {
            return None;
        }

        let range = self.within_the_text(block.span());
        let outer = std::mem::take(&mut self.saw_yield);
        let statements = self.body_statement_list(block.statements.as_slice(), range);
        let is_generator = std::mem::replace(&mut self.saw_yield, outer);
        Some(self.builder.add_body(BodyData {
            statements,
            range,
            is_generator,
        }))
    }

    /// A function's or method's body, with the `@global` tags of its docblock read as annotations at
    /// its top.
    ///
    /// Where WordPress writes them: `@global wpdb $wpdb` in the docblock of the function whose body
    /// says `global $wpdb;`, not above the statement -- 1 734 of them, and `$wpdb` alone is behind
    /// three hundred untyped receivers. A tag above the statement is read by the statement list
    /// already; this is the same claim written one level up.
    pub(crate) fn declared_body(
        &mut self,
        block: &Block<'p>,
        declaration_start: TextSize,
    ) -> Option<BodyId> {
        let body = self.body(block)?;
        let globals = self.doc_global_bindings(declaration_start);
        if globals.is_empty() {
            return Some(body);
        }
        let mut ids = globals
            .into_iter()
            .map(|(name, ty, range)| {
                self.builder
                    .add_stmt(StmtData::Annotation { name, ty, range })
            })
            .collect::<Vec<_>>();
        ids.extend_from_slice(self.builder.body_statements(body));
        let statements = self.builder.alloc_stmt_list(&ids);
        self.builder.set_body_statements(body, statements);
        Some(body)
    }

    /// The statements written at file scope, outside any declaration.
    ///
    /// PHP allows code there, and plenty of real code lives there: a script, a Drupal `.module`
    /// file, a bootstrap. Without it a call at file scope has no node, so nothing can be clicked.
    pub(crate) fn root_body(&mut self, statements: &[Statement<'p>]) -> BodyId {
        // The whole file: a statement at file scope is inside no braces, so the only enclosing
        // range there is is the source itself.
        let range = TextRange::up_to(
            phrost_model::TextSize::try_from(self.source_len()).unwrap_or_default(),
        );
        let outer = std::mem::take(&mut self.saw_yield);
        let statements = self.body_statement_list(statements, range);
        // A `yield` at file scope is a parse error rather than a generator, but the flag is scoped
        // the same way regardless: a closure written at file scope must not leak its own out.
        let is_generator = std::mem::replace(&mut self.saw_yield, outer);
        self.builder.add_body(BodyData {
            statements,
            range,
            is_generator,
        })
    }

    pub(crate) fn statement_list(&mut self, statements: &[Statement<'p>]) -> IdSlice<StmtId> {
        let ids = self.statement_ids(statements);
        self.builder.alloc_stmt_list(&ids)
    }

    /// A whole body's statements, plus the annotations error recovery left with nothing below them.
    ///
    /// The tail of a body is where mago gives up: a statement it cannot finish is dropped rather
    /// than recovered, because there is no closing token left to resynchronize on. So the docblock
    /// above it is never asked for -- and the dropped statement is `$x->`, the one being typed,
    /// which is exactly when `/** @var Foo $x */` is what the answer depends on.
    ///
    /// Bodies only, not every nested list: a statement inside braces has a `}` after it, so mago
    /// recovers there and the statement survives with the annotation already attached. Reading only
    /// the span after a body's last statement is also what makes double emission impossible -- a
    /// docblock a statement claimed lies before that statement's end, so it is never in the window.
    fn body_statement_list(
        &mut self,
        statements: &[Statement<'p>],
        body: TextRange,
    ) -> IdSlice<StmtId> {
        let mut ids = self.statement_ids(statements);

        let claimed_through = statements
            .last()
            .map_or(body.start(), |last| self.within_the_text(last.span()).end());
        if claimed_through < body.end() {
            let window = TextRange::new(claimed_through, body.end());
            for (name, ty, range) in self.orphaned_var_bindings(window) {
                ids.push(
                    self.builder
                        .add_stmt(StmtData::Annotation { name, ty, range }),
                );
            }
        }

        self.builder.alloc_stmt_list(&ids)
    }

    /// The statements of one list, each preceded by the annotations its docblock writes.
    fn statement_ids(&mut self, statements: &[Statement<'p>]) -> Vec<StmtId> {
        let mut ids = Vec::with_capacity(statements.len());
        for statement in statements {
            // Before the statement, because that is where the docblock is: an annotation says what a
            // variable holds from its own position onward, and it has to rank earlier than anything
            // the statement itself binds.
            let start = self.within_the_text(statement.span()).start();
            for (name, ty, range) in self.doc_var_bindings(start) {
                ids.push(
                    self.builder
                        .add_stmt(StmtData::Annotation { name, ty, range }),
                );
            }
            ids.extend(self.body_statement(statement));
        }
        ids
    }

    /// One statement inside a body.
    ///
    /// `None` for the ones that are not statements in any useful sense — the opening tag, inline
    /// HTML, a stray semicolon. Keeping them would make every body start with noise.
    fn body_statement(&mut self, statement: &Statement<'p>) -> Option<StmtId> {
        let range = self.within_the_text(statement.span());
        let data = match statement {
            Statement::OpeningTag(_)
            | Statement::ClosingTag(_)
            | Statement::Inline(_)
            | Statement::Noop(_) => return None,
            Statement::Expression(expression) => StmtData::Expr {
                // The statement's own start, because a `/** @var T $x */` sits above the statement
                // rather than above the expression inside it.
                expr: self.expression_with_annotation(expression.expression, range.start()),
                range,
            },
            Statement::Return(statement) => StmtData::Return {
                value: statement.value.map(|value| self.expression(value)),
                range,
            },
            Statement::If(statement) => self.if_statement(statement, range),
            Statement::Foreach(statement) => self.foreach_statement(statement, range),
            Statement::While(statement) => self.while_statement(statement, range),
            Statement::For(statement) => self.for_statement(statement, range),
            Statement::DoWhile(statement) => self.do_while_statement(statement, range),
            Statement::Block(block) => StmtData::Block {
                body: self.statement_list(block.statements.as_slice()),
                range,
            },
            Statement::Try(statement) => self.try_statement(statement, range),
            Statement::Switch(statement) => self.switch_statement(statement, range),
            Statement::Break(_) => StmtData::Leave {
                kind: LeaveKind::Break,
                range,
            },
            Statement::Continue(_) => StmtData::Leave {
                kind: LeaveKind::Continue,
                range,
            },
            // A `namespace App;` with no braces makes the rest of the file its body, so at file
            // scope everything written after it is nested inside this one node. Not descending would
            // leave a namespaced script with no statements at all.
            Statement::Namespace(namespace) => StmtData::Block {
                body: self.statement_list(namespace.statements().as_slice()),
                range,
            },
            // `static $x;` and `global $x;` name a variable that is not an expression, so the
            // generic walk below never reached it and the statement bound nothing at all.
            Statement::Static(statement) => {
                let bindings = statement
                    .items
                    .iter()
                    .map(|item| match item {
                        StaticItem::Abstract(item) => self.bound_variable(&item.variable),
                        // Spelled as an assignment on purpose: everything that already reads one
                        // gives `$x` the initialiser's type with no new arm anywhere.
                        StaticItem::Concrete(item) => {
                            let target = self.bound_variable(&item.variable);
                            let value = self.expression(item.value);
                            self.builder.add_expr(ExprData::Assign {
                                operator: AssignOp::Plain,
                                target,
                                value,
                                doc_type_ref: None,
                                range: self.within_the_text(item.span()),
                            })
                        }
                    })
                    .collect::<Vec<_>>();
                StmtData::Bind {
                    kind: BindKind::Static,
                    bindings: self.builder.alloc_expr_list(&bindings),
                    range,
                }
            }
            Statement::Global(statement) => {
                let bindings = statement
                    .variables
                    .iter()
                    .filter_map(|variable| match variable {
                        Variable::Direct(direct) => Some(self.bound_variable(direct)),
                        // `global $$name` names nothing this layer can look up.
                        Variable::Indirect(_) | Variable::Nested(_) => None,
                    })
                    .collect::<Vec<_>>();
                StmtData::Bind {
                    kind: BindKind::Global,
                    bindings: self.builder.alloc_expr_list(&bindings),
                    range,
                }
            }
            other => StmtData::Unmodelled {
                inner: self.nested_expressions(Node::Statement(other)),
                range,
                passage: passage_of(other),
                kind: kind_of(other),
            },
        };
        Some(self.builder.add_stmt(data))
    }

    /// The variable a `static` or `global` names, as an expression.
    ///
    /// An expression because that is what every lookup already reads: the statement is what says a
    /// name is bound, and the node is what a cursor lands on.
    fn bound_variable(&mut self, variable: &DirectVariable<'p>) -> ExprId {
        let name = self.variable_name(variable.name);
        self.builder.add_expr(ExprData::Variable {
            name,
            range: self.within_the_text(variable.span()),
        })
    }

    fn if_statement(&mut self, statement: &If<'p>, range: TextRange) -> StmtData {
        // With the annotation, because `/** @var T|null $x */ if ($x = $storage->load($id))` is the
        // same idiom as the annotation above a plain assignment, written for the same reason: the
        // storage returns an interface and the author is saying which class it really is.
        let condition = self.expression_with_annotation(statement.condition, range.start());
        let (then_branch, else_branch) = match &statement.body {
            IfBody::Statement(body) => {
                let then_branch = self.single_statement(body.statement);
                // Built from the last clause backwards, because each `elseif` becomes the else
                // branch of the one before it.
                let mut else_branch = match &body.else_clause {
                    Some(clause) => self.single_statement(clause.statement),
                    None => IdSlice::EMPTY,
                };
                for clause in body.else_if_clauses.iter().rev() {
                    let condition = self.expression(clause.condition);
                    let then_branch = self.single_statement(clause.statement);
                    let nested = self.builder.add_stmt(StmtData::If {
                        condition,
                        then_branch,
                        else_branch,
                        range: Self::clause_range(clause.span(), range),
                    });
                    else_branch = self.builder.alloc_stmt_list(&[nested]);
                }
                (then_branch, else_branch)
            }
            IfBody::ColonDelimited(body) => {
                let then_branch = self.statement_list(body.statements.as_slice());
                let mut else_branch = match &body.else_clause {
                    Some(clause) => self.statement_list(clause.statements.as_slice()),
                    None => IdSlice::EMPTY,
                };
                for clause in body.else_if_clauses.iter().rev() {
                    let condition = self.expression(clause.condition);
                    let then_branch = self.statement_list(clause.statements.as_slice());
                    let nested = self.builder.add_stmt(StmtData::If {
                        condition,
                        then_branch,
                        else_branch,
                        range: Self::clause_range(clause.span(), range),
                    });
                    else_branch = self.builder.alloc_stmt_list(&[nested]);
                }
                (then_branch, else_branch)
            }
        };

        StmtData::If {
            condition,
            then_branch,
            else_branch,
            range,
        }
    }

    /// The range of a synthesized `if` standing in for an `elseif` clause.
    ///
    /// From the clause's own start to the end of the whole `if`, not to the end of the clause: the
    /// synthesized node owns everything after it, including the final `else`, and a node whose range
    /// does not contain its children breaks any lookup that descends by position.
    fn clause_range(clause: Span, whole: TextRange) -> TextRange {
        TextRange::new(text_range(clause).start(), whole.end())
    }

    fn foreach_statement(&mut self, statement: &Foreach<'p>, range: TextRange) -> StmtData {
        let subject = self.expression(statement.expression);
        // The value is a destructuring target as readily as an assignment's left-hand side is:
        // `foreach ($pairs as [$name, $count])` is how a list of pairs is read.
        let (key, value) = match &statement.target {
            ForeachTarget::Value(target) => (None, self.destructuring_target(target.value)),
            ForeachTarget::KeyValue(target) => (
                Some(self.expression(target.key)),
                self.destructuring_target(target.value),
            ),
        };
        let body = match &statement.body {
            ForeachBody::Statement(statement) => self.single_statement(statement),
            ForeachBody::ColonDelimited(body) => self.statement_list(body.statements.as_slice()),
        };

        StmtData::Foreach {
            subject,
            key,
            value,
            body,
            // Read for the value variable only, which is what a nameless `@var` above a loop means
            // to every other tool. A named one still has to name the value: `@var T $key` above a
            // `foreach` is not something to guess about.
            doc_type_ref: self.foreach_doc_type(value, range.start()),
            range,
        }
    }

    /// The `@var` written above a `foreach`, when it names what the value variable holds.
    fn foreach_doc_type(
        &mut self,
        value: phrost_hir::ExprId,
        statement_start: phrost_model::TextSize,
    ) -> Option<phrost_hir::TypeRefId> {
        let ExprData::Variable { name, .. } = self.builder.expr(value) else {
            return None;
        };
        let name = name.clone();
        self.doc_type(
            statement_start,
            crate::docblock::DocTag::LocalVar(name.as_str()),
        )
    }

    fn while_statement(&mut self, statement: &While<'p>, range: TextRange) -> StmtData {
        // With the annotation, for the reason `if_statement` takes it: `/** @var object|false $item */
        // while ($item = $queue->claimItem())` is the idiom for a loop over a source whose declared
        // return is wider than what it hands back, and the tag is about the assignment in the header.
        let condition = self.expression_with_annotation(statement.condition, range.start());
        let body = match &statement.body {
            WhileBody::Statement(statement) => self.single_statement(statement),
            WhileBody::ColonDelimited(body) => self.statement_list(body.statements.as_slice()),
        };
        StmtData::While {
            condition,
            body,
            tested_first: true,
            range,
        }
    }

    fn for_statement(&mut self, statement: &For<'p>, range: TextRange) -> StmtData {
        let init = self.expressions(statement.initializations.iter());
        let conditions = self.expressions(statement.conditions.iter());
        let step = self.expressions(statement.increments.iter());
        let body = match &statement.body {
            ForBody::Statement(statement) => self.single_statement(statement),
            ForBody::ColonDelimited(body) => self.statement_list(body.statements.as_slice()),
        };
        StmtData::For {
            init,
            conditions,
            step,
            body,
            range,
        }
    }

    /// Lowers a comma-separated clause of a `for` header.
    fn expressions<'a>(
        &mut self,
        expressions: impl Iterator<Item = &'a &'p mago_syntax::cst::Expression<'p>>,
    ) -> IdSlice<ExprId>
    where
        'p: 'a,
    {
        let ids = expressions
            .map(|expression| self.expression(expression))
            .collect::<Vec<_>>();
        self.builder.alloc_expr_list(&ids)
    }

    fn do_while_statement(&mut self, statement: &DoWhile<'p>, range: TextRange) -> StmtData {
        let body = self.single_statement(statement.statement);
        // The same idiom with the test at the bottom; the docblock is still above the statement.
        let condition = self.expression_with_annotation(statement.condition, range.start());
        StmtData::While {
            condition,
            body,
            tested_first: false,
            range,
        }
    }

    fn try_statement(&mut self, statement: &Try<'p>, range: TextRange) -> StmtData {
        let body = self.statement_list(statement.block.statements.as_slice());
        let catch_ids = statement
            .catch_clauses
            .iter()
            .map(|clause| self.catch_clause(clause))
            .collect::<Vec<_>>();
        let catches = self.builder.alloc_catch_list(&catch_ids);
        let finally = match &statement.finally_clause {
            Some(clause) => self.statement_list(clause.block.statements.as_slice()),
            None => IdSlice::EMPTY,
        };

        StmtData::Try {
            body,
            catches,
            finally,
            range,
        }
    }

    fn catch_clause(&mut self, clause: &mago_syntax::cst::TryCatchClause<'p>) -> CatchId {
        let type_ref: Option<TypeRefId> = Some(self.hint(&clause.hint));
        let variable = clause
            .variable
            .as_ref()
            .map(|variable| self.variable_name(variable.name));
        let variable_range = clause
            .variable
            .as_ref()
            .map(|variable| self.within_the_text(variable.span));
        let body = self.statement_list(clause.block.statements.as_slice());

        self.builder.add_catch(CatchData {
            type_ref,
            variable,
            variable_range,
            body,
            range: self.within_the_text(clause.span()),
        })
    }

    fn switch_statement(&mut self, statement: &Switch<'p>, range: TextRange) -> StmtData {
        let subject = self.expression(statement.expression);
        let case_ids = self.switch_cases(statement.body.cases());
        let cases = self.builder.alloc_switch_case_list(&case_ids);

        StmtData::Switch {
            subject,
            cases,
            range,
        }
    }

    /// Lowers a switch's `case`/`default` clauses, folding a run of empty-bodied labels into the
    /// arm whose body they fall through to -- see [`SwitchCaseData`].
    ///
    /// `case 1: case 2: echo "x"; break;` writes two labels but runs one body, and a consumer
    /// asking "does this arm leave" needs that answered from one place rather than by walking
    /// forward across clauses itself.
    fn switch_cases(&mut self, cases: &[SwitchCase<'p>]) -> Vec<SwitchCaseId> {
        let mut ids = Vec::new();
        let mut conditions: Vec<ExprId> = Vec::new();
        let mut is_default = false;
        let mut start: Option<TextSize> = None;

        for case in cases {
            let case_range = self.within_the_text(case.span());
            if start.is_none() {
                start = Some(case_range.start());
            }
            if let Some(condition) = case.expression() {
                conditions.push(self.expression(condition));
            }
            is_default |= case.is_default();

            let statements = case.statements();
            if statements.is_empty() {
                // Falls through: the next label's body is this arm's body too.
                continue;
            }

            let body = self.statement_list(statements);
            let arm_conditions = self.builder.alloc_expr_list(&conditions);
            ids.push(self.builder.add_switch_case(SwitchCaseData {
                conditions: arm_conditions,
                is_default,
                body,
                range: TextRange::new(start.take().expect("set above"), case_range.end()),
            }));
            conditions.clear();
            is_default = false;
        }

        // A trailing run of empty-bodied labels with nothing left to fall into, e.g. `case 1:
        // case 2:` at the end of the switch -- still an arm, just one with no statements to run.
        if let Some(start) = start.take() {
            let end = self
                .within_the_text(
                    cases
                        .last()
                        .expect("a pending run implies at least one case")
                        .span(),
                )
                .end();
            let arm_conditions = self.builder.alloc_expr_list(&conditions);
            ids.push(self.builder.add_switch_case(SwitchCaseData {
                conditions: arm_conditions,
                is_default,
                body: IdSlice::EMPTY,
                range: TextRange::new(start, end),
            }));
        }

        ids
    }

    /// A single statement used as a body: `if ($x) return;` has no block to flatten.
    fn single_statement(&mut self, statement: &Statement<'p>) -> IdSlice<StmtId> {
        match statement {
            Statement::Block(block) => self.statement_list(block.statements.as_slice()),
            other => match self.body_statement(other) {
                Some(id) => self.builder.alloc_stmt_list(&[id]),
                None => IdSlice::EMPTY,
            },
        }
    }
}

/// Whether an unmodelled statement declares something rather than running.
///
/// A class, an interface, a trait, an enum, a function, a top-level `const`, an import. What is
/// written inside one -- a property default, a class constant, a parameter default, an attribute's
/// argument -- is a constant expression that runs when the declaration is *loaded*, not where the
/// statement sits, and it is lowered as a nested expression of this statement so goto and hover can
/// reach it. `extractable_at` is the reader that needs the two told apart: it offered to lift one
/// into a variable above the declaration, which PHP refuses.
fn kind_of(statement: &Statement<'_>) -> UnmodelledKind {
    match statement {
        Statement::Class(_)
        | Statement::Interface(_)
        | Statement::Trait(_)
        | Statement::Enum(_)
        | Statement::Function(_)
        | Statement::Constant(_)
        | Statement::Use(_) => UnmodelledKind::Declaration,
        Statement::Unset(_) => UnmodelledKind::Unset,
        _ => UnmodelledKind::Statement,
    }
}

/// What control does at a statement this layer does not model, decided while the node is still in
/// hand.
///
/// Exhaustive over what actually reaches the unmodelled arm, which is fourteen of mago's
/// thirty-three statement shapes and a closed list -- everything else has a `StmtData` of its own.
/// Written as a `match` with no catch-all for that reason: a statement that stops being modelled
/// elsewhere has to be classified here rather than silently inheriting somebody's default.
fn passage_of(statement: &Statement<'_>) -> Passage {
    match statement {
        // Walks straight through. `echo` is the one this was written for: it was answered the same
        // way as `goto`, so a dead region beginning with one was dropped whole.
        Statement::Echo(_)
        | Statement::EchoTag(_)
        | Statement::Unset(_)
        | Statement::Declare(_) => Passage::Onward,
        // Leaves for somewhere this layer does not follow.
        Statement::Goto(_) | Statement::HaltCompiler(_) => Passage::Elsewhere,
        // Control can arrive here from elsewhere. A label is a `goto` target; a declaration is
        // hoisted, so it exists whatever it was written after and nothing following it is dead on
        // account of what came before.
        Statement::Label(_)
        | Statement::Class(_)
        | Statement::Interface(_)
        | Statement::Trait(_)
        | Statement::Enum(_)
        | Statement::Function(_)
        | Statement::Constant(_)
        | Statement::Use(_) => Passage::Arrival,
        // Everything else has a modelled shape and never reaches here. Answered as the safe
        // direction rather than with a panic: a mago version adding a statement would otherwise
        // take the server down instead of being slightly vague about it.
        _ => Passage::Arrival,
    }
}
