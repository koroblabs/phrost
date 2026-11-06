//! Giving a read that may find nothing something to find.
//!
//! The sibling of [`nullsafe`](super::nullsafe) and the same argument: `$row['vin'] ?? null` is what
//! a person writes when `$row` may not be there, and it is a fix rather than a silencer. The null
//! moves to whatever consumes the expression, and if that consumer cannot take one the next check
//! says so. Nothing is suppressed.
//!
//! **Which default depends on which sentinel.** `??` tests for null only, so it answers the null
//! half and does nothing about a `false` -- `preg_split()` returning `false` survives it untouched.
//! `?:` tests falsy, which covers the `false` and takes an empty array with it, and that is
//! harmless where the value is about to be iterated. So the null cases get `??` and the false cases
//! get `?:`, and neither is offered where the other's sentinel is the one that arrives.

use phrost_view::IndexView;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};
use crate::diagnostics::{
    foreach::{POSSIBLY_FALSE_ITERATION, POSSIBLY_NULL_ITERATION},
    subscripts::POSSIBLY_NULL_SUBSCRIPT,
};

pub(super) struct DefaultTheRead;

impl Offer for DefaultTheRead {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    /// Every offer here works from a position rather than from a list the subject layer holds, so
    /// there is nothing for it to gate on.
    fn subject(&self) -> ActionSubject {
        ActionSubject::Expression
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        let at = sites.at();
        Self::subscripts(view, at, out);
        Self::loops(view, at, out);
    }
}

impl DefaultTheRead {
    /// `$row['vin']` where `$row` may be null.
    ///
    /// Parenthesised, because `??` binds looser than nearly everything: `$row['vin'] ?? null . '!'`
    /// is `$row['vin'] ?? (null . '!')`, so an unparenthesised edit would change what a
    /// concatenation or a comparison means. The brackets cost a reader nothing and are the only
    /// form that is right at every position.
    fn subscripts(view: &IndexView<'_>, at: phrost_model::TextRange, out: &mut Vec<CodeAction>) {
        for subscript in view.null_subscripts() {
            // Certain null has nothing to default *to* a value: the read is null on every run, and
            // `?? null` would say so twice. The same exclusion `nullsafe` makes.
            if subscript.certain || !subscript.whole.contains_range(at) {
                continue;
            }
            let Some(text) = view.text_in(subscript.whole) else {
                continue;
            };
            out.push(CodeAction {
                title: match &subscript.key {
                    Some(key) => format!("Read '{key}' as null where there is nothing to read"),
                    None => "Read the element as null where there is nothing to read".to_owned(),
                },
                edits: vec![TextEdit::replace(
                    subscript.whole,
                    format!("({text} ?? null)"),
                )],
                preferred: true,
                kind: ActionKind::QuickFix,
                fixes: Some(POSSIBLY_NULL_SUBSCRIPT),
            });
        }
    }

    /// `foreach ($rows as $row)` where `$rows` may be null or false.
    ///
    /// Unparenthesised, and that is not an inconsistency with the rule above: `as` ends the
    /// expression, so there is no operator left for the precedence to be wrong about.
    fn loops(view: &IndexView<'_>, at: phrost_model::TextRange, out: &mut Vec<CodeAction>) {
        for loop_ in view.not_iterable_loops() {
            if !loop_.range.contains_range(at) {
                continue;
            }
            let Some(text) = view.text_in(loop_.range) else {
                continue;
            };
            // `??` answers a null and leaves a `false` standing; `?:` answers both, and taking an
            // empty array with it costs nothing where the value is about to be walked.
            let (operator, code) = match loop_.why {
                phrost_view::NotIterable::Null { certain: false } => {
                    ("??", POSSIBLY_NULL_ITERATION)
                }
                phrost_view::NotIterable::False { certain: false } => {
                    ("?:", POSSIBLY_FALSE_ITERATION)
                }
                // A certain sentinel iterates nothing on every run, and a default would hide a loop
                // whose body never ran rather than fix one.
                _ => continue,
            };
            out.push(CodeAction {
                title: "Walk nothing where there is nothing to walk".to_owned(),
                edits: vec![TextEdit::replace(
                    loop_.range,
                    format!("{text} {operator} []"),
                )],
                preferred: true,
                kind: ActionKind::QuickFix,
                fixes: Some(code),
            });
        }
    }
}
