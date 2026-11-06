//! A guard that names a *subtype* of what a union operand held.
//!
//! The fourth place the same asymmetry between one class and several had to be closed, and the
//! shape is the one the other three had: the single-class path knows all three hierarchy relations
//! -- the guard restates the operand, the guard is below it, or the two are unrelated -- and the
//! union path knew only the first.
//!
//! It knew only the first because it was written as a *filter*: keep the operands that can be the
//! guarded class. `Base|false` under `instanceof Sub` has no operand that is already a `Sub`, so the
//! filter kept nothing, and keeping nothing is read as "this guard says nothing" and leaves the union
//! whole. So the guard ran, returned a type, and changed neither operand -- including the `false`
//! it was written to rule out.
//!
//! Invisible until a union with a sentinel in it became a thing the type layer would admit to
//! holding: while `V|false` was folded into `?V` this went down the nullable path, which strips the
//! null first and then has the whole hierarchy to work with.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return ($array is non-empty-array ? V : V|false)
 */
function end(object|array &$array): mixed {}

//- /src/Kinds.php
<?php
namespace App;

interface Base
{
    public function bundle(): string;
}

interface Sub extends Base
{
    public function id(): int;
}

class Runner
{
    /** @param Base[] $rows */
    public function subtype(array $rows): void
    {
        $last = end($rows);
        if ($last instanceof Sub) {
            $s@sub@ame;
        }
    }

    /** @param Base[] $rows */
    public function sameType(array $rows): void
    {
        $last = end($rows);
        if ($last instanceof Base) {
            $s@same@ame;
        }
    }

    /** @param Sub[] $rows */
    public function supertype(array $rows): void
    {
        $last = end($rows);
        if ($last instanceof Base) {
            $s@super@ame;
        }
    }
}
"#;

/// All three relations, over a union whose other operand is a sentinel.
///
/// The middle one is what already worked, and it is here so the other two are read against it rather
/// than on their own -- a guard that narrows one declaration and not another is the tell this whole
/// family shares.
#[test]
fn each_hierarchy_relation_takes_the_sentinel_off() {
    check_view(
        FIXTURE,
        &[
            Query::variable_class("the guard is below the operand", "sub", "last"),
            Query::variable_class("the guard restates the operand", "same", "last"),
            Query::variable_class("the guard is above the operand", "super", "last"),
        ],
        expect![[r#"
            the guard is below the operand
            - App\Sub
            the guard restates the operand
            - App\Base
            the guard is above the operand
            - App\Sub
        "#]],
    );
}
