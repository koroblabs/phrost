//! What `?:` leaves behind, against what `??` does.
//!
//! Both operators choose the right-hand value when the left did not survive, and *what* did not
//! survive is the whole difference: `??` tests for null and `?:` tests truthiness, so a `false`
//! lives through the first and cannot live through the second. Reading them as one claim put a
//! `false` in the type of `tempnam(...) ?: NULL`, which is the one thing that line is written to
//! prevent -- `false` is exactly what it replaced.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_elvis_operator_rules_out_the_false_as_well_as_the_null() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
function tempnam(string $directory, string $prefix): string|false {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $path = tempnam('/tmp', 'lv-') ?: NULL;
        $p@elvis@ath;
    }
}
"#,
        &[Query::variable_type("after `?:`", "elvis")],
        expect![[r#"
            after `?:`
            - ?non-falsy-string
        "#]],
    );
}

/// `??` is the other half of the comparison: it names null and nothing else, so the `false` a
/// function reports "nothing" with is still there afterwards.

#[test]
fn the_coalesce_operator_leaves_a_false_where_it_found_one() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
function tempnam(string $directory, string $prefix): string|false {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $path = tempnam('/tmp', 'lv-') ?? NULL;
        $p@coalesce@ath;
    }
}
"#,
        &[Query::variable_type("after `??`", "coalesce")],
        expect![[r#"
            after `??`
            - ?string|false
        "#]],
    );
}
