//! The parentheses a formatted file has to keep, and the formatter does not.
//!
//! An arithmetic chain's: mago lays one out flat and drops the parentheses that grouped it, which
//! changes what a line computes -- see [`grouping_kept`]. The right-hand side of `instanceof` was
//! the other case until mago 1.60, which keeps every parenthesis PHP needs there; putting them back
//! from here was deleted with that release. Were it to drop them again, the reprint would not parse
//! by PHP's rules, and the guard in `format` refuses a reprint like that rather than handing it back.

use mago_syntax::cst::{Binary, Expression, Node, Program};
use mago_syntax::token::{GetPrecedence, Precedence};
use phrost_syntax::ParseSession;

/// Whether `printed` keeps every parenthesis of `written` that decides how an arithmetic chain
/// groups.
///
/// mago drops the ones inside a chain it lays out flat: `$a + ($b - $c) + $d` comes back
/// `$a + $b - $c + $d`. For integers that is the same number until one of them overflows into a
/// float; for floats it is a different number -- `0.1 + (0.2 - 0.3)` and `0.1 + 0.2 - 0.3` differ
/// in the last bits -- and a formatter that changes what a line computes has to be refused, however
/// rarely: one file of 60 000 across five corpora. Nothing here can put them back, since nothing in
/// the output says where they were, so the file is left as it was written.
///
/// Counted rather than located, because a formatter cannot *add* such a parenthesis without changing
/// the code either, so fewer of them in the output is exactly a regrouping. Concatenation and the
/// logical operators are left out: they group the same either way, and refusing a file over them
/// would cost a save for nothing.
pub(super) fn grouping_kept(written: &str, printed: &Program<'_>) -> bool {
    if !operand_opens_after_an_operator(written) {
        return true;
    }
    let session = ParseSession::new();
    let parsed = session.parse(written);
    grouping(printed) >= grouping(parsed.program())
}

/// Whether an arithmetic operator is followed by `(` anywhere in `text` -- the only place a
/// grouping parenthesis can be, and absent from most files.
fn operand_opens_after_an_operator(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().any(|(at, byte)| {
        *byte == b'('
            && bytes[..at]
                .iter()
                .rev()
                .find(|before| !before.is_ascii_whitespace())
                .is_some_and(|before| {
                    matches!(before, b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>')
                })
    })
}

/// How many operands in `program` are parenthesised against PHP's own grouping: the right operand
/// of a left-associative arithmetic operator, or the left of `**`, that is itself an operator of
/// the same precedence.
fn grouping(program: &Program<'_>) -> usize {
    let mut count = 0;
    let mut pending = vec![Node::Program(program)];
    while let Some(node) = pending.pop() {
        if let Node::Binary(binary) = node {
            count += usize::from(regrouped_operand(binary));
        }
        node.visit_children(|child| pending.push(child));
    }
    count
}

fn regrouped_operand(binary: &Binary<'_>) -> bool {
    let level = binary.operator.precedence();
    let operand = match level {
        Precedence::AddSub | Precedence::MulDivMod | Precedence::BitShift => binary.rhs,
        Precedence::Pow => binary.lhs,
        _ => return false,
    };
    let Expression::Parenthesized(parenthesized) = operand else {
        return false;
    };
    matches!(parenthesized.expression, Expression::Binary(inner) if inner.operator.precedence() == level)
}
