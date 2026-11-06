//! A variable passed to a runtime function's output parameter: a stub's `&` beside `@param-out`.
//!
//! The type the stub writes there is what comes *out*, and the engine does not check what goes in:
//! `$output = 'log'; exec($cmd, $output);` runs on PHP 8.5.10 and leaves a list. Reading the hint as
//! a demand reported Symfony's PHPUnit bridge three times, `proc_open`'s `$pipes` among them. A
//! function anybody wrote enforces its hint on the way in, so this is the stub set's rule alone.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn what_the_runtime_overwrites_is_not_judged_on_the_way_in() {
    check(
        r#"
//- /stubs/standard.php  stub
<?php
/**
 * @param-out list<string> $output
 */
function exec(string $command, ?array &$output = null): string|false {}

//- /src/app.php
<?php

/**
 * @param-out list<string> $output
 */
function collect(string $command, ?array &$output = null): void
{
}

function run(): void
{
    $output = 'log';
    exec('ls', $output);
    $mine = 'log';
    collect('ls', $mine);
}
"#,
        expect![[r#"
            15:19-15:24 error [incompatible-argument] $output takes ?array, and this is non-falsy-string
        "#]],
    );
}
