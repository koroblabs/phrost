//! Which `foreach` a loop variable's name belongs to, when several bind it.
//!
//! The rule the rest of goto follows is that the *first* write is the declaration: `$x = 1; ...;
//! $x = 2` has one variable and one place it came from. A loop variable is not that. Two sibling
//! `foreach`es over two different lists are two declarations that happen to share a name, and a
//! reader standing in the second means the second -- which is what every other server answers, and
//! what phrost did not: measured on one real method with three loops, all three answered with the
//! first.
//!
//! So a loop the cursor is *inside* wins, innermost first, and failing that the last one written
//! above it -- which is the binding whose value PHP leaked past its closing brace.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn each_loop_answers_with_its_own_binding() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(array $a, array $b, bool $flag): void
    {
        if ($flag) {
            foreach ($a as $item) {
                echo $it@first@em;
            }
        }
        else {
            foreach ($b as $item) {
                echo $it@second@em;
            }
        }
        foreach ($a as $item) {
            echo $it@third@em;
        }
    }
}
"#,
        &[
            Query::goto("inside the first loop", "first"),
            Query::goto("inside the second", "second"),
            Query::goto("and inside the third", "third"),
        ],
        expect![[r#"
            inside the first loop
            - variable item @ src/Repo.php 9:28-9:33 (full 9:28-9:33)
            inside the second
            - variable item @ src/Repo.php 14:28-14:33 (full 14:28-14:33)
            and inside the third
            - variable item @ src/Repo.php 18:24-18:29 (full 18:24-18:29)
        "#]],
    );
}

/// After the loops, PHP still holds the last value, so the binding a reader means is the last one
/// written above the cursor rather than the first.
#[test]
fn a_read_after_the_loops_answers_with_the_last_of_them() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(array $a, array $b): void
    {
        foreach ($a as $item) {
            echo $item;
        }
        foreach ($b as $item) {
            echo $item;
        }
        echo $it@after@em;
    }
}
"#,
        &[Query::goto("after both loops", "after")],
        expect![[r#"
            after both loops
            - variable item @ src/Repo.php 11:24-11:29 (full 11:24-11:29)
        "#]],
    );
}

/// A nested loop that shadows the name it is inside answers with the inner one.
#[test]
fn a_nested_loop_shadows_the_one_around_it() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(array $rows): void
    {
        foreach ($rows as $row) {
            foreach ($row as $row) {
                echo $ro@inner@w;
            }
        }
    }
}
"#,
        &[Query::goto("inside the inner loop", "inner")],
        expect![[r#"
            inside the inner loop
            - variable row @ src/Repo.php 9:30-9:34 (full 9:30-9:34)
        "#]],
    );
}
