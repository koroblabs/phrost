//! A list a call built for one variable, then written into.
//!
//! `$crumbs = array_fill(0, $n, null)` and then a value into its slots: the writes were counted
//! only for a variable holding a literal, so the list read as `null[]` -- narrower than the truth,
//! and a `@return string[]` above it reported correct code as returning nothing but nulls. A list
//! the call built is the variable's own exactly as a literal is.
//!
//! Asked through a member under the cursor, which reads the variable the way every check does; the
//! hover card reads the binding alone, for a literal as much as for this.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_writes_after_a_built_list_are_counted() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
/**
 * @template T
 * @param T $value
 * @return list<T>
 */
function array_fill(int $start_index, int $count, mixed $value): array {}

//- /src/Crumbs.php
<?php
namespace App;

class Row
{
    public function name(): string {}
}

class Crumbs
{
    public function run(int $n): void
    {
        $crumbs = array_fill(0, $n, null);
        $crumbs[] = new Row();
        $crumbs[0]?->na@written@me();
    }
}
"#,
        &[Query::cursor("an element after the append", "written")],
        expect![[r#"
            an element after the append
            - member access App\Row::name [method]
        "#]],
    );
}
