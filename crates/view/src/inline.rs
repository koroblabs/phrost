//! Putting a variable's value back where its name is used.
//!
//! The other direction of [`extraction_at`](IndexView::extraction_at), and the one whose danger is
//! the mirror image: extracting makes an expression run earlier, and inlining makes it run **as
//! many times as the name is read**. `$rows = $repo->all(); f($rows); g($rows);` inlined runs the
//! query twice, which is a different program and a slower one -- and nothing about the file would
//! say so afterwards.
//!
//! So the rule is read *once*, with one exception: a value that is itself a literal or another
//! variable costs nothing to repeat and cannot behave differently for having been. Everything else
//! -- a call, an access, an operator, a construction -- is inlined only into a single use.
//!
//! # What else it refuses
//!
//! - **A name written to more than once.** Which of the two values a use meant is a question the
//!   variable was answering, and inlining throws the answer away. A compound assignment is one of
//!   those writes: `$offset += $batch_size` was invisible here while it lowered to an unmodelled
//!   node, and inlining rewrote it to `0 += $batch_size` -- a file PHP will not parse.
//! - **A name bound by anything but an assignment.** A parameter, a `foreach` target and a `catch`
//!   clause have no value to put anywhere; a `/** @var */` is an annotation on something else.
//! - **An assignment sharing its line.** The line is what gets removed, and taking a neighbour with
//!   it is not what anybody asked for.
//! - **A name read inside a string.** `"path - message: $violationMessage"` interpolates the name,
//!   and what goes in its place there is a variable, a property or an element and nothing else --
//!   inlining an `array_reduce(...)` into it wrote a file PHP will not parse, on the reference
//!   project. Rewriting the string as a concatenation is a different edit from this one.
//! - **A name standing where PHP reads a name.** `$this->$option_tag`, `$$name`, `new $class` and
//!   `$x instanceof $class` take the variable's *value as a name*, and what may replace it there is
//!   another variable -- `$this->'option_tag_'.$tag_name` is a parse error, on WordPress. Braces
//!   or parentheses would make each of them legal again, four different ways; the offer is withheld
//!   instead.

use phrost_body::OccurrenceKind;
use phrost_hir::body::ExprData;
use phrost_model::{TextRange, TextSize};

use crate::{BindingKind, view::IndexView};

/// A variable whose value could go back where its name is, and every place that is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Inlining {
    /// Without the leading `$`, for naming the offer.
    pub name: String,
    /// The whole assignment line, newline included, which is what is removed.
    pub statement: TextRange,
    /// The value's own range, which is what each use is replaced by.
    pub value: TextRange,
    /// Where the name is read, in source order.
    pub reads: Vec<TextRange>,
    /// Whether the value has to be written in parentheses where the name was.
    ///
    /// A name binds tighter than every operator and the expression behind it may not: `$sum = 1 +
    /// 2; return $sum * 3;` inlined bare is `1 + 2 * 3`, which is 7 where the program said 9 --
    /// and nothing about the file says so afterwards, because it still parses. Four shapes were
    /// measured wrong this way and only two of them were parse errors, which is the reason this is
    /// decided here rather than left to whoever writes the edit.
    pub needs_parentheses: bool,
}

impl IndexView<'_> {
    /// The variable at `offset`, when its value could be put back where it is used.
    pub fn inlining_at(&self, offset: TextSize) -> Option<Inlining> {
        let open = self.current()?;
        let hir = open.hir();
        let text = open.text();

        let variable = self.variable_at(offset)?;
        let (bound_at, BindingKind::Assignment) = variable.declared_at? else {
            return None;
        };

        let occurrences = self.local_occurrences(offset, variable.name.as_str());
        let mut reads = Vec::new();
        let mut writes = 0;
        for occurrence in &occurrences {
            match occurrence.kind {
                OccurrenceKind::Read => reads.push(occurrence.range),
                // A compound assignment counts as a write and is not a read this may replace:
                // `$offset += $batch_size` puts a value in `$offset`, and putting the initialiser
                // there instead produced `0 += $batch_size` on a real project.
                OccurrenceKind::Write | OccurrenceKind::ReadWrite => writes += 1,
            }
        }
        // A second write means the variable was answering which value a use meant.
        if writes != 1 || reads.is_empty() {
            return None;
        }
        // A read spelled without its `$` is `compact('name')` taking the variable by name: there is
        // no value to put there, and removing the assignment would leave the call collecting nothing.
        if reads
            .iter()
            .any(|range| !text[usize::from(range.start())..].starts_with('$'))
        {
            return None;
        }

        // A read a string's own text runs right up to is interpolated: outside a string a literal
        // never touches a `$`, since an operator or a separator always stands between them.
        if reads.iter().any(|read| {
            hir.string_literals().any(|string| {
                string.end() == read.start()
                    || (string.end() + TextSize::from(1) == read.start()
                        && text[usize::from(string.end())..].starts_with('{'))
            })
        }) {
            return None;
        }

        if reads.iter().any(|read| {
            let before = text[..usize::from(read.start())].trim_end();
            before.ends_with("->")
                || before.ends_with("::")
                || before.ends_with('$')
                || ["new", "instanceof"].iter().any(|keyword| {
                    before.strip_suffix(keyword).is_some_and(|rest| {
                        !rest.ends_with(|ch: char| ch.is_alphanumeric() || ch == '_')
                    })
                })
        }) {
            return None;
        }

        let site = phrost_body::assignment_sites(hir)
            .into_iter()
            .find(|site| site.target_range == bound_at)?;
        // A value repeated has to be one that costs nothing to repeat.
        if reads.len() > 1 && !is_free_to_repeat(hir.expr(site.value_expr)) {
            return None;
        }

        Some(Inlining {
            name: variable.name.as_str().to_owned(),
            statement: whole_line(text, site.target_range, site.value_range)?,
            value: site.value_range,
            reads,
            needs_parentheses: !binds_tightest(hir.expr(site.value_expr)),
        })
    }
}

/// Whether a value may be written where a name was without parentheses around it.
///
/// A name is a primary expression: nothing binds tighter, so the characters around it keep meaning
/// what they meant. The shapes here are the ones that are primaries too -- a literal, a variable,
/// an array, and everything reached by a postfix operator, which is every call and every access.
/// Everything else is an operator expression, and four of them were measured producing a different
/// program: `1 + 2` inlined into `$sum * 3`, a ternary inlined into a sum, a closure inlined into
/// the callee position of its own call, and a `new` inlined before a `->`.
fn binds_tightest(data: &ExprData) -> bool {
    matches!(
        data,
        ExprData::Literal { .. }
            | ExprData::Variable { .. }
            | ExprData::ArrayLiteral { .. }
            | ExprData::Index { .. }
            | ExprData::PropertyAccess { .. }
            | ExprData::StaticPropertyAccess { .. }
            | ExprData::ClassConstAccess { .. }
            | ExprData::MethodCall { .. }
            | ExprData::StaticCall { .. }
            | ExprData::FunctionCall { .. }
            | ExprData::IndirectCall { .. }
            | ExprData::CallableMember { .. }
            | ExprData::CallableFunction { .. }
    )
}

/// Whether a value can be written in several places without the program doing more.
///
/// A literal and a variable, and nothing else. A property access looks free and is not: PHP runs a
/// `__get` for one, and a hooked property runs its own code.
fn is_free_to_repeat(data: &ExprData) -> bool {
    matches!(data, ExprData::Literal { .. } | ExprData::Variable { .. })
}

/// The line an assignment occupies, when nothing else shares it.
fn whole_line(text: &str, target: TextRange, value: TextRange) -> Option<TextRange> {
    let from = text[..usize::from(target.start())]
        .rfind('\n')
        .map_or(0, |at| at + 1);
    if !text[from..usize::from(target.start())]
        .chars()
        .all(char::is_whitespace)
    {
        return None;
    }
    let after = usize::from(value.end());
    let end = text[after..]
        .find('\n')
        .map_or(text.len(), |at| after + at + 1);
    // Only the `;` that ends it may lie between the value and the line's end.
    if !text[after..end].trim_end().trim_end_matches(';').is_empty() {
        return None;
    }
    Some(TextRange::new(
        TextSize::try_from(from).ok()?,
        TextSize::try_from(end).ok()?,
    ))
}
