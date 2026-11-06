//! Which bodies are generators.
//!
//! A `yield` anywhere makes a function a generator, and then its declared return type describes the
//! *generator* rather than what `return` produces. Anything judging a returned value has to know,
//! or it reports every generator in the project.
//!
//! The hard half is scope. A `yield` belongs to the innermost function-like around it, so a closure
//! that yields does not make its enclosing function a generator, and a function that yields does not
//! make a closure inside it one.

use expect_test::expect;

use super::check_bodies;

#[test]
fn a_yielding_function_is_a_generator() {
    check_bodies(
        r#"
//- /src/a.php
<?php
function counter(): iterable
{
    yield 1;
    yield 2;
}

function plain(): int
{
    return 1;
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-12:1
                ?? declaration (arrival) @ 2:1-6:2
                ?? declaration (arrival) @ 8:1-11:2
            function counter
              body @ 3:1-6:2  [generator]
                ?? expression @ 4:5-4:12
                  int @ 4:11-4:12
                ?? expression @ 5:5-5:12
                  int @ 5:11-5:12
            function plain
              body @ 9:1-11:2
                return @ 10:5-10:14
                  int @ 10:12-10:13
        "#]],
    );
}

#[test]
fn a_yield_from_counts_too() {
    check_bodies(
        r#"
//- /src/a.php
<?php
function counter(): iterable
{
    yield from [1, 2];
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-6:1
                ?? declaration (arrival) @ 2:1-5:2
            function counter
              body @ 3:1-5:2  [generator]
                ?? expression @ 4:5-4:22
                  array @ 4:16-4:22
                    int @ 4:17-4:18
                    int @ 4:20-4:21
        "#]],
    );
}

#[test]
fn a_yield_deep_inside_control_flow_still_counts() {
    check_bodies(
        r#"
//- /src/a.php
<?php
function counter(array $rows): iterable
{
    foreach ($rows as $row) {
        if ($row) {
            yield $row;
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            function counter
              body @ 3:1-9:2  [generator]
                foreach @ 4:5-8:6
                  $rows @ 4:14-4:19
                  value
                    $row @ 4:23-4:27
                  if @ 5:9-7:10
                    $row @ 5:13-5:17
                    then
                      ?? expression @ 6:13-6:23
                        $row @ 6:19-6:23
        "#]],
    );
}

#[test]
fn a_closure_that_yields_leaves_the_function_around_it_alone() {
    // The case a naive flag gets wrong, and the reason this is per body rather than per function.
    check_bodies(
        r#"
//- /src/a.php
<?php
function make(): callable
{
    return function () {
        yield 1;
    };
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-8:1
                ?? declaration (arrival) @ 2:1-7:2
            function make
              body @ 3:1-7:2
                return @ 4:5-6:7
                  closure() @ 4:12-6:6
                    body @ 4:24-6:6  [generator]
                      ?? expression @ 5:9-5:16
                        int @ 5:15-5:16
        "#]],
    );
}

#[test]
fn a_closure_inside_a_generator_is_not_one_itself() {
    // And the mirror of it: the flag must not leak downwards either.
    check_bodies(
        r#"
//- /src/a.php
<?php
function counter(): iterable
{
    yield 1;
    $plain = function () {
        return 2;
    };
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            function counter
              body @ 3:1-8:2  [generator]
                ?? expression @ 4:5-4:12
                  int @ 4:11-4:12
                assign @ 5:5-7:6
                  $plain @ 5:5-5:11
                  =
                    closure() @ 5:14-7:6
                      body @ 5:26-7:6
                        return @ 6:9-6:18
                          int @ 6:16-6:17
        "#]],
    );
}
