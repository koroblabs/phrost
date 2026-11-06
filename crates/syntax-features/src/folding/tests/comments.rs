//! Folds that come from trivia rather than from the tree.

use expect_test::expect;

use super::check;

#[test]
fn a_docblock_over_several_lines_folds_and_a_one_line_one_does_not() {
    check(
        r#"<?php

/**
 * Finds things.
 */
class Repo
{
}

/** One line. */
class Other
{
}
"#,
        expect![[r#"
            comment 3:0-5:3  /**
            block   7:0-8:1  {
            block   12:0-13:1  {
        "#]],
    );
}

#[test]
fn a_plain_block_comment_folds_too() {
    check(
        r#"<?php

/*
 * Not documentation, still collapsible.
 */
$x = 1;
"#,
        expect![[r#"
            comment 3:0-5:3  /*
        "#]],
    );
}

#[test]
fn a_run_of_line_comments_folds_as_one() {
    check(
        r#"<?php

// First.
// Second.
// Third.
$x = 1;
"#,
        expect![[r#"
            comment 3:0-5:9  // First.
        "#]],
    );
}

#[test]
fn hash_comments_run_together_with_slash_ones() {
    // Both are single-line comments in PHP, and nobody writing them thinks of the difference.
    check(
        r#"<?php

# First.
// Second.
$x = 1;
"#,
        expect![[r#"
            comment 3:0-4:10  # First.
        "#]],
    );
}

#[test]
fn a_blank_line_ends_a_run() {
    check(
        r#"<?php

// First.
// Second.

// Third.
// Fourth.
$x = 1;
"#,
        expect![[r#"
            comment 3:0-4:10  // First.
            comment 6:0-7:10  // Third.
        "#]],
    );
}

#[test]
fn a_comment_after_code_does_not_join_the_run_below_it() {
    // It is a remark about that statement. Folding from the middle of a line to somewhere below
    // collapses code the comment was never about.
    check(
        r#"<?php

$x = 1; // Why.
// First.
// Second.
$y = 2;
"#,
        expect![[r#"
            comment 4:0-5:10  // First.
        "#]],
    );
}

#[test]
fn a_lone_line_comment_is_not_a_fold() {
    check("<?php\n\n// Alone.\n$x = 1;\n", expect![""]);
}

#[test]
fn a_region_pair_folds_in_every_spelling_people_use() {
    check(
        r#"<?php

#region Setup
$a = 1;
#endregion

//region Work
$b = 2;
//endregion

// #region Teardown
$c = 3;
// #endregion
"#,
        expect![[r#"
            region  3:0-5:10  #region Setup
            region  7:0-9:11  //region Work
            region  11:0-13:13  // #region Teardown
        "#]],
    );
}

#[test]
fn regions_nest() {
    check(
        r#"<?php

#region Outer
#region Inner
$a = 1;
#endregion
#endregion
"#,
        expect![[r#"
            region  3:0-7:10  #region Outer
            region  4:0-6:10  #region Inner
        "#]],
    );
}

#[test]
fn an_unmatched_region_marker_contributes_nothing() {
    // Where it ends is a guess, and both available guesses -- the end of the file, or the
    // enclosing block -- are wrong often enough to be worse than no fold arrow.
    check(
        r#"<?php

#region Never closed
$a = 1;
"#,
        expect![""],
    );
}

#[test]
fn a_region_marker_does_not_also_form_a_comment_run() {
    // Without this the pair would fold twice over almost the same lines, once as a region and
    // once as two adjacent comments.
    check(
        r#"<?php

#region Setup
#endregion
"#,
        expect![[r#"
            region  3:0-4:10  #region Setup
        "#]],
    );
}

#[test]
fn an_editor_fold_marker_is_a_region_too() {
    check(
        r#"<?php

//<editor-fold desc="Setup">
$a = 1;
//</editor-fold>
"#,
        expect![[r#"
            region  3:0-5:16  //<editor-fold desc="Setup">
        "#]],
    );
}

#[test]
fn a_region_that_crosses_a_block_boundary_is_offered_as_written() {
    // Ranges from the tree nest. A region does not have to: `#region` is a comment and can be
    // opened inside a method and closed outside it. Nothing here squares that -- the pair is
    // reported where it was written, and the client decides what to draw. Repairing it would mean
    // guessing which of the two the author meant.
    check(
        r#"<?php

function build(): void
{
    #region Inside
    $a = 1;
}
#endregion
"#,
        expect![[r#"
            block   4:0-7:1  {
            region  5:4-8:10  #region Inside
        "#]],
    );
}
