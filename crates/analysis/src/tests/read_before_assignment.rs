//! A variable read above the line that gives it a value.
//!
//! The end of the path the body layer's own tests cover shape by shape: what a client is shown, and
//! that the sibling check about a name nothing binds at all does not speak about the same name.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_read_above_the_assignment_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): int
    {
        $doubled = $total * 2;
        $total = 21;

        return $doubled;
    }
}
"#,
        expect![[r#"
            6:20-6:26 warning [read-before-assignment] Nothing has given $total a value yet
        "#]],
    );
}

#[test]
fn a_name_nothing_binds_is_still_only_the_unbound_check() {
    // One mistake, one report. The positional check is silent where nothing binds the name at all,
    // and the never-bound one is silent where something does.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(int $applicationId): int
    {
        return $aplicationId + 1;
    }
}
"#,
        expect![[r#"
            6:16-6:29 warning [unbound-variable] Nothing gives $aplicationId a value
        "#]],
    );
}

#[test]
fn a_broken_file_is_not_asked_at_all() {
    // The binding that would clear the finding is by definition below the read, and below the break
    // is exactly the text recovery threw away -- so the whole check stands down rather than
    // reporting a variable the author has already assigned in the half that did not parse.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): int
    {
        $doubled = $total * 2
        $total = 21;

        return $doubled;
    }
}
"#,
        expect![[r#"
            7:9-7:15 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `Variable`
        "#]],
    );
}
