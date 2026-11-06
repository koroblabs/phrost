//! PHP 8.5's `|>`, lowered as the call it makes.
//!
//! `$title |> trim(...) |> createDate(...)` passes each left operand as the sole argument of the
//! callable on its right, so the body has to show `createDate(trim($title))` -- a call node per
//! stage, with the piped value as the argument -- and no node of the pipe's own, because every
//! reader of a call already says what the pipe does. The three first-class shapes each become the
//! call they name; a closure on the right is a value being called.

use expect_test::expect;

use super::check_bodies;

#[test]
fn a_chain_of_first_class_callables_is_a_chain_of_calls() {
    check_bodies(
        r#"
//- /src/a.php
<?php

function go(string $title, Formatter $f): void
{
    $result = $title
        |> trim(...)
        |> createDate(...)
        |> $f->format(...)
        |> Formatter::wrap(...);
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 3:1-10:2
            function go
              body @ 4:1-10:2
                assign @ 5:5-9:32
                  $result @ 5:5-5:12
                  =
                    call Formatter::wrap() @ 5:15-9:32
                      args
                        call ->format() @ 5:15-8:27
                          receiver
                            $f @ 8:12-8:14
                          args
                            call createDate() @ 5:15-7:27
                              args
                                call trim() @ 5:15-6:21
                                  args
                                    $title @ 5:15-5:21
        "#]],
    );
}

/// Parenthesised, because PHP refuses the file otherwise -- and the fixture was written without
/// them, so the test pinned a spelling no runtime accepts. See `phrost_syntax`'s `refused`, which
/// is what noticed.
#[test]
fn a_closure_on_the_right_is_called_as_a_value() {
    check_bodies(
        r#"
//- /src/a.php
<?php

function go(string $title): void
{
    $result = $title |> (fn (string $s): int => strlen($s));
    $other = $title |> function (string $s): int { return strlen($s); };
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-8:1
                ?? declaration (arrival) @ 3:1-7:2
            function go
              body @ 4:1-7:2
                assign @ 5:5-5:60
                  $result @ 5:5-5:12
                  =
                    call <value>() @ 5:15-5:60
                      callee
                        fn(string $s): int @ 5:26-5:59
                          body @ 5:49-5:59
                            return @ 5:49-5:59
                              call strlen() @ 5:49-5:59
                                args
                                  $s @ 5:56-5:58
                      args
                        $title @ 5:15-5:21
                assign @ 6:5-6:72
                  $other @ 6:5-6:11
                  =
                    call <value>() @ 6:14-6:72
                      callee
                        closure(string $s): int @ 6:24-6:72
                          body @ 6:50-6:72
                            return @ 6:52-6:70
                              call strlen() @ 6:59-6:69
                                args
                                  $s @ 6:66-6:68
                      args
                        $title @ 6:14-6:20
        "#]],
    );
}
