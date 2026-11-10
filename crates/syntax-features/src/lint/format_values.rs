//! How many values a `printf`-family call is written with, which `no-missing-format-argument`
//! gets wrong twice.
//!
//! The rule counts the placeholders in a literal format string against the arguments after it, and
//! both of its defects are in the second half of that:
//!
//! - **An unpacked argument is counted as one value.** `printf('%s %s %d', 'total', ...$sums)` is
//!   two arguments after the format and any number of values, so the rule reports a call that runs.
//!   mago checks the *format* argument for `...` and none of the ones it is counting.
//! - **The sentence prints the number it compared against.** "contains 8 placeholders, 8 values
//!   given" on a call given two: the last number is `required_args`, where the count of values was
//!   meant. Every finding of the rule says it, the genuine ones included, so a reader is told the
//!   two numbers agree in the one message whose point is that they do not.
//!
//! Both are in mago 1.58.2 and still in its `main` on 2026-11-10, and neither is a position: the
//! first is a call PHP runs without complaint, the second a sentence that contradicts itself. So
//! they are put right here, on the tree the rule itself read, rather than by switching the rule
//! off -- what is left is a true `ArgumentCountError` found before the line runs.
//!
//! Measured on Symfony's 13 118 files: the rule fires six times and all six are the unpacked
//! shape (`sprintf('%s is deprecated, use %s_new instead.', ...$args)`), so there it was wrong
//! every time it spoke. WordPress's 2 086 files have none of either kind.

use mago_reporting::{AnnotationKind, Issue};
use mago_span::{HasSpan, Span};
use mago_syntax::cst::{Argument, Node, Program};

use super::descent::Descent;

/// What stands after the format string in one reported call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FormatValues {
    /// This many values, each written out.
    Written(usize),
    /// At least one is unpacked, so how many arrive is decided when the line runs and the page
    /// cannot say that any are missing.
    Unpacked,
}

impl FormatValues {
    /// The values of the call a finding is about, or nothing when this cannot tell which call
    /// that is -- and then the finding stands exactly as mago wrote it, because a filter that
    /// cannot find what it is judging must not act as though it judged it.
    ///
    /// `arguments` is the finding's primary annotation, which the rule puts on the whole argument
    /// list. The format string is its secondary one, found by kind rather than by position; the
    /// rule's other report -- a `v*printf` with no array at all -- has no secondary annotation and
    /// is not about a count of values, so it is passed over here by having none.
    pub(super) fn of(program: &Program<'_>, issue: &Issue, arguments: Span) -> Option<Self> {
        let format = issue
            .annotations
            .iter()
            .find(|annotation| annotation.kind == AnnotationKind::Secondary)?
            .span;
        let call = Descent::to(program, arguments, |node| match node {
            Node::FunctionCall(call) if call.argument_list.span() == arguments => Some(call),
            _ => None,
        })?;

        // After the format and not after the first argument: `fprintf` takes a handle before it.
        let mut written = 0;
        for argument in call.argument_list.arguments.iter() {
            if argument.span().start.offset < format.end.offset {
                continue;
            }
            if matches!(argument, Argument::Positional(positional) if positional.ellipsis.is_some())
            {
                return Some(Self::Unpacked);
            }
            written += 1;
        }
        Some(Self::Written(written))
    }

    /// mago's sentence with `given` where it printed the number of values the format needs.
    ///
    /// Only the clause after the last comma, and only when it is the clause this knows -- a count,
    /// `value` or `values`, `given.` -- so wording that changes upstream comes through untouched
    /// and shows up in this rule's snapshots, rather than being rewritten into something neither
    /// side wrote. A fix upstream makes this a rewrite of a sentence into itself.
    pub(super) fn stated(message: String, given: usize) -> String {
        let Some((said, clause)) = message.rsplit_once(", ") else {
            return message;
        };
        let counts_values = clause
            .strip_suffix(" given.")
            .and_then(|counted| counted.split_once(' '))
            .is_some_and(|(count, noun)| {
                !count.is_empty()
                    && count.bytes().all(|byte| byte.is_ascii_digit())
                    && matches!(noun, "value" | "values")
            });
        if !counts_values {
            return message;
        }
        format!(
            "{said}, {given} value{} given.",
            if given == 1 { "" } else { "s" }
        )
    }
}
