//! A whole-file reprint, cut into the line runs that actually moved.
//!
//! `format` answers with one replacement spanning everything between the first and last byte that
//! changed, which is what a client wants for a whole-document format and is exactly wrong for a
//! selection: a file with a change above the selection and another below it has one span covering
//! both, and returning it would rewrite lines the caller did not ask about. The protocol is clear
//! that a range format may only edit inside the range.
//!
//! So the same reprint is cut into hunks first. Line-aligned, because a formatter's output is read
//! by a person and a hunk that starts mid-line shows up in the editor's undo as a change to a line
//! nobody touched.

use std::collections::HashMap;

use phrost_model::{TextRange, TextSize};

/// The line runs that turn `before` into `after`, as replacements into `before`.
pub(crate) fn between(before: &str, after: &str) -> Vec<(TextRange, String)> {
    let old = lines(before);
    let new = lines(after);
    let mut offsets = Vec::with_capacity(old.len() + 1);
    let mut at = 0;
    for line in &old {
        offsets.push(at);
        at += line.len();
    }
    offsets.push(at);

    let mut hunks = Vec::new();
    let mut cut = |mut from: (usize, usize), mut to: (usize, usize)| {
        // The edges of a segment, trimmed. Anchors are the lines that occur *once*, and a closing
        // brace or a blank line never does -- so a segment bounded by two anchors carries whatever
        // repeated lines sit between them and the change. Leaving them in makes a hunk that starts
        // four lines above what moved, which a selection then refuses for running past its start.
        while from.0 < to.0 && from.1 < to.1 && old[from.0] == new[from.1] {
            from = (from.0 + 1, from.1 + 1);
        }
        while to.0 > from.0 && to.1 > from.1 && old[to.0 - 1] == new[to.1 - 1] {
            to = (to.0 - 1, to.1 - 1);
        }
        if from.0 == to.0 && from.1 == to.1 {
            return;
        }
        hunks.push((
            TextRange::new(size(offsets[from.0]), size(offsets[to.0])),
            new[from.1..to.1].concat(),
        ));
    };

    let mut previous = (0, 0);
    for anchor in anchors(&old, &new) {
        cut(previous, anchor);
        previous = (anchor.0 + 1, anchor.1 + 1);
    }
    cut(previous, (old.len(), new.len()));
    hunks
}

/// The lines the two texts certainly agree on, in order.
///
/// A line that occurs exactly once on each side and is the same line is the same line -- that is
/// the patience-diff observation, and it is what a shortest-edit search cannot give. Asked for
/// this because a formatter's output is full of lines that repeat: a shortest edit script matched
/// `}` and a blank line wherever it liked, and rebuilt the file correctly while attributing the
/// class declaration to the position of the second method. That is invisible to a whole-document
/// format, which replaces the span between the first and last change either way, and is the whole
/// answer for a selection, which shows the caller the hunks that fall inside it.
///
/// Kept in increasing order on both sides by dropping any anchor that would go backwards, which is
/// a greedy stand-in for the longest increasing subsequence patience diff takes here. It costs a
/// hunk that could have been two; it cannot misattribute one, because what is between two anchors
/// is bounded by lines both sides agree on.
fn anchors(old: &[&str], new: &[&str]) -> Vec<(usize, usize)> {
    let (left, right) = (once(old), once(new));

    let mut found = Vec::new();
    let mut last = (0, 0);
    for (line, at) in left.iter().filter_map(|(line, at)| Some((*line, (*at)?))) {
        let Some(Some(there)) = right.get(line) else {
            continue;
        };
        found.push((at, *there));
    }
    found.sort_unstable();
    found.retain(|&(here, there)| {
        let keep = here >= last.0 && there >= last.1;
        if keep {
            last = (here + 1, there + 1);
        }
        keep
    });
    found
}

/// Where each line that occurs exactly once sits, and `None` for one that repeats.
fn once<'a>(lines: &[&'a str]) -> HashMap<&'a str, Option<usize>> {
    let mut seen = HashMap::new();
    for (at, line) in lines.iter().enumerate() {
        seen.entry(*line)
            .and_modify(|found| *found = None)
            .or_insert(Some(at));
    }
    seen
}

fn size(offset: usize) -> TextSize {
    TextSize::try_from(offset).unwrap_or_default()
}

/// `text` split so that concatenating the pieces gives `text` back.
///
/// Keeping the newline on its line is what makes an offset derivable by adding lengths, and what
/// makes a hunk that adds or removes a line come out as whole lines rather than as a newline
/// moved from one to the next.
fn lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let end = rest.find('\n').map_or(rest.len(), |at| at + 1);
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(before: &str, hunks: &[(TextRange, String)]) -> String {
        let mut out = String::new();
        let mut at = 0;
        for (range, text) in hunks {
            out.push_str(&before[at..usize::from(range.start())]);
            out.push_str(text);
            at = usize::from(range.end());
        }
        out.push_str(&before[at..]);
        out
    }

    #[track_caller]
    fn round_trip(before: &str, after: &str) -> Vec<(TextRange, String)> {
        let hunks = between(before, after);
        assert_eq!(
            applied(before, &hunks),
            after,
            "the hunks do not rebuild it"
        );
        hunks
    }

    #[test]
    fn a_file_that_did_not_move_has_no_hunks() {
        assert!(round_trip("a\nb\nc\n", "a\nb\nc\n").is_empty());
    }

    #[test]
    fn two_changes_with_untouched_lines_between_them_are_two_hunks() {
        // The whole reason this exists: one span covering both would rewrite the middle, and a
        // range format may only edit inside the range it was given.
        let hunks = round_trip("a\nb\nc\nd\ne\n", "A\nb\nc\nd\nE\n");
        assert_eq!(hunks.len(), 2);
        assert_eq!(usize::from(hunks[0].0.start()), 0);
        assert_eq!(usize::from(hunks[1].0.start()), 8);
    }

    #[test]
    fn an_added_line_is_an_insertion_and_not_a_rewrite_of_its_neighbour() {
        let hunks = round_trip("a\nc\n", "a\nb\nc\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].1, "b\n");
    }

    #[test]
    fn a_removed_line_leaves_the_lines_around_it_alone() {
        let hunks = round_trip("a\nb\nc\n", "a\nc\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].1, "");
    }

    #[test]
    fn a_file_with_no_trailing_newline_still_rebuilds() {
        round_trip("a\nb", "a\nB");
    }

    #[test]
    fn a_wholly_rewritten_file_is_one_hunk() {
        // Nothing is unique on both sides, so there is no anchor and no honest way to say which
        // line became which -- one hunk is the true answer rather than a giving-up.
        let before: String = (0..200).map(|n| format!("{n}\n")).collect();
        let after: String = (0..200).map(|n| format!("x{n}\n")).collect();
        assert_eq!(round_trip(&before, &after).len(), 1);
    }

    #[test]
    fn a_repeated_line_does_not_anchor_a_change_to_the_wrong_place() {
        // What a shortest-edit search got wrong: `}` and the blank line between methods occur
        // several times, and matching them wherever it is cheapest rebuilds the file while saying
        // the first method's text belongs where the second one is.
        let before = "<?php\nclass A {\n    function one(){}\n\n    function two(){}\n}\n";
        let after = "<?php\nclass A\n{\n    function one() {}\n\n    function two() {}\n}\n";
        for (range, text) in round_trip(before, after) {
            let at = &before[usize::from(range.start())..usize::from(range.end())];
            assert_eq!(
                at.contains("one"),
                text.contains("one"),
                "a hunk over {at:?} was given {text:?}"
            );
        }
    }
}
