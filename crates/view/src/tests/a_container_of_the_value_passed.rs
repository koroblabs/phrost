//! `array_fill(0, 3, 'value')` -- a container of whatever the call passed.
//!
//! The stub binds `T` to the value parameter itself (`@param T $value`) and returns it in a container
//! under a conditional: `($start_index is 0 ? ($count is int<1, max> ? non-empty-list<T> : list<T>)
//! : (...))`. Nothing read a container of a template a parameter is declared as, and the inner test
//! is a range nothing settles, so the whole contract was refused and every call read as the
//! declaration's `array<int, mixed>` -- `$strings[0]->missing()` passed unremarked.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn every_branch_of_array_fill_holds_the_value() {
    check_view(
        r#"
//- /stubs/builtins.php  stub
<?php
/**
 * @template T
 *
 * @param int<0, max> $count
 * @param T $value
 *
 * @return (
 *   $start_index is 0 ?
 *   ($count is int<1, max> ? non-empty-list<T> : list<T>) :
 *   ($count is int<1, max> ? non-empty-array<int, T> : array<int, T>)
 * )
 */
function array_fill(int $start_index, int $count, mixed $value): array {}

//- /src/Caller.php
<?php
namespace App;

class Row {}

class Caller
{
    public function run(int $start): void
    {
        $fromZero = array_fill(0, 3, new Row());
        $f@from_zero@romZero;
        $fromFive = array_fill(5, 3, new Row());
        $f@from_five@romFive;
        $fromAnywhere = array_fill($start, 3, new Row());
        $f@from_anywhere@romAnywhere;
        $element = $fromAnywhere[0];
        $e@element@lement;
    }
}
"#,
        &[
            Query::variable_type("the settled list branch", "from_zero"),
            Query::variable_type("the settled array branch", "from_five"),
            Query::variable_type("a start nothing settles", "from_anywhere"),
            Query::variable_type("what one slot holds", "element"),
        ],
        expect![[r#"
            the settled list branch
            - Row[]
            the settled array branch
            - Row[]
            a start nothing settles
            - Row[]
            what one slot holds
            - Row
        "#]],
    );
}

#[test]
fn a_plain_list_of_the_template_holds_the_value() {
    // The same claim without a conditional around it, which the plain-return reader learned at the
    // same time: one rule for a container of the argument, wherever it is written.
    check_view(
        r#"
//- /src/Caller.php
<?php
namespace App;

class Row {}

/**
 * @template T
 * @param T $value
 * @return list<T>
 */
function wrap(mixed $value): array { return [$value]; }

/**
 * @template T
 * @param T $value
 * @return T[]
 */
function wrapKeyless(mixed $value): array { return [$value]; }

class Caller
{
    public function run(): void
    {
        $wrapped = wrap(new Row());
        $w@wrapped@rapped;
        $keyless = wrapKeyless(new Row());
        $k@keyless@eyless;
    }
}
"#,
        &[
            Query::variable_type("a list of it", "wrapped"),
            Query::variable_type("a container that states no key is not read", "keyless"),
        ],
        expect![[r#"
            a list of it
            - Row[]
            a container that states no key is not read
            - array
        "#]],
    );
}
