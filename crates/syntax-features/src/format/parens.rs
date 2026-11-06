//! The parentheses PHP needs that the formatter takes away.
//!
//! PHP reads the right-hand side of `instanceof` as a class reference and anything else only
//! between parentheses; mago keeps them for a class constant and nowhere else, so
//! `$job instanceof ((string) $jobToFake)` in Laravel's `QueueFake` came back as
//! `$job instanceof (string) $jobToFake`, which PHP refuses outright.
//!
//! Put back rather than refused: the formatter is right about everything else in the file, and a
//! file that formats except for one expression is better served by the expression keeping what it
//! was written with than by being left unformatted. Which operands need them is the parser's rule
//! -- see `phrost_syntax`'s `refused` -- so the two cannot disagree.

use mago_syntax::cst::{Binary, Expression, Node, Program};
use mago_syntax::token::{GetPrecedence, Precedence};
use phrost_syntax::ParseSession;

/// `text` with the parentheses back around every `instanceof` right-hand side that needs them, or
/// `None` when there is none to put back.
pub(super) fn restored(text: &str) -> Option<String> {
    if !text.contains("instanceof") {
        return None;
    }
    let session = ParseSession::new();
    let operands = session.parse(text).bare_instanceof_operands();
    if operands.is_empty() {
        return None;
    }

    // Every insertion from the end of the text back, so an offset still to be used is never moved
    // by one already made -- nested ranges included.
    let mut insertions: Vec<(usize, char)> = operands
        .into_iter()
        .flat_map(|range| {
            [
                (usize::from(range.start()), '('),
                (usize::from(range.end()), ')'),
            ]
        })
        .collect();
    insertions.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let mut out = text.to_owned();
    for (at, parenthesis) in insertions {
        if !out.is_char_boundary(at) {
            return None;
        }
        out.insert(at, parenthesis);
    }
    Some(out)
}

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
