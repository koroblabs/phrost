//! `new $class` where `$class` is one of several `Foo::class` constants the code wrote.
//!
//! A ternary, a `match` or two branches each writing a class name: the value is a union of strings
//! to the type layer, so the classes survive only on the path -- and the `new` makes an instance of
//! whichever it names, which is each of them.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_ternary_of_two_class_names_makes_either() {
    check_view(
        r#"
//- /app.php
<?php
class User {}
final class Admin extends User {}

function make(bool $admin): void
{
    $class = $admin ? Admin::class : User::class;
    $made = new $class();
    $m@made@ade;
}
"#,
        &[Query::variable_type("an instance of either", "made")],
        expect![[r#"
            an instance of either
            - Admin|User
        "#]],
    );
}

#[test]
fn a_match_over_class_names_makes_each() {
    check_view(
        r#"
//- /app.php
<?php
class Circle {}
class Square {}
class Line {}

function make(string $kind): void
{
    $class = match ($kind) {
        'circle' => Circle::class,
        'square' => Square::class,
        default => Line::class,
    };
    $shape = new $class();
    $s@shape@hape;
}
"#,
        &[Query::variable_type("an instance of each arm", "shape")],
        expect![[r#"
            an instance of each arm
            - Circle|Square|Line
        "#]],
    );
}

#[test]
fn a_class_name_written_in_each_branch_makes_either() {
    check_view(
        r#"
//- /app.php
<?php
class User {}
class Guest {}

function make(bool $known): void
{
    if ($known) {
        $class = User::class;
    } else {
        $class = Guest::class;
    }
    $made = new $class();
    $m@made@ade;
}
"#,
        &[Query::variable_type("an instance of either branch", "made")],
        expect![[r#"
            an instance of either branch
            - User|Guest
        "#]],
    );
}

/// A path the `new` cannot make anything on is not one whose other half is the answer.
#[test]
fn an_operand_naming_no_class_names_none() {
    check_view(
        r#"
//- /app.php
<?php
class User {}

function make(bool $known, string $other): void
{
    $class = $known ? User::class : $other;
    $made = new $class();
    $m@made@ade;
}
"#,
        &[Query::variable_type("one operand is any string", "made")],
        expect![[r#"
            one operand is any string
            - (no type)
        "#]],
    );
}
