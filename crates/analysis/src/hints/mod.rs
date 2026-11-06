//! Text an editor draws that the file does not contain.
//!
//! Parameter names at call sites, which is what makes `->send($body, true, 3)` readable without
//! going to look at the declaration, and the type a variable was given, which is the one fact PHP
//! never writes down.
//!
//! # Nothing a reader can already see
//!
//! A hint that repeats what is written is worse than none: it costs the same screen space and
//! teaches the reader to stop looking. So `f($id)` gets no `id:` over it, and neither does a named
//! argument, a spread, or anything past them -- see
//! [`argument_bindings`](phrost_view::IndexView::argument_bindings) for where the correspondence
//! ends.

use phrost_model::{TextRange, TextSize};
use phrost_view::IndexView;

use crate::type_width::{self, WIDEST_LINE};

/// One piece of text to draw at a position.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InlayHint {
    /// Where it goes, between two characters of the file.
    pub offset: TextSize,
    /// What to draw, punctuation included, so the protocol layer does no formatting.
    pub label: String,
    /// The whole of what the label had to leave out, for a client that can show it on demand.
    /// `None` when the label already says everything.
    pub tooltip: Option<String>,
    pub kind: InlayKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InlayKind {
    Parameter,
    Type,
}

/// Hints for the part of the current file that `range` covers.
///
/// Ranged because a client asks per viewport: a thousand-line file is a hundred lines of hints
/// nobody will scroll to.
pub fn inlay_hints(view: &IndexView<'_>, range: TextRange) -> Vec<InlayHint> {
    let Some(text) = view.current_text() else {
        return Vec::new();
    };

    let mut hints = view
        .argument_bindings()
        .into_iter()
        .filter(|binding| range.contains_range(binding.range))
        .filter(|binding| !repeats_the_argument(text, binding))
        .map(|binding| InlayHint {
            offset: binding.range.start(),
            label: format!("{}:", binding.parameter),
            tooltip: None,
            kind: InlayKind::Parameter,
        })
        .collect::<Vec<_>>();

    hints.extend(variable_types(view, text, range));
    hints.sort_by_key(|hint| hint.offset);
    hints
}

/// The widest type a hint spells out in full.
///
/// Not a measurement of any screen: it is how much of a line a reader will give up to be told
/// something they could also get by hovering. Past it the shapes fold to `array{…}`, which is the
/// one type that grows without a bound -- twelve keys and 250 characters out of one real project,
/// which is where this number came from.
const WIDEST_HINT: usize = 48;

/// The type a variable was given, drawn where it was given one.
///
/// At the binding and not at the uses, which is the difference between a hint and a highlight: a
/// reader wants to know what a name means once, at the point it starts meaning it.
///
/// The filter is what decides whether this is useful or unbearable. Most assignments in real code
/// say their own type -- `new Repo()`, `User::load(1)`, `5`, `[]` -- and a hint over one of those
/// costs a reader the same screen space to learn nothing, which teaches them to stop reading hints
/// at all. So the only ones drawn are the assignments where the type appears nowhere on the line:
/// `$repo = $factory->make()`, which is exactly where a reader would otherwise have to go and look.
fn variable_types(
    view: &IndexView<'_>,
    text: &str,
    range: TextRange,
) -> impl Iterator<Item = InlayHint> {
    view.assigned_types()
        .into_iter()
        .filter(move |assigned| range.contains_range(assigned.range))
        .filter_map(move |assigned| {
            let ty = assigned.ty?;
            let value = &text[usize::from(assigned.value_range.start())
                ..usize::from(assigned.value_range.end())];
            if states_its_own_type(value, &ty) {
                return None;
            }
            // Folded rather than cut, and only where the whole will not fit: a hint is drawn
            // *into* the line, so its width is taken from the code the reader came for.
            //
            // Two folds in order, because two kinds of type grow without a bound and a value can be
            // both: a shape's fields go first, and if what is left is still too wide, the union's
            // trailing operands follow. A `match` over eight product types is the second kind --
            // eight classes, 135 characters of them.
            let folded = (ty.chars().count() > WIDEST_HINT)
                .then(|| {
                    let shapes = type_width::folded_shapes(&ty);
                    match shapes.chars().count() > WIDEST_HINT {
                        true => type_width::folded_operands(&shapes, WIDEST_HINT),
                        false => shapes,
                    }
                })
                .filter(|folded| folded != &ty);
            Some(InlayHint {
                // After the name, where a declaration would be written in a language that had one.
                offset: assigned.range.end(),
                label: format!(": {}", folded.as_deref().unwrap_or(&ty)),
                // What the fold left out, for the client to show when the reader asks -- which is
                // what makes eliding a shape cost a gesture rather than the fact.
                tooltip: folded
                    .is_some()
                    .then(|| type_width::over_lines(&ty, WIDEST_LINE)),
                kind: InlayKind::Type,
            })
        })
}

/// Whether the value as written already tells the reader the type.
///
/// A text test, like [`repeats_the_argument`], and for the same reason: the question is what is on
/// the screen, not what the IR knows. `new Repo()` and `Repo::make()` both put the name there, and
/// nobody needs to be told that `5` is an int.
fn states_its_own_type(value: &str, ty: &str) -> bool {
    let value = value.trim();
    // Case-sensitively, so `$repository->find()` does not read as stating `Repo`.
    if value.contains(ty.trim_start_matches('?')) {
        return true;
    }

    let mut characters = value.chars();
    match characters.next() {
        // A string, an array, or a number, negative or otherwise.
        Some('\'' | '"' | '[') => true,
        Some(first) if first.is_ascii_digit() => true,
        Some('-') => characters.next().is_some_and(|next| next.is_ascii_digit()),
        _ => {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "false" | "null"
            ) || value.starts_with("array(")
        }
    }
}

/// Whether the argument already says the parameter's name.
///
/// `save($user)` filled from `save(User $user)` is the commonest call there is, and `user:` over it
/// is noise. Compared case-sensitively, because PHP variable names are.
fn repeats_the_argument(text: &str, binding: &phrost_view::ArgumentBinding) -> bool {
    let written = &text[usize::from(binding.range.start())..usize::from(binding.range.end())];
    written
        .strip_prefix('$')
        .is_some_and(|name| name == binding.parameter.as_str())
}
