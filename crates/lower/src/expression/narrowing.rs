//! The operators a later read narrows on: `&&`/`||`, null and `false` comparisons, `instanceof`,
//! and `isset`/`empty`'s `and`-of-tests expansion.

use phrost_hir::{
    ExprId,
    body::{ArithmeticOp, ExprData, LogicalOp, NullTestKind},
};
use phrost_model::{BuiltinTy, TextRange};

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// Only the operators narrowing reads are modelled: `instanceof`, and the `&&`/`||` that decide
    /// whether one of them holds.
    pub(crate) fn binary(
        &mut self,
        binary: &mago_syntax::cst::Binary<'p>,
        range: TextRange,
    ) -> ExprId {
        use mago_syntax::cst::BinaryOperator as Op;

        let logical = match binary.operator {
            Op::And(_) | Op::LowAnd(_) => Some(LogicalOp::And),
            Op::Or(_) | Op::LowOr(_) => Some(LogicalOp::Or),
            _ => None,
        };
        if let Some(operator) = logical {
            let left = self.expression(binary.lhs);
            let right = self.expression(binary.rhs);
            return self.builder.add_expr(ExprData::Logical {
                operator,
                left,
                right,
                range,
            });
        }

        // `??`, modelled for what it does to its left operand rather than for its own value.
        if matches!(binary.operator, Op::NullCoalesce(_)) {
            let left = self.expression(binary.lhs);
            let right = self.expression(binary.rhs);
            return self
                .builder
                .add_expr(ExprData::Coalesce { left, right, range });
        }

        // A comparison against `null`, in any of its four spellings and from either side. Yoda
        // order is included because generated code writes it.
        if let Some((kind, negated)) = null_comparison(&binary.operator) {
            let tested = match (is_null_literal(binary.lhs), is_null_literal(binary.rhs)) {
                (false, true) => Some(binary.lhs),
                (true, false) => Some(binary.rhs),
                // `null === null` and `$a === $b` say nothing about one value being null.
                _ => None,
            };
            if let Some(tested) = tested {
                let value = self.expression(tested);
                let test = self
                    .builder
                    .add_expr(ExprData::NullTest { value, kind, range });
                return match negated {
                    true => self.builder.add_expr(ExprData::Not { value: test, range }),
                    false => test,
                };
            }
        }

        // The same, for a strict comparison against `false`. Only strict: `$x == false` is a
        // truthiness test and says nothing about the value being `false` in particular.
        if let Some(negated) = false_comparison(&binary.operator) {
            // Which side holds the literal, and which literal it is. Both sides being one --
            // `false === false` -- says nothing about any value and is left as an ordinary
            // comparison, the way it was when only `false` was read here.
            let tested = match (bool_literal(binary.lhs), bool_literal(binary.rhs)) {
                (None, Some(expected)) => Some((binary.lhs, expected)),
                (Some(expected), None) => Some((binary.rhs, expected)),
                _ => None,
            };
            if let Some((tested, expected)) = tested {
                let value = self.expression(tested);
                let test = self.builder.add_expr(ExprData::BoolTest {
                    value,
                    expected,
                    range,
                });
                return match negated {
                    true => self.builder.add_expr(ExprData::Not { value: test, range }),
                    false => test,
                };
            }
        }

        // `count($rows) === 0` is `empty($rows)` written out, and PHP's own way of asking it of a
        // `Countable`. Lowered as the same test, so the guard machinery reads both with one rule:
        // what matters downstream is the negation -- `count($rows) !== 0` above a `reset($rows)` is
        // how half the standard library's failure branches are ruled out.
        if let Some((counted, kind, negated)) = counting_comparison(binary) {
            let value = self.expression(counted);
            let test = self
                .builder
                .add_expr(ExprData::NullTest { value, kind, range });
            return match negated {
                true => self.builder.add_expr(ExprData::Not { value: test, range }),
                false => test,
            };
        }

        // A comparison, which answers a bool and says *which* comparison it was -- the half the
        // bool-producing catch-all below used to throw away, leaving `$count > 0` and `$count <= 0`
        // indistinguishable and neither of them able to rule null out of a branch.
        if let Some(operator) = compare_operator(&binary.operator) {
            let left = self.expression(binary.lhs);
            let right = self.expression(binary.rhs);
            return self.builder.add_expr(ExprData::Comparison {
                operator,
                left,
                right,
                range,
            });
        }

        // Operators whose result the operator alone decides. `.` builds a string, and the bitwise
        // ones and `%` produce an int whatever they were given.
        let produces = match binary.operator {
            // As much as the piece that holds the most -- see `string_holding`.
            Op::StringConcat(_) => Some(
                crate::string_holding::piece_holds(binary.lhs)
                    .concatenated(crate::string_holding::piece_holds(binary.rhs)),
            ),
            Op::BitwiseAnd(_)
            | Op::BitwiseOr(_)
            | Op::BitwiseXor(_)
            | Op::LeftShift(_)
            | Op::RightShift(_)
            | Op::Modulo(_)
            | Op::Spaceship(_) => Some(BuiltinTy::Int),
            _ => None,
        };
        if let Some(produces) = produces {
            let inner = self.nested_expressions(mago_syntax::cst::Node::Binary(binary));
            return self.builder.add_expr(ExprData::Operator {
                produces,
                inner,
                range,
            });
        }

        // The operators the *operands* decide, resolved wherever both can be. Which one was written
        // still matters: only `+` accepts two arrays, and only `/` can turn two ints into a float.
        if let Some(operator) = arithmetic_operator(&binary.operator) {
            let left = self.expression(binary.lhs);
            let right = self.expression(binary.rhs);
            return self.builder.add_expr(ExprData::Arithmetic {
                operator,
                left,
                right,
                range,
            });
        }

        if !matches!(binary.operator, Op::Instanceof(_)) {
            let inner = self.nested_expressions(mago_syntax::cst::Node::Binary(binary));
            return self.builder.add_expr(ExprData::Unmodelled { inner, range });
        }

        let value = self.expression(binary.lhs);
        let class = self.class_target(binary.rhs);
        self.builder.add_expr(ExprData::Instanceof {
            value,
            class,
            range,
        })
    }

    /// `isset($a, $b, ...)` as the `and` of one test per value, so a guard reads it with the
    /// connective machinery it already has.
    pub(crate) fn isset(
        &mut self,
        values: &[&&'p mago_syntax::cst::Expression<'p>],
        range: TextRange,
    ) -> ExprId {
        let mut combined = None;
        for value in values {
            let value = self.expression(value);
            let test = self.builder.add_expr(ExprData::NullTest {
                value,
                kind: NullTestKind::Isset,
                range,
            });
            let present = self.builder.add_expr(ExprData::Not { value: test, range });
            combined = Some(match combined {
                None => present,
                Some(left) => self.builder.add_expr(ExprData::Logical {
                    operator: LogicalOp::And,
                    left,
                    right: present,
                    range,
                }),
            });
        }
        combined.expect("the caller rejected an empty list")
    }
}

/// Which arithmetic operator this is, for the five whose result their operands decide.
///
/// `%`, the bitwise operators and `<=>` are not here: each answers `int` whatever it was given, so
/// they are settled a dozen lines above without looking at either side.
/// Which comparison a binary operator is, or `None` for one that is not a comparison.
///
/// A table over a closed list rather than a catch-all, for the reason `Passage` is one: the node
/// still has the fact and a reader below needs it. `<>` folds into `!=`, being the same operator
/// with an older spelling.
fn compare_operator(
    operator: &mago_syntax::cst::BinaryOperator<'_>,
) -> Option<phrost_hir::body::CompareOp> {
    use mago_syntax::cst::BinaryOperator as Op;
    use phrost_hir::body::CompareOp;

    Some(match operator {
        Op::Equal(_) => CompareOp::Equal,
        Op::Identical(_) => CompareOp::Identical,
        Op::NotEqual(_) | Op::AngledNotEqual(_) => CompareOp::NotEqual,
        Op::NotIdentical(_) => CompareOp::NotIdentical,
        Op::LessThan(_) => CompareOp::Less,
        Op::LessThanOrEqual(_) => CompareOp::LessOrEqual,
        Op::GreaterThan(_) => CompareOp::Greater,
        Op::GreaterThanOrEqual(_) => CompareOp::GreaterOrEqual,
        _ => return None,
    })
}

fn arithmetic_operator(operator: &mago_syntax::cst::BinaryOperator<'_>) -> Option<ArithmeticOp> {
    use mago_syntax::cst::BinaryOperator as Op;

    Some(match operator {
        Op::Addition(_) => ArithmeticOp::Add,
        Op::Subtraction(_) => ArithmeticOp::Subtract,
        Op::Multiplication(_) => ArithmeticOp::Multiply,
        Op::Exponentiation(_) => ArithmeticOp::Power,
        Op::Division(_) => ArithmeticOp::Divide,
        _ => return None,
    })
}

/// Whether an operator compares for equality with null, and how.
///
/// `negated` marks the two that assert the value is *not* null; the caller wraps those in a `Not`,
/// which is the shape narrowing already reads backwards.
fn null_comparison(
    operator: &mago_syntax::cst::BinaryOperator<'_>,
) -> Option<(NullTestKind, bool)> {
    use mago_syntax::cst::BinaryOperator as Op;

    match operator {
        Op::Identical(_) => Some((NullTestKind::Identical, false)),
        Op::NotIdentical(_) => Some((NullTestKind::Identical, true)),
        Op::Equal(_) => Some((NullTestKind::Loose, false)),
        Op::NotEqual(_) | Op::AngledNotEqual(_) => Some((NullTestKind::Loose, true)),
        _ => None,
    }
}

/// Whether a strict identity comparison, and whether it is the negated spelling.
///
/// Loose `==`/`!=` are deliberately absent: against `false` they test truthiness, so `0 == false`
/// holds and reading it as an identity would narrow away an `int` that is still possible.
fn false_comparison(operator: &mago_syntax::cst::BinaryOperator<'_>) -> Option<bool> {
    use mago_syntax::cst::BinaryOperator as Op;

    match operator {
        Op::Identical(_) => Some(false),
        Op::NotIdentical(_) => Some(true),
        _ => None,
    }
}

/// A comparison that asks whether a `count()` -- or a `strlen()` -- came out zero, which test that
/// is, and whether it is the negated spelling.
///
/// The five anybody writes, from either side: `count($x) === 0` and `== 0` say empty, `!== 0`,
/// `!= 0` and `> 0` say not. `>= 1` is the same claim as `> 0` and is written too rarely to add a
/// sixth arm for.
///
/// `strlen($s) > 0` is the same shape asked of a string, and a different test: `'0'` is one
/// character long and still empty to `empty()`, so it reads as [`NullTestKind::Length`] and not as
/// the falsy test a count is.
///
/// Nothing else about the number is read. `count($x) === 3` says the array holds three things,
/// which is a fact no type here can carry -- and reading it as "not empty" would be right and
/// reading it as "empty" would be wrong, so it is left alone rather than guessed at.
fn counting_comparison<'p>(
    binary: &mago_syntax::cst::Binary<'p>,
) -> Option<(&'p mago_syntax::cst::Expression<'p>, NullTestKind, bool)> {
    use mago_syntax::cst::BinaryOperator as Op;

    // Which side counts and which side is the number, since generated code writes it either way.
    let ((counted, kind), number, counted_first) =
        match (counted_argument(binary.lhs), counted_argument(binary.rhs)) {
            (Some(counted), None) => (counted, binary.rhs, true),
            (None, Some(counted)) => (counted, binary.lhs, false),
            // `count($a) === count($b)` compares two lengths and says nothing about either being
            // zero.
            _ => return None,
        };
    if !is_zero_literal(number) {
        return None;
    }

    Some((
        counted,
        kind,
        match binary.operator {
            Op::Identical(_) | Op::Equal(_) => false,
            Op::NotIdentical(_) | Op::NotEqual(_) | Op::AngledNotEqual(_) => true,
            // `count($x) > 0` and `0 < count($x)` are the same claim written from the two sides, so
            // which side the count is on decides which operator spells it -- and the other way
            // round says the count is *below* zero, which is not a thing and narrows nothing.
            Op::GreaterThan(_) if counted_first => true,
            Op::LessThan(_) if !counted_first => true,
            _ => return None,
        },
    ))
}

/// The one thing a `count()` or a `strlen()` call measures, when an expression is one, and the
/// test a zero length is for it.
///
/// By the name as written and unqualified only: PHP falls back to the global function for an
/// unqualified call, and a project that declares its own `count()` in a namespace is a project this
/// would be wrong about. `sizeof` is the same function under its old name, and `mb_strlen` counts
/// characters where `strlen` counts bytes -- zero for the same strings.
fn counted_argument<'p>(
    expression: &'p mago_syntax::cst::Expression<'p>,
) -> Option<(&'p mago_syntax::cst::Expression<'p>, NullTestKind)> {
    use mago_syntax::cst::{Argument, Call, Expression};

    let Expression::Call(Call::Function(call)) = expression else {
        return None;
    };
    let Expression::Identifier(identifier) = call.function else {
        return None;
    };
    let written = phrost_syntax::identifier_text(identifier.value());
    let name = written.trim_start_matches('\\');
    let kind = match () {
        _ if name.eq_ignore_ascii_case("count") || name.eq_ignore_ascii_case("sizeof") => {
            NullTestKind::Empty
        }
        _ if name.eq_ignore_ascii_case("strlen") || name.eq_ignore_ascii_case("mb_strlen") => {
            NullTestKind::Length
        }
        _ => return None,
    };
    // One positional argument. `count($x, COUNT_RECURSIVE)` counts something else again, and a
    // spread names nothing this can read.
    let [Argument::Positional(only)] = call.argument_list.arguments.as_slice() else {
        return None;
    };
    only.ellipsis.is_none().then_some((only.value, kind))
}

/// Whether an expression is the integer literal `0`.
fn is_zero_literal(expression: &mago_syntax::cst::Expression<'_>) -> bool {
    let mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::Integer(written)) =
        expression
    else {
        return false;
    };
    written.value == Some(0)
}

/// Whether an expression is the literal `false`, which PHP spells case-insensitively.
/// Which boolean an expression is, when it is one written literally.
///
/// Both, rather than `false` alone: the narrowing either removes is a union operand like any other,
/// and a rule that reads one of two spellings of the same shape is how the other comes to be
/// missing what it has -- which is what happened here for as long as only `false` was read.
fn bool_literal(expression: &mago_syntax::cst::Expression<'_>) -> Option<bool> {
    match expression {
        mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::False(_)) => Some(false),
        mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::True(_)) => Some(true),
        _ => None,
    }
}

/// Whether an expression is the literal `null`, which PHP spells case-insensitively.
fn is_null_literal(expression: &mago_syntax::cst::Expression<'_>) -> bool {
    matches!(
        expression,
        mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::Null(_))
    )
}
