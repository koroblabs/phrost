//! What must hold of every range, whatever produced it.
//!
//! The snapshots next door pin exact positions for the constructs they name. This file is for the
//! construct nobody wrote a snapshot for: an arm that reaches for the wrong field produces a range
//! that is inside-out, or empty, or starts in the middle of an identifier, and that is visible
//! without knowing which construct it came from.

use crate::folding::{FoldKind, ranges};

/// Something of every shape the walk has an arm for, in one file.
const DENSE: &str = r#"<?php

namespace App;

use App\First;
use App\Second;

/**
 * Everything at once.
 */
#region Setup
abstract class Repo extends Base implements Reads
{
    use Counts {
        Counts::total insteadof Reads;
    }

    public const array DEFAULTS = [
        'a' => 1,
    ];

    public int $count = 0 {
        get => $this->count;
    }

    public function find(
        int $id,
        array $options = array(
            'deep' => true,
        ),
    ): void {
        // A note.
        // Another note.
        $sql = <<<SQL
            SELECT 1
            SQL;

        list(
            $first,
            $second,
        ) = $this->pair(
            $id,
            $options,
        );

        $name = match ($id) {
            1 => 'one',
            default => 'other',
        };

        switch ($id) {
            case 1:
                break;
        }

        foreach ($options as $option):
            echo $option;
        endforeach;

        $make = function (int $n) {
            return new class {
                public int $n = 0;
            };
        };
    }
}
#endregion
"#;

#[test]
fn no_range_is_empty_inverted_or_outside_the_text() {
    for fold in ranges(DENSE) {
        let start = usize::from(fold.range.start());
        let end = usize::from(fold.range.end());
        assert!(start < end, "inside-out or empty range: {fold:?}");
        assert!(
            end <= DENSE.len(),
            "range past the end of the text: {fold:?}"
        );
        assert!(
            DENSE.is_char_boundary(start) && DENSE.is_char_boundary(end),
            "range splits a character: {fold:?}"
        );
    }
}

#[test]
fn every_range_crosses_a_line_boundary() {
    for fold in ranges(DENSE) {
        let text = &DENSE[usize::from(fold.range.start())..usize::from(fold.range.end())];
        assert!(text.contains('\n'), "single-line range offered: {fold:?}");
    }
}

#[test]
fn ranges_come_back_in_document_order_without_duplicates() {
    let folds = ranges(DENSE);
    for pair in folds.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        assert!(
            (left.range.start(), left.range.end()) < (right.range.start(), right.range.end()),
            "out of order or duplicated: {left:?} then {right:?}"
        );
    }
}

#[test]
fn every_range_starts_where_its_kind_says_it_should() {
    // The check that catches an arm reaching for the wrong field. A fold that begins in the middle
    // of an identifier is a wrong span even when the construct it came from is unknown.
    for fold in ranges(DENSE) {
        let start = &DENSE[usize::from(fold.range.start())..];
        let expected: &[&str] = match fold.kind {
            // An opening delimiter, the colon of the alternative syntax, or a heredoc marker.
            FoldKind::Block => &["{", "[", "(", ":", "<<<"],
            FoldKind::Comment | FoldKind::Region => &["//", "#", "/*"],
            FoldKind::Imports => &["use"],
        };
        assert!(
            expected.iter().any(|prefix| start.starts_with(prefix)),
            "a {:?} range starts at {:?}, expected one of {expected:?}",
            fold.kind,
            start.chars().take(8).collect::<String>()
        );
    }
}

#[test]
fn the_dense_fixture_actually_exercises_every_kind() {
    // Without this the four tests above pass on an empty list, which is exactly how a property
    // test comes to guard nothing.
    let kinds = ranges(DENSE)
        .into_iter()
        .map(|fold| fold.kind)
        .collect::<Vec<_>>();

    for kind in [
        FoldKind::Block,
        FoldKind::Comment,
        FoldKind::Region,
        FoldKind::Imports,
    ] {
        assert!(
            kinds.contains(&kind),
            "the fixture produces no {kind:?} range"
        );
    }
    // A loose floor rather than the exact count, which would make this a snapshot of a number and
    // break on every change to any arm. What it is guarding against is a fixture that stopped
    // parsing and reduced these tests to checking an empty list.
    let blocks = kinds
        .iter()
        .filter(|kind| **kind == FoldKind::Block)
        .count();
    assert!(
        blocks >= 10,
        "only {blocks} delimiter pairs, expected the fixture to be dense"
    );
}
