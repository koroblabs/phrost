//! Expressions, lowered.
//!
//! The other half of body lowering, split from the statements because the two are read for
//! different reasons: a statement is a place, an expression is a value. Growing one should not mean
//! opening the file the other lives in.
//!
//! Same rule as the statements: shapes that carry a name, a receiver, or a type, and `Unmodelled`
//! for the rest — with its nested expressions still lowered, so a shape this does not model never
//! hides what is written inside it.
//!
//! This module holds the top-level dispatch and the handful of expression forms with nowhere more
//! specific to live. Everything else is a submodule by responsibility: [`access`] (member and
//! static access), [`array`] (array literals and elements), [`calls`] (calls, partial application
//! and argument lists), [`class_target`] (the class in a `new`/`::`/`instanceof`), [`closures`]
//! (closures and arrow functions), [`literals`] (literal values read out of arguments and array
//! keys), [`narrowing`] (the operators that narrow a later read: `&&`/`||`, null and `false`
//! comparisons, `instanceof`, `isset`), and [`walk`] (the generic unmodelled-node walk).

mod access;
pub(crate) mod array;
mod calls;
mod class_target;
mod closures;
mod literals;
mod narrowing;
mod walk;

// Re-exported so `crate::expression::keyword_of` keeps resolving for callers outside this module
// (`docblock.rs`) exactly as it did before the split.
pub(crate) use literals::keyword_of;

use mago_span::HasSpan as _;
use mago_syntax::cst::{Assignment, Expression, Instantiation, Match, MatchArm, Node, Variable};
use phrost_hir::{
    ExprId, ImportKind, MatchArmId,
    body::{AssignOp, ExprData, LiteralKind, MatchArmData, NullTestKind},
};
use phrost_model::{BuiltinTy, Name, TextRange};
use phrost_syntax::identifier_text;

use crate::{docblock::DocTag, lowerer::Lowerer};

impl<'p> Lowerer<'_, 'p> {
    pub(crate) fn expression(&mut self, expression: &Expression<'p>) -> ExprId {
        let range = self.within_the_text(expression.span());
        // Noted before the match rather than as an arm of it, because what a `yield` *produces* is
        // not modelled and should not be: the generic walk already keeps whatever names its
        // operands hold. That it happened at all is the part nothing else can recover, and it is
        // what tells a return check it is looking at a generator.
        if matches!(expression, Expression::Yield(_)) {
            self.saw_yield = true;
        }
        let data = match expression {
            // Parentheses are grouping, not structure. Unwrapping here means no consumer has to.
            Expression::Parenthesized(inner) => return self.expression(inner.expression),
            Expression::Variable(variable) => self.variable(variable, range),
            Expression::Assignment(assignment) => self.assignment(assignment, range),
            Expression::Instantiation(instantiation) => self.instantiation(instantiation, range),
            Expression::AnonymousClass(anonymous) => self.anonymous_class(anonymous, range),
            // `clone $row` is another of whatever it was handed, and the operand is an ordinary
            // expression written where it stands.
            Expression::Clone(cloned) => ExprData::Cloned {
                value: self.expression(cloned.object),
                range,
            },
            Expression::Call(call) => self.call(call, range),
            Expression::PartialApplication(partial) => self.partial_application(partial, range),
            Expression::Pipe(pipe) => self.pipe(pipe, range),
            Expression::Access(access) => self.access(access, range),
            Expression::Literal(literal) => ExprData::Literal {
                kind: match literal {
                    mago_syntax::cst::Literal::String(_) => LiteralKind::String,
                    mago_syntax::cst::Literal::Integer(_) => LiteralKind::Int,
                    mago_syntax::cst::Literal::Float(_) => LiteralKind::Float,
                    mago_syntax::cst::Literal::True(_) => LiteralKind::True,
                    mago_syntax::cst::Literal::False(_) => LiteralKind::False,
                    mago_syntax::cst::Literal::Null(_) => LiteralKind::Null,
                },
                range,
            },
            Expression::ConstantAccess(access) => ExprData::ConstantAccess {
                name: self.name_ref_of(&access.name, ImportKind::Constant),
                range,
            },
            Expression::Identifier(identifier) => ExprData::ConstantAccess {
                name: self.name_ref_of(identifier, ImportKind::Constant),
                range,
            },
            Expression::Binary(binary) => return self.binary(binary, range),
            Expression::Conditional(conditional) => ExprData::Conditional {
                condition: self.expression(conditional.condition),
                then_branch: conditional.then.map(|then| self.expression(then)),
                else_branch: self.expression(conditional.r#else),
                range,
            },
            // `!` and `throw` are modelled for the same reason as the logical operators: a guard is
            // read backwards, and a statement that throws is one the code after does not follow.
            Expression::UnaryPrefix(unary) => {
                // `@$row['k']` is the value `$row['k']` has, with the engine's warning silenced.
                // The `@` is not an operator that produces anything, so it leaves no node: left as
                // an `Unmodelled` it said the expression had no type at all, and `$x = @$a['k'];`
                // -- 622 occurrences in Symfony, 226 in WordPress -- was judged by nobody. What it
                // suppresses is a *diagnostic of PHP's*, not a type: a `TypeError` still throws
                // through it, which is why passing the type on is the honest reading and why the
                // syntax rule that objects to `@` at all is where the objection belongs.
                if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPrefixOperator::ErrorControl(_)
                ) {
                    return self.expression(unary.operand);
                }
                if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPrefixOperator::Not(_)
                ) {
                    ExprData::Not {
                        value: self.expression(unary.operand),
                        range,
                    }
                } else if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPrefixOperator::Reference(_)
                ) {
                    // The reference wraps what is bound rather than replacing it, so the variable
                    // inside stays a variable: `foreach ($rows as &$row)` binds `$row`.
                    ExprData::Reference {
                        value: self.expression(unary.operand),
                        range,
                    }
                } else if let Some(to) = cast_target(&unary.operator) {
                    // A cast states the type outright, so it is the one operator whose result needs
                    // nothing inferred.
                    ExprData::Cast {
                        to,
                        value: self.expression(unary.operand),
                        range,
                    }
                } else if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPrefixOperator::PreIncrement(_)
                        | mago_syntax::cst::UnaryPrefixOperator::PreDecrement(_)
                ) {
                    // A write with no right-hand side. Modelled so a reader asking what puts a
                    // value in a name finds it: `$i++` beside `use (&$i)` is two writes, and a
                    // reader that saw neither offered to inline `$i` and wrote `use (&0)`.
                    ExprData::CompoundAssign {
                        target: self.expression(unary.operand),
                        value: None,
                        produces: None,
                        arithmetic: None,
                        range,
                    }
                } else if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPrefixOperator::Negation(_)
                        | mago_syntax::cst::UnaryPrefixOperator::Plus(_)
                ) {
                    ExprData::Signed {
                        value: self.expression(unary.operand),
                        range,
                    }
                } else if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPrefixOperator::BitwiseNot(_)
                ) {
                    // An int, as every other bitwise operator's result is -- the string a `~` over a
                    // string gives back is a byte-flipping trick nobody writes where a type is read.
                    ExprData::Operator {
                        produces: phrost_model::BuiltinTy::Int,
                        inner: self.nested_expressions(Node::UnaryPrefix(unary)),
                        range,
                    }
                } else {
                    ExprData::Unmodelled {
                        inner: self.nested_expressions(Node::UnaryPrefix(unary)),
                        range,
                    }
                }
            }
            // An array literal, in either spelling. Its elements are lowered one at a time rather
            // than by the generic walk, which flattened keys and values into one list and so lost
            // which was which -- see `array_element`.
            Expression::Array(array) => self.array_or_callable(array.elements.as_slice(), range),
            Expression::LegacyArray(array) => {
                self.array_or_callable(array.elements.as_slice(), range)
            }
            // `list($a, , 'k' => $c) = $row`, which since PHP 7.1 is the same construct as
            // `[$a, , 'k' => $c] = $row` down to the keys and the holes -- and it only ever appears
            // as an assignment or `foreach` target, so reading it as a literal claims nothing about
            // a value. One shape, because every reader of a destructuring target matches
            // `ArrayLiteral`: left as an `Unmodelled` it kept its variables as nested expressions,
            // so `occurrences::collect_bindings` saw uses of names nothing bound and
            // `list($xlen) = unpack(...); return $xlen;` reported the binding itself as unbound.
            Expression::List(list) => ExprData::ArrayLiteral {
                elements: self.array_elements(list.elements.as_slice()),
                range,
            },
            // A write to an element creates the variable it is written through, so this is a
            // binding site and not just a read: `$rows['a'] = 1` is where `$rows` comes from.
            Expression::ArrayAccess(access) => ExprData::Index {
                base: self.expression(access.array),
                key: self.literal_key(access.index),
                index: Some(self.expression(access.index)),
                range,
            },
            Expression::ArrayAppend(append) => ExprData::Index {
                base: self.expression(append.array),
                index: None,
                key: None,
                range,
            },
            Expression::Throw(throw) => ExprData::Throw {
                value: self.expression(throw.exception),
                range,
            },
            Expression::Closure(closure) => self.closure(closure, range),
            Expression::ArrowFunction(function) => self.arrow_function(function, range),
            Expression::Match(match_expression) => self.match_expression(match_expression, range),
            // `isset` and `empty` are null tests spelled as constructs. `isset($x)` is exactly
            // `$x !== null` for anything with a name; `empty($x)` is exactly `!$x`.
            Expression::Construct(construct) => match construct {
                mago_syntax::cst::Construct::Isset(isset) => {
                    match isset.values.iter().collect::<Vec<_>>().as_slice() {
                        // `isset($a, $b)` is an `and` of both, and the walk below builds it as one so
                        // narrowing needs no separate shape for it.
                        [] => ExprData::Unmodelled {
                            inner: self.nested_expressions(Node::Construct(construct)),
                            range,
                        },
                        values => return self.isset(values, range),
                    }
                }
                mago_syntax::cst::Construct::Empty(empty) => ExprData::NullTest {
                    value: self.expression(empty.value),
                    kind: NullTestKind::Empty,
                    range,
                },
                // The four spellings of an include are one shape: what a reader of scopes needs is
                // that a file was pulled in here, not which keyword did it.
                mago_syntax::cst::Construct::Include(include) => ExprData::Include {
                    path: self.expression(include.value),
                    range,
                },
                mago_syntax::cst::Construct::IncludeOnce(include) => ExprData::Include {
                    path: self.expression(include.value),
                    range,
                },
                mago_syntax::cst::Construct::Require(require) => ExprData::Include {
                    path: self.expression(require.value),
                    range,
                },
                mago_syntax::cst::Construct::RequireOnce(require) => ExprData::Include {
                    path: self.expression(require.value),
                    range,
                },
                // `exit` and `die` are one shape: nothing runs after either, and which keyword
                // was written is not something any reader here asks.
                mago_syntax::cst::Construct::Exit(_) | mago_syntax::cst::Construct::Die(_) => {
                    ExprData::Terminate {
                        inner: self.nested_expressions(Node::Construct(construct)),
                        range,
                    }
                }
                _ => ExprData::Unmodelled {
                    inner: self.nested_expressions(Node::Construct(construct)),
                    range,
                },
            },
            // `$i++` and `$i--`, the other spelling of the write above.
            Expression::UnaryPostfix(unary)
                if matches!(
                    unary.operator,
                    mago_syntax::cst::UnaryPostfixOperator::PostIncrement(_)
                        | mago_syntax::cst::UnaryPostfixOperator::PostDecrement(_)
                ) =>
            {
                ExprData::CompoundAssign {
                    target: self.expression(unary.operand),
                    value: None,
                    produces: None,
                    arithmetic: None,
                    range,
                }
            }
            // `"order.{$field}[0]"` and its heredoc twin: a string whatever the holes hold, which
            // is what PHP's own interpolation is for. Left unmodelled they said nothing at all --
            // every local assigned one was untyped, and `$path = "a/$b"; f($path);` was judged by
            // nobody -- while the `.` beside them already produced a `string` through this same
            // node. The holes stay where every other nested expression goes.
            // `__LINE__` is an `int` and the other nine are `string`s, which is the whole of what
            // they are: the compiler substitutes a literal, so this is the same kind of fact as a
            // quoted string and not a name anything resolves. Left unmodelled they said nothing,
            // and `$class = __CLASS__;` was untyped -- 1 446 of them in WordPress, 2 973 in
            // Symfony, mostly as one side of a `.` that already produced a string on its own.
            Expression::MagicConstant(magic) => ExprData::Literal {
                kind: match magic {
                    mago_syntax::cst::MagicConstant::Line(_) => LiteralKind::Int,
                    mago_syntax::cst::MagicConstant::File(_)
                    | mago_syntax::cst::MagicConstant::Directory(_)
                    | mago_syntax::cst::MagicConstant::Trait(_)
                    | mago_syntax::cst::MagicConstant::Method(_)
                    | mago_syntax::cst::MagicConstant::Function(_)
                    | mago_syntax::cst::MagicConstant::Property(_)
                    | mago_syntax::cst::MagicConstant::Namespace(_)
                    | mago_syntax::cst::MagicConstant::Class(_) => LiteralKind::String,
                },
                range,
            },
            // What it holds is what the text around its holes does -- see `string_holding`.
            Expression::CompositeString(composite) => ExprData::Operator {
                produces: crate::string_holding::composite_holds(composite),
                inner: self.nested_expressions(Node::Expression(expression)),
                range,
            },
            other => ExprData::Unmodelled {
                inner: self.nested_expressions(Node::Expression(other)),
                range,
            },
        };
        self.builder.add_expr(data)
    }

    fn variable(&mut self, variable: &Variable<'p>, range: TextRange) -> ExprData {
        match variable {
            Variable::Direct(direct) => ExprData::Variable {
                name: self.variable_name(direct.name),
                range,
            },
            // `$$name` and `${expr}`: there is no name to look up, and treating one as an ordinary
            // variable would answer about whichever variable happened to share the spelling.
            Variable::Indirect(_) | Variable::Nested(_) => ExprData::IndirectVariable { range },
        }
    }

    fn assignment(&mut self, assignment: &Assignment<'p>, range: TextRange) -> ExprData {
        // `=` and `??=`. Every other compound assignment has a result the operator decides rather
        // than the right-hand side, so calling one an assignment would give the target the wrong
        // type -- `$n .= 'x'` is a string however `$n` was written. It keeps its own shape rather
        // than falling to `Unmodelled`, because the target is still a **write** and a reader that
        // could not see it inlined `$offset += $batch_size` into `0 += $batch_size`.
        let Some(operator) = assign_operator(&assignment.operator) else {
            let (produces, arithmetic) = compound_result(&assignment.operator);
            // `$n .= 'x'` holds at least what was appended, whatever `$n` held before.
            let produces = produces.map(|produced| match produced.is_string() {
                true => crate::string_holding::piece_holds(assignment.rhs),
                false => produced,
            });
            return ExprData::CompoundAssign {
                target: self.destructuring_target(assignment.lhs),
                value: Some(self.expression(assignment.rhs)),
                produces,
                arithmetic,
                range,
            };
        };

        ExprData::Assign {
            operator,
            target: self.destructuring_target(assignment.lhs),
            value: self.expression(assignment.rhs),
            // Filled in by `expression_with_annotation`, which is the only caller that knows where
            // the statement starts.
            doc_type_ref: None,
            range,
        }
    }

    /// An assignment or `foreach` target, which is the one place an array literal binds rather than
    /// builds -- and the one place its elements need to say which slot each of them is.
    ///
    /// Every other spelling lowers as it always did: only a literal *destructures*.
    pub(crate) fn destructuring_target(&mut self, target: &Expression<'p>) -> ExprId {
        let range = self.within_the_text(target.span());
        let elements = match target {
            Expression::Parenthesized(inner) => return self.destructuring_target(inner.expression),
            Expression::Array(array) => array.elements.as_slice(),
            Expression::LegacyArray(array) => array.elements.as_slice(),
            Expression::List(list) => list.elements.as_slice(),
            other => return self.expression(other),
        };
        let elements = self.destructuring_elements(elements);
        self.builder
            .add_expr(ExprData::ArrayLiteral { elements, range })
    }

    fn instantiation(&mut self, instantiation: &Instantiation<'p>, range: TextRange) -> ExprData {
        ExprData::New {
            class: self.class_target(instantiation.class),
            args: self.arguments(instantiation.argument_list.as_ref()),
            range,
        }
    }

    /// `new class extends Base implements Contract { ... }`.
    ///
    /// Lowered as the class it is. Before this it fell into the generic walk, which -- correctly,
    /// for a *named* class -- steps around a class body because that body is lowered as a
    /// declaration elsewhere. An anonymous one is declared nowhere else, so the guard removed the
    /// only copy and the whole shape vanished: no supertypes, no members, no locals, no name
    /// references. Goto, hover, find-references, rename and every diagnostic were dead inside one,
    /// and `new class implements X { ... }` is how a test double is written without a mock library.
    ///
    /// The name is synthesised from where the `class` keyword sits and from what the whole file
    /// says, which makes it deterministic from the file's content -- `incremental == from-scratch`
    /// compares these renderings, so a counter or an address would not do -- and unique across the
    /// project, which the offset alone was not. See [`crate::lowerer::Lowerer::file_mark`]: every
    /// migration Laravel generates puts its `class` keyword on the same byte. It claims no
    /// fully-qualified name: see [`phrost_hir::ClassData::is_anonymous`].
    ///
    /// The value is the class, and reaching it needs no name a person could write: the synthesised
    /// one is claimed in the catalog like any other -- which is what makes `$this` work inside the
    /// body -- and is marked unnameable only so that completion and the outline never offer it. So
    /// the expression names it the same way `new Foo` names `Foo`, and `$x = new class { ... }`
    /// then types `$x`, resolves its members, and reports the ones it does not have. A test double
    /// written without a mock library is the shape, and before this every call on one answered
    /// nothing at all.
    fn anonymous_class(
        &mut self,
        anonymous: &mago_syntax::cst::AnonymousClass<'p>,
        range: TextRange,
    ) -> ExprData {
        let keyword = self.within_the_text(anonymous.class.span());
        self.attribute_names(&anonymous.attribute_lists);
        let name = format!(
            "{}{}@{:08x}",
            phrost_hir::ANONYMOUS_CLASS_PREFIX,
            u32::from(keyword.start()),
            self.file_mark()
        );
        self.class_like(crate::lowerer::ClassLikeParts {
            is_anonymous: true,
            kind: phrost_hir::ClassKind::Class,
            modifiers: crate::lowerer::class_modifiers(&anonymous.modifiers),
            name: name.clone(),
            name_range: keyword,
            extends: anonymous.extends.as_ref(),
            implements: anonymous.implements.as_ref(),
            enum_backing: None,
            members: &anonymous.members,
            attributes: &anonymous.attribute_lists,
            declaration_start: crate::lowerer::declaration_start(
                &anonymous.attribute_lists,
                Some(&anonymous.modifiers),
                anonymous.class.span(),
            ),
            full_range: range,
            // The braces alone. Everything before them -- the arguments most of all -- is written
            // in the scope the `new` is in.
            body_range: self.within_the_text(anonymous.left_brace.join(anonymous.right_brace)),
        });

        // The class the expression produces, named by the same synthesised name the declaration
        // claimed. Relative to the file's namespace, which is where the claim was made, so the
        // ordinary resolution finds it with no special case of its own.
        let path = self.interner.name(&name);
        let class =
            phrost_hir::body::ClassTarget::Named(self.add_name_ref(phrost_hir::NameRefData {
                path,
                anchor: phrost_hir::NameAnchor::Relative,
                kind: phrost_hir::ImportKind::ClassLike,
                range: keyword,
                in_docblock: false,
                cross_reference: false,
            }));

        // The constructor arguments are written outside the body and are values like any other.
        ExprData::New {
            class,
            args: self.partial_arguments(anonymous.argument_list.as_ref()),
            range,
        }
    }

    fn match_expression(&mut self, expression: &Match<'p>, range: TextRange) -> ExprData {
        let subject = self.expression(expression.expression);
        let arm_ids = expression
            .arms
            .iter()
            .map(|arm| self.match_arm(arm))
            .collect::<Vec<_>>();
        let arms = self.builder.alloc_match_arm_list(&arm_ids);

        ExprData::Match {
            subject,
            arms,
            range,
        }
    }

    /// One `conditions => value` or `default => value` arm. Unlike a `switch` case, there is no
    /// fallthrough to fold: every arm already writes its own result expression.
    fn match_arm(&mut self, arm: &MatchArm<'p>) -> MatchArmId {
        let conditions = match arm {
            MatchArm::Expression(expression_arm) => expression_arm
                .conditions
                .iter()
                .map(|&condition| self.expression(condition))
                .collect::<Vec<_>>(),
            MatchArm::Default(_) => Vec::new(),
        };
        let conditions = self.builder.alloc_expr_list(&conditions);
        let body = self.expression(arm.expression());

        self.builder.add_match_arm(MatchArmData {
            conditions,
            body,
            range: self.within_the_text(arm.span()),
        })
    }

    pub(crate) fn variable_name(&mut self, name: &'p [u8]) -> Name {
        self.interner
            .name(identifier_text(name).trim_start_matches('$'))
    }

    /// Lowers an expression, attaching a `/** @var T $x */` written above the statement.
    ///
    /// Only to a plain assignment, and only when the tag names that variable or names none: the tag
    /// describes what the variable holds, so hanging it on anything else would attach a type to a
    /// value nobody claimed.
    pub(crate) fn expression_with_annotation(
        &mut self,
        expression: &Expression<'p>,
        statement_start: phrost_model::TextSize,
    ) -> ExprId {
        let id = self.expression(expression);

        let ExprData::Assign { target, .. } = self.builder.expr(id) else {
            return id;
        };
        let ExprData::Variable { name, .. } = self.builder.expr(*target) else {
            return id;
        };
        let name = name.clone();

        if let Some(annotated) = self.doc_type(statement_start, DocTag::LocalVar(name.as_str()))
            && let ExprData::Assign { doc_type_ref, .. } = self.builder.expr_mut(id)
        {
            *doc_type_ref = Some(annotated);
        }

        id
    }
}

/// Which assignment this is, or `None` for a compound one whose result the operator decides.
fn assign_operator(operator: &mago_syntax::cst::AssignmentOperator) -> Option<AssignOp> {
    use mago_syntax::cst::AssignmentOperator as Op;

    Some(match operator {
        Op::Assign(_) => AssignOp::Plain,
        Op::Coalesce(_) => AssignOp::Coalesce,
        _ => return None,
    })
}

/// What a compound assignment's operator makes: a builtin it settles alone, or the arithmetic
/// operator whose operands settle it. The same split the binary operators make in
/// `narrowing.rs`, for the same operators written with a `=`.
fn compound_result(
    operator: &mago_syntax::cst::AssignmentOperator,
) -> (Option<BuiltinTy>, Option<phrost_hir::body::ArithmeticOp>) {
    use mago_syntax::cst::AssignmentOperator as Op;
    use phrost_hir::body::ArithmeticOp;

    match operator {
        Op::Concat(_) => (Some(BuiltinTy::String), None),
        Op::BitwiseAnd(_)
        | Op::BitwiseOr(_)
        | Op::BitwiseXor(_)
        | Op::LeftShift(_)
        | Op::RightShift(_)
        | Op::Modulo(_) => (Some(BuiltinTy::Int), None),
        Op::Addition(_) => (None, Some(ArithmeticOp::Add)),
        Op::Subtraction(_) => (None, Some(ArithmeticOp::Subtract)),
        Op::Multiplication(_) => (None, Some(ArithmeticOp::Multiply)),
        Op::Division(_) => (None, Some(ArithmeticOp::Divide)),
        Op::Exponentiation(_) => (None, Some(ArithmeticOp::Power)),
        Op::Assign(_) | Op::Coalesce(_) => (None, None),
    }
}

/// The type a cast operator produces, or `None` for an operator that is not a cast.
///
/// PHP spells several of these two ways -- `(bool)` and `(boolean)`, `(int)` and `(integer)`,
/// `(float)`, `(double)` and `(real)` -- and they mean the same thing, so they collapse here.
/// `(unset)` and `(void)` are not types anything holds and stay unmodelled.
fn cast_target(operator: &mago_syntax::cst::UnaryPrefixOperator<'_>) -> Option<BuiltinTy> {
    use mago_syntax::cst::UnaryPrefixOperator as Op;

    Some(match operator {
        Op::ArrayCast(..) => BuiltinTy::Array,
        Op::BoolCast(..) | Op::BooleanCast(..) => BuiltinTy::Bool,
        Op::DoubleCast(..) | Op::RealCast(..) | Op::FloatCast(..) => BuiltinTy::Float,
        Op::IntCast(..) | Op::IntegerCast(..) => BuiltinTy::Int,
        Op::ObjectCast(..) => BuiltinTy::Object,
        Op::StringCast(..) | Op::BinaryCast(..) => BuiltinTy::String,
        _ => return None,
    })
}
