//! `no-error-control-operator` and the one `@` that is a protocol rather than a habit.
//!
//! The rule reports every `@`, at error level, in a category shown unasked, and advises: "Remove
//! the `@` and use `set_error_handler` to handle errors instead." For `@unlink($file)` that is a
//! position somebody may hold. For
//!
//! ```php
//! @trigger_error('Foo::bar() is deprecated in drupal:11.5.0 ...', E_USER_DEPRECATED);
//! ```
//!
//! it is advice to break the thing the line is for. The notice is one the author raises on
//! purpose, and the `@` is what makes it opt-in: silenced, it reaches nobody's page or log and
//! still reaches every error handler, which is how a test runner or a debug toolbar collects
//! deprecations. Unsilenced, every site running the deprecated path gets a notice per call. So the
//! idiom is already the `set_error_handler` arrangement the help asks for, and it is the spelling
//! Symfony's conventions and Drupal's deprecation policy both require -- a Drupal module that
//! deprecates one of its own methods is told to write this exact line, and was then shown an
//! error on it.
//!
//! Measured: 368 of the rule's 483 findings in Drupal core are this line, 35 of 724 in Symfony,
//! 2 of 411 in WordPress and none of Laravel's 55, which does not deprecate this way. No other
//! finding of any rule moved.
//!
//! Narrow on purpose. The function is `trigger_error` and the level is written as the constant
//! `E_USER_DEPRECATED`; a notice at any other level, or at a level held in a variable, is an error
//! being hidden as far as the page can tell, and stays reported.

use mago_span::Span;
use mago_syntax::cst::{Argument, Call, Expression, Node, Program, UnaryPrefixOperator};

use super::descent::Descent;

/// A deprecation raised silenced, the way a library is asked to raise one.
pub(super) struct DeprecationNotice;

impl DeprecationNotice {
    /// Whether the `@` at `operator` stands in front of `trigger_error(…, E_USER_DEPRECATED)`.
    ///
    /// `operator` is the finding's primary annotation, which the rule puts on the `@` itself. An
    /// operator this cannot find again is not one it has judged, so the answer is no and the
    /// finding stands.
    pub(super) fn is_silenced_at(program: &Program<'_>, operator: Span) -> bool {
        Descent::to(program, operator, |node| match node {
            Node::UnaryPrefix(unary)
                if matches!(unary.operator, UnaryPrefixOperator::ErrorControl(at) if at == operator) =>
            {
                Some(Self::is_raised_by(unary.operand))
            }
            _ => None,
        })
        .unwrap_or(false)
    }

    fn is_raised_by(silenced: &Expression<'_>) -> bool {
        let Expression::Call(Call::Function(call)) = Self::unwrapped(silenced) else {
            return false;
        };
        // By what is written rather than what it resolves to: a namespace that declares its own
        // `trigger_error` is not a shape any project measured has, and the fully qualified
        // spelling is the common one in a codebase that imports its functions.
        let Expression::Identifier(function) = call.function else {
            return false;
        };
        if !Self::global(function.value()).eq_ignore_ascii_case(b"trigger_error") {
            return false;
        }

        call.argument_list
            .arguments
            .iter()
            .enumerate()
            .find_map(|(position, argument)| match argument {
                Argument::Positional(argument) if position == 1 && argument.ellipsis.is_none() => {
                    Some(argument.value)
                }
                Argument::Named(argument) if argument.name.value == b"error_level" => {
                    Some(argument.value)
                }
                _ => None,
            })
            .is_some_and(|level| match Self::unwrapped(level) {
                // Case-sensitive, as a constant is.
                Expression::ConstantAccess(constant) => {
                    Self::global(constant.name.value()) == b"E_USER_DEPRECATED"
                }
                _ => false,
            })
    }

    /// The expression inside any parentheses around it.
    fn unwrapped<'tree, 'arena>(
        expression: &'tree Expression<'arena>,
    ) -> &'tree Expression<'arena> {
        match expression {
            Expression::Parenthesized(parenthesized) => Self::unwrapped(parenthesized.expression),
            _ => expression,
        }
    }

    /// A name without the leading `\` that says it is global.
    fn global(name: &[u8]) -> &[u8] {
        name.strip_prefix(b"\\").unwrap_or(name)
    }
}
