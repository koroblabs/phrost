//! Statements written after a block control cannot get past.
//!
//! Faded rather than underlined: the code does not run, so nothing about the program is wrong and
//! what it asks for is deletion. The tests that matter are the silences -- a guard, a `switch` arm
//! that breaks, and a declaration PHP hoists are all shapes that read like dead code and are not.

use expect_test::expect;

use super::diagnostics::check;

// `undefined-function` is on by default, so a fixture calling a builtin has to declare it or the
// finding lands beside the one the test exists for. Declared in the fixture rather than accepted
// into the snapshot: a test about which statements are dead should not carry a second finding.

#[test]
fn a_run_of_statements_after_a_return() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): string
    {
        return 'found';
        $id = 1;
        $id = 2;
    }
}
"#,
        expect![[r#"
            7:9-8:17 warning [unreachable-code] Nothing reaches this code
        "#]],
    );
}

#[test]
fn a_guard_leaves_the_block_around_it_alone() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): string
    {
        if ($id < 0) {
            return 'none';
        }
        return 'found';
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_switch_arm_that_breaks_says_nothing_about_the_next_arm() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function label(int $id): string
    {
        switch ($id) {
            case 1:
                $name = 'one';
                break;
            case 2:
                $name = 'two';
                break;
            default:
                $name = 'other';
        }
        return $name;
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_declaration_written_after_a_return_is_hoisted_by_php() {
    check(
        r#"
//- /app.php
<?php

return 'done';

function later(): string
{
    return 'still declared';
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_throw_ends_a_block_and_an_ordinary_call_does_not() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function trigger_error(string $message): bool {}

//- /app.php
<?php
class Refused extends \Exception
{
}

class Repo
{
    public function refuse(int $id): string
    {
        throw new Refused('no');
        $id = 1;
    }

    public function work(int $id): string
    {
        \trigger_error('careful');
        $id = 1;
        return 'done';
    }
}
"#,
        expect![[r#"
            2:23-2:33 error [undefined-type] Undefined type Exception
            11:9-11:17 warning [unreachable-code] Nothing reaches this code
        "#]],
    );
}

#[test]
fn dead_code_inside_a_callback_is_found() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function array_map(callable $callback, array $array): array {}

//- /app.php
<?php
class Repo
{
    public function all(array $rows): array
    {
        return \array_map(function (int $row): int {
            return $row + 1;
            $row = 0;
        }, $rows);
    }
}
"#,
        expect![[r#"
            8:13-8:22 warning [unreachable-code] Nothing reaches this code
        "#]],
    );
}
