//! `$find = [$repository, 'find']; $find()`: a pair of a variable and a method name, invoked,
//! gives back what the method does.
//!
//! The literal is an array to every other reader, because `[$id, 'asc']` is the same shape holding
//! data, and its type keeps the variable's class but not the method name. Being called is what
//! makes it a callable, and the path is what still spells which method.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_local_holding_a_pair_invokes_its_method() {
    check_view(
        r#"
//- /app.php
<?php
class User {}

class Repository
{
    public function find(): User { return new User(); }
}

function run(Repository $repository): void
{
    $lookup = [$repository, 'find'];
    $found = $lookup();
    $f@found@ound;
}
"#,
        &[Query::variable_type("what the method returns", "found")],
        expect![[r#"
            what the method returns
            - User
        "#]],
    );
}

#[test]
fn a_pair_invoked_where_it_is_written_invokes_its_method() {
    check_view(
        r#"
//- /app.php
<?php
class User {}

class Repository
{
    public function find(): User { return new User(); }
}

function run(Repository $repository): void
{
    $found = [$repository, 'find']();
    $f@found@ound;
}
"#,
        &[Query::variable_type("what the method returns", "found")],
        expect![[r#"
            what the method returns
            - User
        "#]],
    );
}

/// The class has no such method, so the pair names nothing and calling it is an error, not a call.
#[test]
fn a_pair_naming_no_method_of_the_class_invokes_nothing() {
    check_view(
        r#"
//- /app.php
<?php
class Repository
{
    public function find(): int { return 1; }
}

function run(Repository $repository): void
{
    $lookup = [$repository, 'missing'];
    $found = $lookup();
    $f@found@ound;
}
"#,
        &[Query::variable_type("no method by that name", "found")],
        expect![[r#"
            no method by that name
            - (no type)
        "#]],
    );
}

/// A slot written since the literal names another method, and the binding still reads as the
/// literal -- so the pair is not read at all.
#[test]
fn a_pair_written_into_since_is_not_the_literal_any_more() {
    check_view(
        r#"
//- /app.php
<?php
class Repository
{
    public function find(): int { return 1; }
    public function other(): string { return ''; }
}

function run(Repository $repository): void
{
    $lookup = [$repository, 'find'];
    $lookup[1] = 'other';
    $found = $lookup();
    $f@found@ound;
}
"#,
        &[Query::variable_type(
            "the method is no longer the one written",
            "found",
        )],
        expect![[r#"
            the method is no longer the one written
            - (no type)
        "#]],
    );
}
